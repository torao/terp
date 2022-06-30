use crate::schema::{Item, Location, MatchResult, Primary, Schema, Syntax};
use crate::{Error, Result};
use std::fmt::{Debug, Display};
use std::hash::Hash;

#[cfg(test)]
mod test;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event<ID, E: Item>
where
  ID: Clone + Hash + Eq + Display + Debug,
{
  pub location: E::Location,
  pub kind: EventKind<ID, E>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind<ID, E: Item>
where
  ID: Clone + Hash + Eq + Debug,
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
  ongoing: Vec<State<'s, ID, E>>,
  prev_completed: Vec<State<'s, ID, E>>,
  prev_unmatched: Vec<State<'s, ID, E>>,
}

impl<'s, ID, E: 'static + Item, H: FnMut(Event<ID, E>)> Context<'s, ID, E, H>
where
  ID: Clone + Hash + Eq + Ord + Display + Debug,
{
  pub fn new(schema: &'s Schema<ID, E>, id: ID, event_handler: H) -> Result<E, Self> {
    let buffer = Vec::with_capacity(1024);
    let mut first = State::new(&id, schema)?;
    first.last_result = if let Syntax { primary: Primary::Term(_), .. } = first.current() {
      first.matches(&buffer)?
    } else {
      MatchResult::Match(0)
    };

    first.event_buffer.push(Event { location: E::Location::default(), kind: EventKind::Begin(id.clone()) });

    let location = E::Location::default();
    let ongoing = vec![first];
    let prev_completed = Vec::with_capacity(16);
    let prev_unmatched = Vec::with_capacity(16);
    let mut initial_context =
      Self { id, event_handler, location, buffer, offset_of_buffer_head: 0, ongoing, prev_completed, prev_unmatched };
    initial_context.move_ongoing_states_to_next_term()?;
    Ok(initial_context)
  }

  pub fn id(&self) -> &ID {
    &self.id
  }

  pub fn push(&mut self, item: E) -> Result<E, ()> {
    println!("PUSH: {:?}", item);

    self.check_error(false, Some(item))?;

    if self.ongoing.len() == 1 {
      self.ongoing[0].flush_events(&mut self.event_handler);
    }

    // reduce internal buffer if possible
    self.fit_buffer_to_min_size();

    // add item into buffer
    self.buffer.push(item);
    self.location.increment_with(item);

    self.evaluate_ongoing_states_and_move_matched_ones_to_next_term(false)?;

    self.check_error(false, None)?;

    Ok(())
  }

  pub fn finish(mut self) -> Result<E, ()> {
    println!("FINISH");

    self.check_error(true, None)?;

    while !self.ongoing.is_empty() {
      self.evaluate_ongoing_states_and_move_matched_ones_to_next_term(true)?;
    }

    match self.prev_completed.len() {
      1 => {
        // notify all remaining events and success
        self.prev_completed[0].flush_events(&mut self.event_handler);

        // notify end of requested syntax
        (self.event_handler)(Event { location: self.location, kind: EventKind::End(self.id) });

        Ok(())
      }
      0 => self.check_error(true, None),
      _ => {
        let mut expecteds = Vec::with_capacity(self.prev_completed.len());
        let mut actual = String::new();
        for state in &self.prev_completed {
          match self.error_unmatch_with_eof(Some(state), None) {
            Error::Unmatched { expected, actual: a, .. } => {
              expecteds.push(expected);
              if actual.is_empty() {
                actual = a;
              }
            }
            _ => panic!(),
          }
        }
        Err(Error::MultipleMatches { location: self.location, expecteds, actual })
      }
    }
  }

  fn move_ongoing_states_to_next_term(&mut self) -> Result<E, ()> {
    debug_assert!(!self.ongoing.is_empty());
    let mut ongoing = self.ongoing.drain(..).collect::<Vec<_>>();
    let mut term_reached = Vec::with_capacity(ongoing.len());
    while let Some(mut state) = ongoing.pop() {
      match &state.current().primary {
        Primary::Term(..) => {
          term_reached.push(state);
        }
        Primary::Alias(id) => {
          let next_syntax = state.schema.get(id).ok_or_else(|| Error::UndefinedID(id.to_string()))?;
          debug_assert!(matches!(next_syntax.primary, Primary::Seq(..)));
          state.stack.push((next_syntax, 0));
          state.event_buffer.push(Event { location: state.location, kind: EventKind::Begin(id.clone()) });
          ongoing.push(state);
        }
        Primary::Seq(..) => {
          state.stack.push((state.current(), 0));
          ongoing.push(state);
        }
        Primary::Or(branches) => {
          for branch in branches {
            let mut next = state.clone();
            debug_assert!(matches!(branch.primary, Primary::Seq(..)));
            next.stack.push((branch, 0));
            ongoing.push(next);
          }
        }
      }
    }
    debug_assert!(!term_reached.is_empty());
    debug_assert!(term_reached.iter().all(|t| matches!(t.current().primary, Primary::Term(..))));
    self.ongoing = term_reached;
    Ok(())
  }

  fn evaluate_ongoing_states_and_move_matched_ones_to_next_term(&mut self, eof: bool) -> Result<E, ()> {
    let mut ongoing = self.ongoing.drain(..).collect::<Vec<_>>();
    if !eof {
      self.prev_completed.truncate(0);
      self.prev_unmatched.truncate(0);
    }

    while let Some(mut state) = ongoing.pop() {
      debug_assert!(matches!(state.current().primary, Primary::Term(..)));
      match state.matches(&self.buffer)? {
        MatchResult::Match(_) => (),
        MatchResult::MatchAndCanAcceptMore if eof => {
          state.last_result = MatchResult::Match(state.match_length);
        }
        MatchResult::Unmatch => {
          self.prev_unmatched.push(state);
          continue;
        }
        MatchResult::UnmatchAndCanAcceptMore if eof => {
          state.last_result = MatchResult::Unmatch;
          self.prev_unmatched.push(state);
          continue;
        }
        MatchResult::MatchAndCanAcceptMore | MatchResult::UnmatchAndCanAcceptMore => {
          self.ongoing.push(state);
          continue;
        }
      }

      debug_assert!(matches!(state.current().primary, Primary::Term(..)));
      debug_assert!(matches!(state.last_result, MatchResult::Match(_)));
      if state.match_length > 0 {
        let values = self.buffer[state.match_begin..][..state.match_length].to_vec();
        state.event_buffer.push(Event { location: state.location, kind: EventKind::Fragments(values) });
      }

      let last_pointed_term = *state.stack.last().unwrap();
      while let Some((Syntax { primary: Primary::Seq(sequence), .. }, i)) = state.stack.last_mut() {
        if *i + 1 < sequence.len() {
          if state.match_length > 0 {
            state.location.increment_with_seq(&self.buffer[state.match_begin..][..state.match_length]);
            state.match_begin += state.match_length;
            state.match_length = 0;
          }
          state.repeated = 0;

          *i += 1;
          self.ongoing.push(state);
          break;
        }
        if state.stack.len() == 1 {
          state.stack = vec![last_pointed_term];
          self.prev_completed.push(state);
          break;
        } else {
          state.stack.pop();
          if let Syntax { primary: Primary::Alias(id), .. } = state.current() {
            state.event_buffer.push(Event { location: state.location, kind: EventKind::End(id.clone()) });
          }
        }
      }
    }

    if self.ongoing.is_empty() {
      Ok(())
    } else {
      self.move_ongoing_states_to_next_term()
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
          .map(|u| if eof { self.error_unmatch_with_eof(Some(u), None) } else { self.error_unmatch(Some(u)) })
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
    if self.location.position() & BUFFER_SHRINKAGE_CHECKPOINT == BUFFER_SHRINKAGE_CHECKPOINT {
      let states = vec![&mut self.ongoing, &mut self.prev_completed, &mut self.prev_unmatched];
      let states = states.into_iter().flatten().collect::<Vec<_>>();
      let min_offset = states.iter().map(|s| s.match_begin).min().unwrap();
      if min_offset > 0 {
        self.buffer.drain(0..min_offset);
        self.offset_of_buffer_head += min_offset as u64;
        for state in states {
          state.match_begin -= min_offset;
        }
      }
    }
  }

  fn error_unmatch(&self, expected: Option<&State<ID, E>>) -> Error<E> {
    let actual = self.buffer.last().copied();
    let buffer = if self.buffer.is_empty() { &self.buffer[..] } else { &self.buffer[..(self.buffer.len() - 1)] };
    Self::error_unmatch_with(self.location, buffer, self.offset_of_buffer_head, expected, actual)
  }

  fn error_unmatch_with_eof(&self, expected: Option<&State<ID, E>>, actual: Option<E>) -> Error<E> {
    Self::error_unmatch_with(self.location, &self.buffer, self.offset_of_buffer_head, expected, actual)
  }

  fn error_unmatch_with(
    location: E::Location, buffer: &[E], buffer_offset: u64, expected: Option<&State<ID, E>>, actual: Option<E>,
  ) -> Error<E> {
    const SMPL_LEN: usize = 8;
    const ELPS_LEN: usize = 3;
    const EOF_SYMBOL: &str = "EOF";

    let sampling_end = expected.map(|s| s.match_begin).unwrap_or(buffer.len());
    let sampling_begin = sampling_end - std::cmp::min(SMPL_LEN, sampling_end);
    let prefix_length = std::cmp::min(ELPS_LEN as u64, buffer_offset - sampling_begin as u64) as usize;
    let prefix = (0..prefix_length).map(|_| ".").collect::<String>();
    let expected = {
      let sample = E::debug_symbols(&buffer[sampling_begin..sampling_end]);
      let suffix = expected.map(|s| s.current().to_string()).unwrap_or_else(|| String::from(EOF_SYMBOL));
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
    Error::Unmatched { location, expected, actual }
  }
}

/// The `Cursor` advances step by step, evaluating [`Syntax`] matches.
///
#[derive(Clone, Debug)]
struct State<'s, ID, E: Item>
where
  ID: Clone + Hash + Eq + Ord + Display + Debug,
{
  schema: &'s Schema<ID, E>,
  location: E::Location,
  match_begin: usize,
  match_length: usize,
  last_result: MatchResult,
  repeated: usize,
  event_buffer: Vec<Event<ID, E>>,

  /// The [`Syntax`] must be `Syntax::Seq`.
  stack: Vec<(&'s Syntax<ID, E>, usize)>,
}

impl<'s, ID, E: 'static + Item> State<'s, ID, E>
where
  ID: Clone + Hash + Eq + Ord + Display + Debug,
{
  pub fn new(id: &ID, schema: &'s Schema<ID, E>) -> Result<E, Self> {
    let syntax = if let Some(syntax) = schema.get(id) {
      debug_assert!(matches!(&syntax.primary, Primary::Seq(_)));
      syntax
    } else {
      return Err(Error::UndefinedID(id.to_string()));
    };

    Ok(Self {
      schema,
      location: E::Location::default(),
      match_begin: 0,
      match_length: 0,
      last_result: MatchResult::Unmatch,
      repeated: 0,
      event_buffer: Vec::new(),
      stack: vec![(syntax, 0)],
    })
  }

  pub fn current(&self) -> &'s Syntax<ID, E> {
    if let Some((Syntax { primary: Primary::Seq(sequence), .. }, index)) = self.stack.last() {
      &sequence[*index]
    } else {
      panic!("unexpected deepest stack frame: {:?}", self.stack.last());
    }
  }

  pub fn flush_events<H: FnMut(Event<ID, E>)>(&mut self, handler: &mut H) {
    while !self.event_buffer.is_empty() {
      (handler)(self.event_buffer.remove(0));
    }
  }

  pub fn matches(&mut self, buffer: &[E]) -> Result<E, MatchResult> {
    debug_assert!(buffer.len() >= self.match_begin + self.match_length);

    let reps = self.current().repetition.clone();
    if self.repeated == *reps.end() {
      return Ok(MatchResult::Match(self.match_length));
    }
    debug_assert!(self.repeated < *reps.end(), "{} >= {}", self.repeated, reps.end());

    let matcher = if let Primary::Term(matcher) = &self.current().primary {
      matcher
    } else {
      panic!("current syntax is not term(matcher): {:?}", self.current())
    };

    let buffer = &buffer[(self.match_begin + self.match_length)..];
    let result = match matcher.matches(buffer)? {
      MatchResult::Match(length) => {
        self.repeated += 1;
        self.match_length += length;
        if self.repeated < *reps.start() {
          MatchResult::UnmatchAndCanAcceptMore
        } else if reps.contains(&(self.repeated + 1)) {
          MatchResult::MatchAndCanAcceptMore
        } else {
          debug_assert_eq!(*reps.end(), self.repeated);
          MatchResult::Match(self.match_length)
        }
      }
      MatchResult::Unmatch => {
        if reps.contains(&self.repeated) {
          MatchResult::Match(self.match_length)
        } else {
          debug_assert!(self.repeated < *reps.start());
          MatchResult::Unmatch
        }
      }
      MatchResult::UnmatchAndCanAcceptMore if reps.contains(&self.repeated) => MatchResult::MatchAndCanAcceptMore,
      result => result,
    };

    self.last_result = result;
    Ok(result)
  }
}
