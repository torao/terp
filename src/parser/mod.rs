use crate::schema::{Item, Location, Primary, Schema, Syntax};
use crate::{debug, Error, Result};
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
  ID: Clone + Hash + Eq + Ord + Display + Debug,
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
  ID: 's + Clone + Hash + Eq + Ord + Display + Debug,
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

  pub fn id(&self) -> &ID {
    &self.id
  }

  pub fn push(&mut self, item: E) -> Result<E, ()> {
    debug!("PUSH: {:?}, buf_size={}", item, self.buffer.len());
    for (i, path) in self.ongoing.iter().enumerate() {
      debug!("  ONGOING[{}]: {}", i, path)
    }

    self.check_error(false, Some(item))?;

    // add item into buffer
    self.buffer.push(item);
    self.location.increment_with(item);

    self.proceed(false)?;

    self.check_error(false, None)?;

    if self.ongoing.len() == 1 && self.prev_completed.is_empty() {
      self.ongoing[0].events_flush_to(&mut self.event_handler);
    } else if self.ongoing.is_empty() && self.prev_completed.len() == 1 {
      self.prev_completed[0].events_flush_to(&mut self.event_handler);
    }

    // reduce internal buffer if possible
    self.fit_buffer_to_min_size();

    Ok(())
  }

  pub fn finish(mut self) -> Result<E, ()> {
    debug!("FINISH");

    self.check_error(true, None)?;

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
      0 => self.check_error(true, None),
      _ => {
        let mut expecteds = Vec::with_capacity(self.prev_completed.len());
        let mut repr_actual = String::new();
        for path in &self.prev_completed {
          let expected = Some((path.current().match_begin, path.current().syntax().to_string()));
          let (expected, actual) = error_unmatch_labels(&self.buffer, self.offset_of_buffer_head, expected, None);
          expecteds.push(expected);
          if repr_actual.is_empty() {
            repr_actual = actual;
          }
        }
        Err(Error::MultipleMatches { location: self.location, expecteds, actual: repr_actual })
      }
    }
  }

  fn proceed(&mut self, eof: bool) -> Result<E, ()> {
    if !eof {
      self.prev_completed.truncate(0);
      self.prev_unmatched.truncate(0);
    }
    let mut ongoing = Vec::with_capacity(self.ongoing.len());
    for path in self.ongoing.drain(..) {
      ongoing.append(&mut Self::move_ongoing_paths_to_next_term(path)?);
    }

    while let Some(mut path) = ongoing.pop() {
      debug_assert!(matches!(path.current().syntax().primary, Primary::Term(..)));
      debug!("~ === {}", path);

      let matched = match path.current_mut().matches(&self.buffer, eof)? {
        Matching::Match(_length, event) => {
          if let Some(event) = event {
            path.events_push(event);
          }
          debug_assert!(matches!(path.current().syntax().primary, Primary::Term(..)));
          true
        }
        Matching::Unmatch => false,
        Matching::More => {
          self.ongoing.push(path);
          continue;
        }
      };

      match path.move_to_next(&self.buffer, matched, eof) {
        (true, true) => {
          let uncapture_exists = path.current().match_begin + path.current().match_length < self.buffer.len();
          if uncapture_exists {
            self.prev_unmatched.push(path);
          } else {
            self.prev_completed.push(path);
          }
        }
        (true, _) => {
          let uncapture_exists = path.current().match_begin + path.current().match_length < self.buffer.len();
          let mut nexts = Self::move_ongoing_paths_to_next_term(path)?;
          if uncapture_exists {
            ongoing.append(&mut nexts);
          } else {
            self.ongoing.append(&mut nexts);
          }
        }
        (false, _) => self.prev_unmatched.push(path),
      }
    }

    Self::merge_paths(&mut self.ongoing);
    Self::merge_paths(&mut self.prev_completed);
    Self::merge_paths(&mut self.prev_unmatched);
    Ok(())
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

  fn check_error(&self, eof: bool, item: Option<E>) -> Result<E, ()> {
    if self.ongoing.is_empty() {
      return if !self.prev_completed.is_empty() {
        if item.is_some() {
          Err(self.error_unmatch_with_eof(None, item))
        } else {
          Ok(())
        }
      } else if !self.prev_unmatched.is_empty() {
        let errors = self
          .prev_unmatched
          .iter()
          .map(|p| {
            if eof {
              self.error_unmatch_with_eof(Some(p.current()), None)
            } else {
              self.error_unmatch(Some(p.current()))
            }
          })
          .collect::<Vec<_>>();
        Error::errors(errors)
      } else {
        unreachable!("There is no outgoing or confirmed state");
      };
    }
    Ok(())
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

  fn error_unmatch(&self, expected: Option<&State<ID, E>>) -> Error<E> {
    let location = expected.map(|s| s.location).unwrap_or(self.location);
    let actual = self.buffer.last().copied();
    let buffer = if self.buffer.is_empty() { &self.buffer[..] } else { &self.buffer[..(self.buffer.len() - 1)] };
    Self::error_unmatch_with(location, buffer, self.offset_of_buffer_head, expected, actual)
  }

  fn error_unmatch_with_eof(&self, expected: Option<&State<ID, E>>, actual: Option<E>) -> Error<E> {
    let location = expected.map(|s| s.location).unwrap_or(self.location);
    Self::error_unmatch_with(location, &self.buffer, self.offset_of_buffer_head, expected, actual)
  }

  fn error_unmatch_with(
    location: E::Location, buffer: &[E], buffer_offset: u64, expected: Option<&State<ID, E>>, actual: Option<E>,
  ) -> Error<E> {
    let expected = expected.map(|s| (s.match_begin, s.syntax().to_string()));
    let (expected, actual) = error_unmatch_labels(buffer, buffer_offset, expected, actual);
    Error::Unmatched { location, expected, actual }
  }
}

fn error_unmatch_labels<E: Item>(
  buffer: &[E], buffer_offset: u64, expected: Option<(usize, String)>, actual: Option<E>,
) -> (String, String) {
  const ELPS_LEN: usize = 3;
  const EOF_SYMBOL: &str = "EOF";
  debug_assert!(expected.is_some() || actual.is_some());
  debug_assert!(
    expected.as_ref().map(|x| x.0 <= buffer.len()).unwrap_or(true),
    "buffer overrun: {} > {}",
    expected.as_ref().unwrap().0,
    buffer.len()
  );

  let smpl_len = E::SAMPLING_UNIT_AT_ERROR;
  let sampling_end = expected.as_ref().map(|(begin, _)| *begin).unwrap_or(buffer.len());
  let sampling_begin = sampling_end - std::cmp::min(smpl_len, sampling_end);
  let prefix_length = std::cmp::min(ELPS_LEN as u64, buffer_offset + sampling_begin as u64) as usize;
  let prefix = (0..prefix_length).map(|_| ".").collect::<String>();
  let expected = {
    let sample = E::debug_symbols(&buffer[sampling_begin..sampling_end]);
    let suffix = expected.map(|s| s.1).unwrap_or_else(|| String::from(EOF_SYMBOL));
    format!("{}{}[{}]", prefix, sample, suffix)
  };
  let actual = {
    let sample = if buffer.len() - sampling_begin <= smpl_len * 3 + ELPS_LEN {
      E::debug_symbols(&buffer[sampling_begin..])
    } else {
      let head = E::debug_symbols(&buffer[sampling_begin..][..smpl_len * 2]);
      let ellapse = (0..ELPS_LEN).map(|_| ".").collect::<String>();
      let tail = E::debug_symbols(&buffer[buffer.len() - smpl_len..]);
      format!("{}{}{}", head, ellapse, tail)
    };
    let suffix = actual.map(|i| E::debug_symbol(i)).unwrap_or_else(|| String::from(EOF_SYMBOL));
    format!("{}{}[{}]", prefix, sample, suffix)
  };
  (expected, actual)
}

impl<'s, ID, H: FnMut(Event<ID, char>)> Context<'s, ID, char, H>
where
  ID: 's + Clone + Hash + Eq + Ord + Display + Debug,
{
  pub fn push_str(&mut self, s: &str) -> Result<char, ()> {
    for ch in s.chars() {
      self.push(ch)?;
    }
    Ok(())
  }
}
