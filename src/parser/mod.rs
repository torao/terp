use crate::schema::{Item, Location, Primary, Schema, Syntax};
use crate::{debug, Error, Result};
use std::cmp::Ordering;
use std::fmt::{Debug, Display};
use std::hash::Hash;

mod path;
pub(crate) use path::*;

mod event;
pub use event::*;

#[cfg(test)]
pub mod test;

pub struct Context<'s, ID, E: Item, H: FnMut(Event<ID, E>)>
where
  ID: Clone + Hash + Eq + Ord + Display + Debug + Send + Sync,
{
  id: ID,
  event_handler: H,
  location: E::Location,
  buffer: Vec<E>,
  offset_of_buffer_head: u64,
  ongoing: Vec<Path<'s, ID, E>>,
  prev_completed: Vec<Path<'s, ID, E>>,
  prev_unmatched: Vec<Path<'s, ID, E>>,
}

impl<'s, ID, E: 'static + Item, H: FnMut(Event<ID, E>)> Context<'s, ID, E, H>
where
  ID: 's + Clone + Hash + Eq + Ord + Display + Debug + Send + Sync,
{
  pub fn new(schema: &'s Schema<ID, E>, id: ID, event_handler: H) -> Result<E, Self> {
    let buffer = Vec::with_capacity(1024);

    let mut first = Path::new(&id, schema)?;
    first.events_push(first.current().event(EventKind::Begin(id.clone())));
    let mut ongoing = Vec::with_capacity(16);
    ongoing.push(first);

    let location = E::Location::default();
    let prev_completed = Vec::with_capacity(16);
    let prev_unmatched = Vec::with_capacity(16);
    Ok(Self { id, event_handler, location, buffer, offset_of_buffer_head: 0, ongoing, prev_completed, prev_unmatched })
  }

  pub fn ignore_events_for(mut self, ids: &[ID]) -> Self {
    for ongoing in &mut self.ongoing {
      ongoing.event_buffer_mut().ignore_events_for(ids);
    }
    self
  }

  pub fn id(&self) -> &ID {
    &self.id
  }

  pub fn push(&mut self, item: E) -> Result<E, ()> {
    let buffer = [item];
    self.push_seq(&buffer)
  }

  pub fn push_seq(&mut self, items: &[E]) -> Result<E, ()> {
    debug!(
      "PUSH: {:?}, buf_size={}, {}",
      E::debug_symbols(items),
      self.buffer.len(),
      if cfg!(feature = "concurrent") { "concurrent" } else { "serial" }
    );
    for (i, path) in self.ongoing.iter().enumerate() {
      debug!("  ONGOING[{}]: {}", i, path)
    }

    self.buffer.reserve(items.len());
    for item in items {
      self.buffer.push(*item);
    }
    self.location.increment_with_seq(items);

    self.check_for_previous_error()?;
    self.check_whether_possible_to_proceed()?;

    // append items into buffer
    if items.is_empty() {
      return Ok(());
    }

    self.proceed(false)?;


    if self.ongoing.len() == 1 && self.prev_completed.is_empty() {
      self.ongoing[0].events_flush_to(&mut self.event_handler);
    } else if self.ongoing.is_empty() && self.prev_completed.len() == 1 {
      self.prev_completed[0].events_flush_to(&mut self.event_handler);
    }
    self.check_whether_unmatch_confirmed()?;

    // reduce internal buffer if possible
    self.fit_buffer_to_min_size();

    Ok(())
  }

  pub fn finish(mut self) -> Result<E, ()> {
    debug!("FINISH");

    self.check_for_previous_error()?;

    while !self.ongoing.is_empty() {
      self.proceed(true)?;
    }

    match self.prev_completed.len() {
      1 => {
        // notify all remaining events and success
        self.prev_completed[0].completed();
        self.prev_completed[0].events_push(Event { location: self.location, kind: EventKind::End(self.id) });
        self.prev_completed[0].events_flush_to(&mut self.event_handler);

        Ok(())
      }
      0 => self.error(self.error_unmatch(&self.prev_unmatched)),
      _ => {
        let (prefix, expecteds, actual) =
          create_unmatched_labels(&self.buffer, self.offset_of_buffer_head, &self.prev_completed);
        self.error(Error::MultipleMatches { location: self.location, prefix, expecteds, actual })
      }
    }
  }

  fn proceed(&mut self, eof: bool) -> Result<E, ()> {
    if !eof {
      self.prev_completed.truncate(0);
      self.prev_unmatched.truncate(0);
    }
    let mut evaluating: Vec<Path<'s, ID, E>> = Vec::with_capacity(self.ongoing.len());
    for path in self.ongoing.drain(..) {
      evaluating.append(&mut Self::move_ongoing_paths_to_next_term(path)?);
    }

    let mut i = 0;
    while !evaluating.is_empty() {
      debug!("--- iteration[{}] ---", i + 1);
      i += 1;

      let nexts = {
        #[cfg(feature = "concurrent")]
        if evaluating.len() == 1 {
          vec![Self::proceed_on_path(evaluating.pop().unwrap(), &self.buffer, eof)]
        } else {
          use rayon::prelude::*;
          evaluating.par_drain(..).map(|path| Self::proceed_on_path(path, &self.buffer, eof)).collect::<Vec<_>>()
        }

        #[cfg(not(feature = "concurrent"))]
        evaluating.drain(..).map(|path| Self::proceed_on_path(path, &self.buffer, eof)).collect::<Vec<_>>()
      };

      for next in nexts {
        let NextPaths { mut need_to_be_reevaluated, mut ongoing, unmatched, completed } = next?;
        evaluating.append(&mut need_to_be_reevaluated);
        self.ongoing.append(&mut ongoing);
        if let Some(unmatched) = unmatched {
          self.push_unmatched(unmatched);
        }
        if let Some(completed) = completed {
          self.prev_completed.push(completed);
        }
      }
      Self::merge_paths(&mut evaluating);
    }

    Self::merge_paths(&mut self.ongoing);
    Self::merge_paths(&mut self.prev_completed);
    Ok(())
  }

  fn proceed_on_path(mut path: Path<'s, ID, E>, buffer: &[E], eof: bool) -> Result<E, NextPaths<'s, ID, E>> {
    debug_assert!(matches!(path.current().syntax().primary, Primary::Term(..)));
    debug!("~ === proceed_on_path({}, {}, {})", path, E::debug_symbols(&buffer[path.current().match_begin..]), eof,);

    let mut next = NextPaths {
      need_to_be_reevaluated: Vec::with_capacity(1),
      ongoing: Vec::with_capacity(1),
      unmatched: None,
      completed: None,
    };

    let matched = match path.matches(buffer, eof)? {
      Matching::Match(_length, event) => {
        if let Some(event) = event {
          path.events_push(event);
        }
        debug_assert!(matches!(path.current().syntax().primary, Primary::Term(..)));
        true
      }
      Matching::Unmatch => false,
      Matching::More => {
        next.ongoing.push(path);
        return Ok(next);
      }
    };

    match path.move_to_next(buffer, matched, eof) {
      (true, true) => {
        let uncapture_exists = path.current().match_begin + path.current().match_length < buffer.len();
        if uncapture_exists {
          next.unmatched = Some(path);
        } else {
          next.completed = Some(path);
        }
      }
      (true, _) => {
        let uncapture_exists = path.current().match_begin + path.current().match_length < buffer.len();
        let mut nexts = Self::move_ongoing_paths_to_next_term(path)?;
        if uncapture_exists {
          next.need_to_be_reevaluated.append(&mut nexts);
        } else {
          next.ongoing.append(&mut nexts);
        }
      }
      (false, _) => next.unmatched = Some(path),
    }
    Ok(next)
  }

  fn move_ongoing_paths_to_next_term(path: Path<'s, ID, E>) -> Result<E, Vec<Path<'s, ID, E>>> {
    let mut ongoing = vec![path];
    let mut term_reached = Vec::with_capacity(ongoing.len());
    while let Some(mut eval_path) = ongoing.pop() {
      match &eval_path.current().syntax().primary {
        Primary::Term(..) => {
          term_reached.push(eval_path);
        }
        Primary::Alias(id) => {
          eval_path.stack_push_alias(id)?;
          eval_path.events_push(eval_path.current().event(EventKind::Begin(id.clone())));
          ongoing.push(eval_path);
        }
        Primary::Seq(seq) => {
          eval_path.stack_push(seq);
          ongoing.push(eval_path);
        }
        Primary::Or(branches) => {
          for branch in branches {
            if let Syntax { primary: Primary::Seq(seq), .. } = branch {
              let mut next = eval_path.clone();
              next.stack_push(seq);
              ongoing.push(next);
            } else {
              unreachable!("Primary::Or contains a branch other than Seq")
            }
          }
        }
      }
    }
    debug_assert!(!term_reached.is_empty());
    debug_assert!(term_reached.iter().all(|t| matches!(t.current().syntax().primary, Primary::Term(..))));
    Ok(term_reached)
  }

  fn merge_paths(paths: &mut Vec<Path<ID, E>>) {
    for i in 0..paths.len() {
      let mut j = i + 1;
      while j < paths.len() {
        if paths[i].can_merge(&paths[j]) {
          debug!("~ duplicated: [{},{}]{}", i, j, paths[j]);
          paths.remove(j);
        } else {
          j += 1;
        }
      }
    }
  }

  fn push_unmatched(&mut self, path: Path<'s, ID, E>) {
    let save = if let Some(current) = self.prev_unmatched.last() {
      match path.current().location.cmp(&current.current().location) {
        Ordering::Greater => {
          self.prev_unmatched.truncate(0);
          true
        }
        Ordering::Equal => !self.prev_unmatched.iter().any(|c| c.can_merge(&path)),
        Ordering::Less => false,
      }
    } else {
      true
    };
    if save {
      self.prev_unmatched.push(path);
    }
  }

  fn fit_buffer_to_min_size(&mut self) {
    // reduce internal buffer if possible
    // TODO: how often the buffer is reduced?
    const BUFFER_SHRINKAGE_CHECKPOINT_BIT: usize = 8;
    const BUFFER_SHRINKAGE_CHECKPOINT: u64 = (1u64 << BUFFER_SHRINKAGE_CHECKPOINT_BIT) - 1u64;
    if self.location.position() & BUFFER_SHRINKAGE_CHECKPOINT != BUFFER_SHRINKAGE_CHECKPOINT {
      return;
    }
    let paths = vec![&mut self.ongoing, &mut self.prev_completed, &mut self.prev_unmatched];
    let paths = paths.into_iter().flatten().collect::<Vec<_>>();
    let min_offset = paths.iter().map(|p| p.min_match_begin()).min().unwrap();
    if min_offset > 0 {
      self.buffer.drain(0..min_offset);
      self.offset_of_buffer_head += min_offset as u64;
      for path in paths {
        path.on_buffer_shrunk(min_offset);
      }
    }
  }

  fn check_whether_possible_to_proceed(&mut self) -> Result<E, ()> {
    debug_assert!(!self.ongoing.is_empty() || !self.prev_completed.is_empty() || !self.prev_unmatched.is_empty());
    if self.ongoing.is_empty() {
      if !self.prev_completed.is_empty() {
        // `items` appeared, but the parser state was already complete and waiting for EOF
        let pos = self.prev_completed.iter().map(|p| p.current().location.position()).max().unwrap();
        let buffer_pos = (pos - self.offset_of_buffer_head) as usize;
        if self.buffer.len() == buffer_pos {
          Ok(())
        } else {
          self.error(self.error_eof_expected(&self.prev_completed))
        }
      } else {
        // if unmatch has already been confirmed but the application has attempted to make a further push
        self.check_whether_unmatch_confirmed()
      }
    } else {
      Ok(())
    }
  }

  fn check_whether_unmatch_confirmed(&mut self) -> Result<E, ()> {
    debug_assert!(!self.ongoing.is_empty() || !self.prev_completed.is_empty() || !self.prev_unmatched.is_empty());
    if self.ongoing.is_empty() && self.prev_completed.is_empty() {
      self.error(self.error_unmatch(&self.prev_unmatched))
    } else {
      Ok(())
    }
  }

  fn check_for_previous_error(&self) -> Result<E, ()> {
    if self.ongoing.is_empty() && self.prev_completed.is_empty() && self.prev_unmatched.is_empty() {
      Err(Error::Previous)
    } else {
      Ok(())
    }
  }

  fn error_unmatch(&self, expecteds: &[Path<ID, E>]) -> Error<E> {
    let location = expecteds.first().map(|p| p.current().location).unwrap_or(self.location);
    let expected_syntaxes = expecteds.iter().map(|p| p.to_string()).collect::<Vec<_>>();
    let (prefix, expecteds, actual) = create_unmatched_labels(&self.buffer, self.offset_of_buffer_head, expecteds);
    Error::Unmatched { location, prefix, expecteds, expected_syntaxes, actual }
  }

  fn error_eof_expected(&self, completed: &[Path<ID, E>]) -> Error<E> {
    let location = completed.first().map(|p| p.current().location).unwrap_or(self.location);
    let match_length = completed.first().map(|p| p.current().match_begin).unwrap_or(self.buffer.len());
    let prefix = create_unmatched_label_prefix(&self.buffer, self.offset_of_buffer_head, match_length);
    let expected = format!("[{}]", EOF_SYMBOL);
    let actual = create_unmatched_label_actual(&self.buffer, match_length);
    Error::Unmatched { location, prefix, expecteds: vec![expected], expected_syntaxes: vec![], actual }
  }

  fn error<T>(&mut self, err: Error<E>) -> Result<E, T> {
    self.ongoing.truncate(0);
    self.prev_unmatched.truncate(0);
    self.prev_completed.truncate(0);
    Err(err)
  }
}

fn create_unmatched_labels<ID, E: Item>(
  buffer: &[E], buf_offset: u64, expecteds: &[Path<ID, E>],
) -> (String, Vec<String>, String)
where
  ID: Clone + Display + Debug + PartialEq + Ord + Eq + Hash,
{
  let match_length = expecteds.first().map(|p| p.current().match_begin).unwrap_or(buffer.len());
  debug_assert!(expecteds.iter().all(|p| p.current().match_begin == match_length));

  debug_assert!(!expecteds.is_empty());
  let expecteds = expecteds.iter().map(|path| format!("[{}]", path.current().syntax())).collect::<Vec<_>>();

  (
    create_unmatched_label_prefix(buffer, buf_offset, match_length),
    expecteds,
    create_unmatched_label_actual(buffer, match_length),
  )
}

const ELLAPSE_LENGTH: usize = 3;
const EOF_SYMBOL: &str = "EOF";

fn create_unmatched_label_prefix<E: Item>(buffer: &[E], buf_offset: u64, match_length: usize) -> String {
  debug_assert!(match_length <= buffer.len());
  let sample_length = E::SAMPLING_UNIT_AT_ERROR;
  let sample_end = match_length;
  let sample_begin = sample_end - std::cmp::min(sample_length, sample_end);
  let ellapse_length = std::cmp::min(ELLAPSE_LENGTH as u64, buf_offset + sample_begin as u64) as usize;
  let ellapse = (0..ellapse_length).map(|_| ".").collect::<String>();
  let sample = E::debug_symbols(&buffer[sample_begin..sample_end]);
  format!("{}{}", ellapse, sample)
}

fn create_unmatched_label_actual<E: Item>(buffer: &[E], match_length: usize) -> String {
  let sample_length = E::SAMPLING_UNIT_AT_ERROR;
  if match_length < buffer.len() {
    let target = E::debug_symbol(buffer[match_length]);
    if match_length + 1 < buffer.len() {
      let suffix_length = std::cmp::min(sample_length, buffer.len() - match_length - 1);
      let suffix = E::debug_symbols(&buffer[match_length + 1..][..suffix_length]);
      format!("[{}]{}...", target, suffix)
    } else {
      format!("[{}]...", target)
    }
  } else {
    debug_assert!(match_length == buffer.len());
    format!("[{}]", EOF_SYMBOL)
  }
}

impl<'s, ID, H: FnMut(Event<ID, char>)> Context<'s, ID, char, H>
where
  ID: 's + Clone + Hash + Eq + Ord + Display + Debug + Send + Sync,
{
  pub fn push_str(&mut self, s: &str) -> Result<char, ()> {
    self.push_seq(&s.chars().collect::<Vec<_>>())
  }
}

struct NextPaths<'s, ID, E: Item>
where
  ID: 's + Clone + Hash + Eq + Ord + Display + Debug + Send + Sync,
{
  pub need_to_be_reevaluated: Vec<Path<'s, ID, E>>,
  pub ongoing: Vec<Path<'s, ID, E>>,
  pub unmatched: Option<Path<'s, ID, E>>,
  pub completed: Option<Path<'s, ID, E>>,
}
