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
  states: Vec<State<'s, ID, E>>,
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

    let states = first.move_to_next_term(&buffer, true)?;
    let location = E::Location::default();
    Ok(Self { id, event_handler, location, buffer, offset_of_buffer_head: 0, states })
  }

  pub fn id(&self) -> &ID {
    &self.id
  }

  pub fn push(&mut self, item: E) -> Result<E, ()> {
    println!("PUSH: {:?}", item);

    // move the complete matched cursor to the next
    let mut terminated = Vec::with_capacity(self.states.len());
    let mut unmatched = Vec::with_capacity(self.states.len());
    self.progress(&mut terminated, &mut unmatched, false)?;

    // if there's no cursor to be evaluated, it means that a EOF was expected but followed value was present
    if self.states.is_empty() {
      return Err(self.error_unmatch_with_eof(None, Some(item)));
    }

    // reduce internal event buffer by immediate delivering if only one cursor is present
    if self.states.len() == 1 {
      self.states[0].flush_events(&mut self.event_handler);
    }

    // reduce internal buffer if possible
    // TODO: how often the buffer is reduced?
    const BUFFER_SHRINKAGE_THRESHOLD: usize = 0;
    let min_offset = self.states.iter().map(|s| s.match_begin).min().unwrap();
    if min_offset > BUFFER_SHRINKAGE_THRESHOLD {
      self.buffer.drain(0..min_offset);
      self.offset_of_buffer_head += min_offset as u64;
      for c in &mut self.states {
        c.match_begin -= min_offset;
      }
    }

    // add item into buffer
    self.buffer.push(item);
    self.location.increment_with(item);

    // update all in-process status and move to the next cursor if possible
    // TODO: parallelize
    let mut states = self.states.drain(..).collect::<Vec<_>>();
    for state in &mut states {
      self.evaluate(state)?;
    }
    self.states = states;

    if self.states.iter().all(|s| matches!(s.last_result, MatchResult::Unmatch)) {
      let mut errors = Vec::with_capacity(self.states.len());
      for i in 0..self.states.len() {
        errors.push(self.error_unmatch(Some(&self.states[i])));
      }
      self.dispose();
      return Error::errors(errors);
    }

    Ok(())
  }

  pub fn finish(mut self) -> Result<E, ()> {
    println!("FINISH");

    if self.states.is_empty() {
      // there should be an error in advance
      return Err(Error::UnableToContinue);
    }

    // move the complete matched cursor to the next
    let mut terminated = Vec::with_capacity(self.states.len());
    let mut unmatched = Vec::with_capacity(self.states.len());
    self.progress(&mut terminated, &mut unmatched, true)?;
    assert!(self.states.is_empty());

    // if unmatch
    if terminated.is_empty() {
      debug_assert!(!unmatched.is_empty());
      return Error::errors(unmatched);
    }

    // if multiple matches are detected
    if terminated.len() > 1 {
      let mut expecteds = Vec::with_capacity(terminated.len());
      let mut actual = String::new();
      for state in &terminated {
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
      return Err(Error::MultipleMatches { location: self.location, expecteds, actual });
    }

    // notify all remaining events and success
    let mut state = terminated.remove(0);
    state.flush_events(&mut self.event_handler);

    // notify end of requested syntax
    (self.event_handler)(Event { location: self.location, kind: EventKind::End(self.id) });

    Ok(())
  }

  /// move the complete matched cursor to the next
  ///
  fn progress(
    &mut self, terminated: &mut Vec<State<'s, ID, E>>, unmatched: &mut Vec<Error<E>>, eof: bool,
  ) -> Result<E, ()> {
    let mut states = self.states.drain(..).rev().collect::<Vec<_>>();
    while let Some(mut state) = states.pop() {
      match (state.last_result, eof) {
        (MatchResult::Match(_), _) | (MatchResult::MatchAndCanAcceptMore, true) => {
          if eof {
            state.last_result = MatchResult::Match(state.match_length);
          }
          let mut nexts = state.clone().move_to_next_term(&self.buffer, false)?;
          for s in &mut nexts {
            self.evaluate(s)?;
          }
          if nexts.is_empty() {
            terminated.push(state);
          } else if eof {
            states.append(&mut nexts);
          } else {
            self.states.append(&mut nexts);
          }
        }
        (MatchResult::Unmatch, _) => {
          unmatched.push(self.error_unmatch(Some(&state)));
        }
        (MatchResult::UnmatchAndCanAcceptMore, true) => {
          unmatched.push(self.error_unmatch_with_eof(Some(&state), None));
        }
        (MatchResult::MatchAndCanAcceptMore, false) | (MatchResult::UnmatchAndCanAcceptMore, false) => {
          self.states.push(state);
        }
      }
    }

    debug_assert!(self.states.iter().all(|s| matches!(&s.current().primary, Primary::Term(..))));
    debug_assert!(!eof || self.states.is_empty());

    Ok(())
  }

  fn evaluate(&self, state: &mut State<'s, ID, E>) -> Result<E, ()> {
    state.last_result = state.matches(&self.buffer)?;
    println!(
      "  MATCHES: {} x {:?} => {:?}",
      state.current(),
      E::debug_symbols(&self.buffer[state.match_begin..]),
      state.last_result
    );
    match state.last_result {
      MatchResult::Match(_length) => {
        // store sequence matching the current cursor in events
        let event = state.new_fragment_event(&self.buffer);
        state.event_buffer.push(event);
      }
      MatchResult::Unmatch if state.match_length > 0 => {
        // store previous sequence matching the current cursor in events
        let event = state.new_fragment_event(&self.buffer);
        state.event_buffer.push(event);
        state.last_result = MatchResult::Match(state.match_length);
      }
      _ => (),
    }
    Ok(())
  }

  fn dispose(&mut self) {
    self.buffer.truncate(0);
    self.states.truncate(0);
  }

  fn error_unmatch(&self, expected: Option<&State<ID, E>>) -> Error<E> {
    assert!(!self.buffer.is_empty());
    let actual = self.buffer.last().copied();
    let buffer = &self.buffer[..(self.buffer.len() - 1)];
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

  pub fn move_to_next_term(mut self, buffer: &[E], from_current: bool) -> Result<E, Vec<State<'s, ID, E>>> {
    debug_assert!(self.match_begin + self.match_length <= buffer.len());

    self.location.increment_with_seq(&buffer[self.match_begin..][..self.match_length]);
    self.match_begin += self.match_length;
    self.match_length = 0;
    self.repeated = 0;

    let mut in_process = if from_current { vec![self] } else { self.next()? };
    let mut term_reached = Vec::with_capacity(in_process.len());
    while let Some(state) = in_process.pop() {
      match state.last_result {
        MatchResult::Match(_) => {
          let mut routes = state.next()?;
          while let Some(mut route) = routes.pop() {
            if route.is_term() {
              route.matches(&buffer[..route.match_begin])?;
              term_reached.push(route);
            } else {
              in_process.push(route);
            }
          }
        }
        MatchResult::MatchAndCanAcceptMore | MatchResult::UnmatchAndCanAcceptMore => {
          if state.is_term() {
            term_reached.push(state);
          } else {
            in_process.append(&mut state.next()?);
          }
        }
        MatchResult::Unmatch => panic!(),
      }
    }
    debug_assert!(term_reached.iter().all(|s| s.is_term()));

    Ok(term_reached)
  }

  fn next(mut self) -> Result<E, Vec<State<'s, ID, E>>> {
    // refer to the deepest sequence in the stack and the index being currently evaluated
    let (sequence, index) = match self.stack.last_mut() {
      Some((Syntax { primary: Primary::Seq(sequence), .. }, index)) => (sequence, index),
      _ => panic!("stack frame contains non-sequence syntax: {:?}", self.stack),
    };
    debug_assert!(*index < sequence.len());

    let routes = match &sequence[*index].primary {
      Primary::Term(..) => {
        print!("  MOVE: {} => ", &sequence[*index]);
        if *index + 1 < sequence.len() {
          *index += 1;
          println!("{}", &sequence[*index]);
          self.last_result = MatchResult::UnmatchAndCanAcceptMore;
          vec![self]
        } else {
          self.stack.pop();
          while !self.stack.is_empty() {
            if let Some((Syntax { primary: Primary::Seq(seq), .. }, i)) = self.stack.last_mut() {
              if *i + 1 < seq.len() {
                *i += 1;
                println!("{}", &seq[*i]);
                self.last_result = MatchResult::UnmatchAndCanAcceptMore;
                break;
              }
            } else {
              panic!("stack frame contains non-sequence syntax: {:?}", self.stack);
            }
            self.stack.pop();
          }
          // the stack frame to be evaluated is the topmost one
          if self.stack.is_empty() {
            println!("EOF");
            vec![]
          } else {
            vec![self]
          }
        }
      }
      Primary::Seq(_) => {
        let seq = &sequence[*index];
        self.stack.push((seq, 0));
        vec![self]
      }
      Primary::Alias(id) => {
        let syntax = if let Some(syntax) = self.schema.get(id) {
          assert!(matches!(syntax.primary, Primary::Seq(_)));
          syntax
        } else {
          return Err(Error::UndefinedID(id.to_string()));
        };
        self.stack.push((syntax, 0));
        vec![self]
      }
      Primary::Or(branches) => {
        let mut routes = Vec::with_capacity(branches.len());
        for branch in branches {
          assert!(matches!(branch.primary, Primary::Seq(_)));
          let mut state = self.clone();
          state.stack.push((branch, 0));
          routes.push(state);
        }
        routes
      }
    };
    Ok(routes)
  }

  pub fn current(&self) -> &'s Syntax<ID, E> {
    if let Some((Syntax { primary: Primary::Seq(sequence), .. }, index)) = self.stack.last() {
      &sequence[*index]
    } else {
      panic!()
    }
  }

  pub fn is_term(&self) -> bool {
    matches!(self.current(), Syntax { primary: Primary::Term(_), .. })
  }

  pub fn new_fragment_event(&self, buffer: &[E]) -> Event<ID, E> {
    let values = buffer[self.match_begin..][..self.match_length].to_vec();
    let location = self.location;
    let kind = EventKind::Fragments(values);
    Event { location, kind }
  }

  pub fn flush_events<H: FnMut(Event<ID, E>)>(&mut self, handler: &mut H) {
    while !self.event_buffer.is_empty() {
      (handler)(self.event_buffer.remove(0));
    }
  }

  pub fn matches(&mut self, buffer: &[E]) -> Result<E, MatchResult> {
    debug_assert!(buffer.len() >= self.match_begin + self.match_length);

    let result = if let Primary::Term(matcher) = &self.current().primary {
      let repetition = self.current().repetition.clone();
      match matcher.matches(&buffer[(self.match_begin + self.match_length)..])? {
        MatchResult::Match(length) => {
          self.repeated += 1;
          self.match_length += length;
          if self.repeated < *repetition.start() {
            MatchResult::UnmatchAndCanAcceptMore
          } else if repetition.contains(&(self.repeated + 1)) {
            MatchResult::MatchAndCanAcceptMore
          } else {
            debug_assert_eq!(*repetition.end(), self.repeated);
            MatchResult::Match(self.match_length)
          }
        }
        MatchResult::Unmatch => {
          if repetition.contains(&self.repeated) {
            MatchResult::Match(self.match_length)
          } else {
            debug_assert!(self.repeated < *repetition.start());
            MatchResult::Unmatch
          }
        }
        result => result,
      }
    } else {
      panic!("current syntax is not term(matcher): {:?}", self.current())
    };
    self.last_result = result;
    Ok(result)
  }
}
