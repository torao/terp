use crate::schema::{Item, Location, Primary, Schema, Syntax};
use crate::{Error, Result};
use std::fmt::{Debug, Display};
use std::hash::Hash;

mod path;
pub(crate) use path::*;

#[cfg(test)]
mod test;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event<ID, E: Item>
where
  ID: Clone + Display + Debug,
{
  pub location: E::Location,
  pub kind: EventKind<ID, E>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind<ID, E: Item>
where
  ID: Clone + Debug,
{
  Begin(ID),
  End(ID),
  Fragments(Vec<E>),
}

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

    let location = E::Location::default();
    let ongoing = Self::move_ongoing_paths_to_next_term(first)?;
    let prev_completed = Vec::with_capacity(16);
    let prev_unmatched = Vec::with_capacity(16);
    Ok(Self { id, event_handler, location, buffer, offset_of_buffer_head: 0, ongoing, prev_completed, prev_unmatched })
  }

  pub fn id(&self) -> &ID {
    &self.id
  }

  pub fn push(&mut self, item: E) -> Result<E, ()> {
    println!("PUSH: {:?}, buf_size={}", item, self.buffer.len());
    for (i, state) in self.ongoing.iter().enumerate() {
      println!("  ONGOING[{}]: {}", i, state.current().syntax())
    }

    self.check_error(false, Some(item))?;

    if self.ongoing.len() == 1 {
      self.ongoing[0].events_flush_to(&mut self.event_handler);
    }

    // reduce internal buffer if possible
    self.fit_buffer_to_min_size();

    // add item into buffer
    self.buffer.push(item);
    self.location.increment_with(item);

    self.evaluate_ongoing_paths_and_move_matched_ones_to_next_term(false)?;

    self.check_error(false, None)?;

    Ok(())
  }

  pub fn finish(mut self) -> Result<E, ()> {
    println!("FINISH");

    self.check_error(true, None)?;

    while !self.ongoing.is_empty() {
      self.evaluate_ongoing_paths_and_move_matched_ones_to_next_term(true)?;
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
          let (expected, actual) =
            Self::error_unmatch_labels(&self.buffer, self.offset_of_buffer_head, Some(path.current()), None);
          expecteds.push(expected);
          if repr_actual.is_empty() {
            repr_actual = actual;
          }
        }
        Err(Error::MultipleMatches { location: self.location, expecteds, actual: repr_actual })
      }
    }
  }

  fn move_ongoing_paths_to_next_term(path: Path<'s, ID, E>) -> Result<E, Vec<Path<'s, ID, E>>> {
    let mut ongoing = vec![path];
    let mut term_reached = Vec::with_capacity(ongoing.len());
    while let Some(mut path) = ongoing.pop() {
      match &path.current().syntax().primary {
        Primary::Term(..) => {
          term_reached.push(path);
        }
        Primary::Alias(id) => {
          path.stack_push_alias(id)?;
          path.events_push(path.current().event(EventKind::Begin(id.clone())));
          ongoing.push(path);
        }
        Primary::Seq(seq) => {
          path.stack_push(seq);
          ongoing.push(path);
        }
        Primary::Or(branches) => {
          for branch in branches {
            if let Syntax { primary: Primary::Seq(seq), .. } = branch {
              let mut next = path.clone();
              next.stack_push(seq);
              ongoing.push(next);
            } else {
              panic!()
            }
          }
        }
      }
    }
    debug_assert!(!term_reached.is_empty());
    debug_assert!(term_reached.iter().all(|t| matches!(t.current().syntax().primary, Primary::Term(..))));
    Ok(term_reached)
  }

  fn evaluate_ongoing_paths_and_move_matched_ones_to_next_term(&mut self, eof: bool) -> Result<E, ()> {
    let mut ongoing = self.ongoing.drain(..).collect::<Vec<_>>();
    if !eof {
      self.prev_completed.truncate(0);
      self.prev_unmatched.truncate(0);
    }

    while let Some(mut path) = ongoing.pop() {
      debug_assert!(matches!(path.current().syntax().primary, Primary::Term(..)));

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
          if uncapture_exists {
            ongoing.append(&mut Self::move_ongoing_paths_to_next_term(path)?);
          } else {
            self.ongoing.append(&mut Self::move_ongoing_paths_to_next_term(path)?);
          }
        }
        (false, _) => self.prev_unmatched.push(path),
      }
    }

    Ok(())
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
        panic!("there is no outgoing or confirmed state");
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
    let (expected, actual) = Self::error_unmatch_labels(buffer, buffer_offset, expected, actual);
    Error::Unmatched { location, expected, actual }
  }

  fn error_unmatch_labels(
    buffer: &[E], buffer_offset: u64, expected: Option<&State<ID, E>>, actual: Option<E>,
  ) -> (String, String) {
    const SMPL_LEN: usize = 8;
    const ELPS_LEN: usize = 3;
    const EOF_SYMBOL: &str = "EOF";

    let sampling_end = expected.map(|s| s.match_begin).unwrap_or(buffer.len());
    let sampling_begin = sampling_end - std::cmp::min(SMPL_LEN, sampling_end);
    let prefix_length = std::cmp::min(ELPS_LEN as u64, buffer_offset + sampling_begin as u64) as usize;
    let prefix = (0..prefix_length).map(|_| ".").collect::<String>();
    let expected = {
      let sample = E::debug_symbols(&buffer[sampling_begin..sampling_end]);
      let suffix = expected.map(|s| s.syntax().to_string()).unwrap_or_else(|| String::from(EOF_SYMBOL));
      format!("{}{}[{}]", prefix, sample, suffix)
    };
    let actual = {
      let sample = if buffer.len() - sampling_begin <= SMPL_LEN * 3 + ELPS_LEN {
        E::debug_symbols(&buffer[sampling_begin..])
      } else {
        let head = E::debug_symbols(&buffer[sampling_begin..][..SMPL_LEN * 2]);
        let ellapse = (0..ELPS_LEN).map(|_| ".").collect::<String>();
        let tail = E::debug_symbols(&buffer[buffer.len() - SMPL_LEN..]);
        format!("{}{}{}", head, ellapse, tail)
      };
      let suffix = actual.map(|i| E::debug_symbol(i)).unwrap_or_else(|| String::from(EOF_SYMBOL));
      format!("{}{}[{}]", prefix, sample, suffix)
    };
    (expected, actual)
  }
}
