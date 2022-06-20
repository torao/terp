use crate::schema::{Item, Location, MatchResult, Primary, Schema, Syntax};
use crate::{Error, Result};

pub struct Event<E: Item> {
  pub location: E::Location,
  pub kind: EventKind<E>,
}

pub enum EventKind<E: Item> {
  Begin(String),
  End(String),
  Fragments(Vec<E>),
}

pub struct Context<'s, E: Item, H: FnMut(Event<E>)> {
  name: String,
  schema: &'s Schema<E>,
  event_handler: H,
  location: E::Location,
  buffer: Vec<E>,
  offset_of_buffer_head: u64,
  cursors: Vec<State<'s, E>>,
}

impl<'s, E: Item, H: FnMut(Event<E>)> Context<'s, E, H> {
  pub fn new(schema: &'s Schema<E>, name: &str, event_handler: H) -> Self {
    let syntax = schema.get(name).unwrap_or_else(|| panic!("{:?} isn't defined in the schema: {:?}", name, schema));
    debug_assert!(matches!(&syntax.primary, Primary::Seq(_)));

    Self {
      name: name.to_string(),
      schema,
      event_handler,
      location: E::Location::default(),
      buffer: Vec::with_capacity(1024),
      offset_of_buffer_head: 0,
      cursors: vec![State::new(syntax)],
    }
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn push(&mut self, item: E) -> Result<()> {
    // if there's no cursor to be evaluated, it means that a EOF was expected but followed value was present
    if self.cursors.is_empty() {
      return Err(Error::Unexpected(self.debug_symbols_at_tail(5, false), E::debug_symbol(item)));
    }

    // add item into buffer
    self.buffer.push(item);
    self.location.next_with(item);

    // update all in-process status and move to the next cursor if possible
    // TODO: parallelize
    let mut unmatches = Vec::new();
    let mut i = 0;
    while i < self.cursors.len() {
      if let Primary::Term(matcher) = &self.cursors[i].current_syntax().primary {
        let values = &self.buffer[self.cursors[i].buffer_begin..];
        let last_result = self.cursors[i].last_result;
        self.cursors[i].last_result = matcher.matches(values)?;
        match (last_result, self.cursors[i].last_result) {
          (_, MatchResult::Match) => {
            // store sequence matching the current cursor in events
            let location = self.cursors[i].location;
            let kind = EventKind::Fragments(values.to_vec());
            self.cursors[i].event_buffer.push(Event { location, kind });

            // move the complete matched cursors to the next, and skip to evaluate them
            let nexts = self.cursors[i].next_steps(self.schema);
            let nexts_len = nexts.len();
            self.cursors.splice(i..=i, nexts);
            i += nexts_len;
          }
          (MatchResult::MatchAndCanAcceptMore, MatchResult::Unmatch) => {
            // store previous sequence matching the current cursor in events
            let values = &self.buffer[self.cursors[i].buffer_begin..self.buffer.len()];
            if !values.is_empty() {
              let location = self.cursors[i].location;
              let kind = EventKind::Fragments(values.to_vec());
              self.cursors[i].event_buffer.push(Event { location, kind });
            }

            // move the complete matched cursors to the next, and reevaluate them
            let nexts = self.cursors[i].next_steps(self.schema);
            self.cursors.splice(i..=i, nexts);
          }
          (_, MatchResult::Unmatch) => {
            self.cursors.remove(i);
            let expected = self.cursors[i].current_syntax().to_string();
            let actual = E::debug_symbol(item);
            unmatches.push(Error::Unexpected(expected, actual));
          }
          _ => i += 1,
        }
      } else {
        panic!();
      }
    }

    // if there're no more cursors to be evaluated due to unmatches
    if self.cursors.is_empty() && !unmatches.is_empty() {
      return Err(if unmatches.len() == 1 { unmatches.remove(0) } else { Error::Multi(unmatches) });
    }

    // reduce internal event buffer by immediate delivering if only one cursor is present
    if self.cursors.len() == 1 {
      self.cursors[0].flush_events(&mut self.event_handler);
    }

    // reduce internal buffer if possible
    let mut min = self.buffer.len();
    for c in &self.cursors {
      min = std::cmp::min(min, c.buffer_begin);
    }
    if min > 0 {
      self.buffer.drain(0..min);
      self.offset_of_buffer_head += min as u64;
      for c in &mut self.cursors {
        c.buffer_begin -= min;
      }
    }

    Ok(())
  }

  pub fn finish(mut self) -> Result<()> {
    if self.cursors.is_empty() {
      return Err(Error::CantMatchAnymore);
    }

    let mut matches = Vec::with_capacity(self.cursors.len());
    let mut unmatches = Vec::with_capacity(self.cursors.len());
    for i in 0..self.cursors.len() {
      let c = &self.cursors[i];
      match c.last_result {
        MatchResult::Match | MatchResult::MatchAndCanAcceptMore => {
          let actual = self.debug_symbols_at_tail(5, false);
          if let Some(mut errors) = Self::to_error_not_matching_eof_in_all_the_rest(c, self.schema, &actual) {
            unmatches.append(&mut errors);
          } else {
            matches.push(i);
          }
        }
        MatchResult::UnmatchAndCanAcceptMore => {
          let expected = c.current_syntax().to_string();
          let actual = self.debug_symbols_at_tail(5, false);
          unmatches.push(Error::Unexpected(expected, actual));
        }
        MatchResult::Unmatch => panic!(),
      }
    }

    // if unmatch
    if matches.is_empty() {
      debug_assert_ne!(0, unmatches.len());
      return Error::errors(unmatches);
    }

    // if multiple matches are detected
    if matches.len() > 1 {
      return Err(Error::MultipleMatches());
    }

    // notify all remaining events and success
    self.cursors[matches.remove(0)].flush_events(&mut self.event_handler);

    Ok(())
  }

  fn to_error_not_matching_eof_in_all_the_rest(
    c: &State<E>, schema: &'s Schema<E>, actual: &str,
  ) -> Option<Vec<Error>> {
    let mut current = c.next_steps(schema);
    loop {
      let mut errors = Vec::with_capacity(current.len());
      let mut nexts = Vec::with_capacity(current.len());
      while !current.is_empty() {
        let c = current.remove(0);
        if !c.last_result.is_match() {
          let expected = c.current_syntax().to_string();
          errors.push(Error::Unexpected(expected, actual.to_string()));
        } else {
          nexts.push(c);
        }
      }
      if nexts.is_empty() {
        return if errors.is_empty() { None } else { Some(errors) };
      }
      current = nexts;
    }
  }

  fn debug_symbols_at_tail(&self, len: usize, ellipsis: bool) -> String {
    let begin = self.buffer.len() - std::cmp::min(len, self.buffer.len());
    let tail = E::debug_symbols_with_ellipsis(&self.buffer[begin..], ellipsis);
    if self.offset_of_buffer_head > 0 || self.buffer.len() > len {
      format!("...{}", tail)
    } else {
      tail
    }
  }
}

/// The `Cursor` advances step by step, evaluating [`Syntax`] matches.
///
struct State<'s, E: Item> {
  location: E::Location,
  /// All of elements must be `Syntax::Seq()`.
  call_stack: Vec<(&'s Syntax<E>, usize)>,
  buffer_begin: usize,
  last_result: MatchResult,
  repetition_count: usize,
  event_buffer: Vec<Event<E>>,
}

impl<'s, E: Item> State<'s, E> {
  pub fn new(syntax: &'s Syntax<E>) -> Self {
    State {
      call_stack: vec![(syntax, 0)],
      buffer_begin: 0,
      last_result: MatchResult::UnmatchAndCanAcceptMore,
      location: E::Location::default(),
      repetition_count: 0,
      event_buffer: Vec::new(),
    }
  }

  pub fn current_syntax(&self) -> &'s Syntax<E> {
    let s = self.call_stack.last().unwrap();
    match &s.0.primary {
      Primary::Seq(sequence) => &sequence[s.1],
      _ => panic!(),
    }
  }

  pub fn next_steps(&self, _schema: &'s Schema<E>) -> Vec<State<'s, E>> {
    todo!()
  }

  pub fn flush_events<H: FnMut(Event<E>)>(&mut self, handler: &mut H) {
    while !self.event_buffer.is_empty() {
      (handler)(self.event_buffer.remove(0));
    }
  }
}
