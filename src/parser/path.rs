use crate::parser::{Event, EventBuffer, EventKind};
use crate::schema::{Location, MatchResult, Primary, Schema, Symbol, Syntax};
use crate::{debug, Error, Result};
use std::fmt::{Debug, Display, Write};
use std::hash::Hash;

#[derive(Clone, Debug)]
pub(crate) struct Path<'s, ID, Σ: Symbol>
where
  ID: Clone + Display + Debug + PartialEq + Eq + Hash,
{
  schema: &'s Schema<ID, Σ>,
  event_buffer: EventBuffer<ID, Σ>,
  stack: Vec<StackFrame<'s, ID, Σ>>,

  // For variable watch during step execution.
  #[cfg(debug_assertions)]
  _debug: String,
  #[cfg(debug_assertions)]
  _eval: String,
}

impl<'s, ID, Σ: Symbol> Path<'s, ID, Σ>
where
  ID: Clone + Hash + Ord + Display + Debug,
{
  pub fn new(id: &ID, schema: &'s Schema<ID, Σ>) -> Result<Σ, Self> {
    let event_buffer = EventBuffer::new(16);
    let stack = Vec::with_capacity(16);

    let mut path = Self {
      schema,
      event_buffer,
      stack,
      #[cfg(debug_assertions)]
      _debug: String::from(""),
      #[cfg(debug_assertions)]
      _eval: String::from(""),
    };
    path.stack_push_alias(id)?;
    Ok(path)
  }

  pub fn current(&self) -> &State<'s, ID, Σ> {
    &self.stack.last().unwrap().state
  }

  pub fn current_mut(&mut self) -> &mut State<'s, ID, Σ> {
    &mut self.stack.last_mut().unwrap().state
  }

  pub fn event_buffer(&self) -> &EventBuffer<ID, Σ> {
    &self.event_buffer
  }

  pub fn event_buffer_mut(&mut self) -> &mut EventBuffer<ID, Σ> {
    &mut self.event_buffer
  }

  /// return false if the end of reached.
  /// returns (matched, confirmed), where matched=true, it needs to move to term and continue
  /// processing, and confirmed=true
  /// Note that if called by matched=false, it may be overriden by matched=true at the upper layer
  /// of the stack.
  ///
  pub fn move_to_next(&mut self, buffer: &[Σ], mut matched: bool, eof: bool) -> (bool, bool) {
    for i in 0..self.stack.len() {
      let stack_position = self.stack.len() - i - 1;
      let StackFrame { state, current, parent, _debug } = &mut self.stack[stack_position];
      debug_assert!(state.appearances <= *state.syntax().repetition.end());

      if matched && state.appearances < *state.syntax().repetition.end() {
        state.appearances += 1;
      }

      matched = match (matched, eof) {
        (true, true) => state.appearances >= *state.syntax().repetition.start(),
        (true, false) => {
          if state.appearances < *state.syntax().repetition.end() {
            debug!("~ repeated: {} / {}", state.syntax(), state.appearances);
            state.proceed_along_buffer(buffer);
            self.stack_pop(i);
            self.complete_eval_of_current_position(false);
            return (true, false);
          }
          debug_assert_eq!(state.appearances, *state.syntax().repetition.end());
          true
        }
        (false, _) => state.appearances >= *state.syntax.repetition.start(),
      };

      if matched {
        state.proceed_along_buffer(buffer);
        if *current + 1 < parent.len() {
          self.stack_pop(i);
          self.complete_eval_of_current_position(true);
          return (true, false);
        }
      }
    }

    debug!("~ confirmed: {} ({})", self.current().syntax(), if matched { "Matched" } else { "Unmatched" });
    (matched, true)
  }

  #[inline]
  pub fn matches(&mut self, buffer: &[Σ], eof: bool) -> Result<Σ, Matching<ID, Σ>> {
    let result = self.current_mut().matches(buffer, eof);
    #[cfg(debug_assertions)]
    {
      self._eval = format!(
        "{}(\"{}\") => {:?}",
        self.current().syntax(),
        Σ::debug_symbols(
          &buffer[self.current().match_begin..std::cmp::min(buffer.len(), self.current().match_begin + 8)]
        ),
        result.as_ref().ok().map(|r| format!("{:?}", r)).unwrap_or_else(|| String::from("ERR"))
      );
    }
    result
  }

  pub fn completed(&mut self) {
    self.stack_pop(self.stack.len() - 1);
    debug_assert!(self.stack.len() == 1);
    debug_assert!(self.stack[0].current + 1 == self.stack[0].parent.len());

    self.complete_eval_of_current_position(false);
    debug_assert!(self.stack[0].current + 1 == self.stack[0].parent.len());
  }

  pub fn can_merge(&self, other: &Path<'s, ID, Σ>) -> bool {
    // points the same syntax
    debug_assert_eq!(self.stack[0].parent.len(), other.stack[0].parent.len()); // their root must be same
    if self.stack.len() != other.stack.len() {
      return false;
    }
    for i in (0..self.stack.len()).rev() {
      if self.stack[i].state.syntax().id != other.stack[i].state.syntax().id
        || self.stack[i].state.appearances != other.stack[i].state.appearances
        || self.stack[i].state.location != other.stack[i].state.location
      {
        return false;
      }
    }

    // holds the same events
    debug_assert_eq!(self.event_buffer.clone().normalize(), self.event_buffer);
    self.event_buffer == other.event_buffer
  }

  pub fn stack_push_alias(&mut self, id: &ID) -> Result<Σ, ()> {
    debug!("~ begined: {}", id);
    self.stack_push(Self::get_definition(id, self.schema)?);
    Ok(())
  }

  pub fn stack_push(&mut self, seq: &'s Vec<Syntax<ID, Σ>>) {
    let mut sf = StackFrame::new(seq);
    if !self.stack.is_empty() {
      sf.state.location = self.current().location;
      sf.state.match_begin = self.current().match_begin;
    }
    self.stack.push(sf);
    #[cfg(debug_assertions)]
    {
      self._debug = self.to_string();
    }
  }

  fn stack_pop(&mut self, count: usize) {
    for _ in 0..count {
      // The current of stack frame to be discarding may not point to the end of the stack frame if it was interpreted
      // by unmatch but matched at the upper layer.
      // let StackFrame { state, parent, current } = self.stack.pop().unwrap();
      // debug_assert!(current + 1 == parent.len());
      self.complete_eval_of_current_position(false);

      let StackFrame { state, .. } = self.stack.pop().unwrap();
      self.current_mut().match_begin = state.match_begin;
      self.current_mut().location = state.location;
    }
    #[cfg(debug_assertions)]
    {
      self._debug = self.to_string();
    }
  }

  fn complete_eval_of_current_position(&mut self, move_next: bool) {
    let StackFrame { state, current, parent, _debug } = self.stack.last_mut().unwrap();
    let event = if let Primary::Alias(id) = &parent[*current].primary {
      debug!("~ ended: {}", id);
      Some(state.event(EventKind::End(id.clone())))
    } else {
      None
    };

    if move_next {
      debug!("~ moved: {} -> {}", parent[*current], parent[*current + 1]);
      *current += 1;
      state.syntax = &parent[*current];
      state.appearances = 0;
    }
    if let Some(e) = event {
      self.events_push(e);
    }
  }

  pub fn events_push(&mut self, e: Event<ID, Σ>) {
    self.event_buffer.push(e)
  }

  pub fn events_flush_all_to<H: FnMut(&Event<ID, Σ>)>(&mut self, handler: &mut H) {
    self.events_flush_forward_to(self.event_buffer.len(), handler)
  }

  pub fn events_flush_forward_to<H: FnMut(&Event<ID, Σ>)>(&mut self, n: usize, handler: &mut H) {
    self.event_buffer.flush_to(n, handler)
  }

  pub fn events_forward_matching_length(&self, other: &Self) -> usize {
    self.event_buffer().forward_matching_length(other.event_buffer())
  }

  pub fn min_match_begin(&self) -> usize {
    self.stack.iter().map(|sf| sf.state.match_begin).min().unwrap()
  }

  pub fn on_buffer_shrunk(&mut self, amount: usize) {
    for sf in &mut self.stack {
      sf.state.match_begin -= amount;
    }
  }

  fn get_definition(id: &ID, schema: &'s Schema<ID, Σ>) -> Result<Σ, &'s Vec<Syntax<ID, Σ>>> {
    if let Some(Syntax { primary: Primary::Seq(seq), repetition, .. }) = schema.get(id) {
      debug_assert!(!seq.is_empty());
      debug_assert!(*repetition.start() == 1 && *repetition.end() == 1);
      Ok(seq)
    } else {
      Err(Error::UndefinedID(id.to_string()))
    }
  }
}

impl<'s, ID, Σ: Symbol> Display for Path<'s, ID, Σ>
where
  ID: Clone + Hash + Ord + Display + Debug,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    for (i, StackFrame { parent, current, .. }) in self.stack.iter().enumerate() {
      if i != 0 {
        f.write_str(">>")?;
      }
      f.write_char('[')?;
      Display::fmt(&parent[*current], f)?;
      f.write_char(']')?;
    }
    Ok(())
  }
}

#[derive(Clone, Debug)]
struct StackFrame<'s, ID, Σ: Symbol>
where
  ID: Clone + Display + Debug,
{
  state: State<'s, ID, Σ>,
  parent: &'s Vec<Syntax<ID, Σ>>,
  current: usize,

  _debug: String,
}

impl<'s, ID, Σ: Symbol> StackFrame<'s, ID, Σ>
where
  ID: Clone + Hash + Ord + Display + Debug,
{
  pub fn new(parent: &'s Vec<Syntax<ID, Σ>>) -> Self {
    debug_assert!(!parent.is_empty());
    let state = State::new(&parent[0]);
    Self { state, parent, current: 0, _debug: format!("{}", parent[0]) }
  }
}

/// The `Cursor` advances step by step, evaluating [`Syntax`] matches.
///
#[derive(Clone, Debug)]
pub struct State<'s, ID, Σ: Symbol>
where
  ID: Clone + Display + Debug,
{
  pub location: Σ::Location,
  pub match_begin: usize,
  pub match_length: usize,
  pub appearances: usize,

  /// The [`Syntax`] must be `Syntax::Seq`.
  syntax: &'s Syntax<ID, Σ>,
}

impl<'s, ID, Σ: 'static + Symbol> State<'s, ID, Σ>
where
  ID: Clone + Display + Debug + PartialEq + Eq + Hash,
{
  pub fn new(syntax: &'s Syntax<ID, Σ>) -> Self {
    Self { location: Σ::Location::default(), match_begin: 0, match_length: 0, appearances: 0, syntax }
  }

  pub fn syntax(&self) -> &'s Syntax<ID, Σ> {
    self.syntax
  }

  fn matches(&mut self, buffer: &[Σ], eof: bool) -> Result<Σ, Matching<ID, Σ>> {
    debug_assert!(buffer.len() >= self.match_begin + self.match_length);

    let items = &buffer[self.match_begin..];
    let reps = &self.syntax.repetition;
    debug_assert!(self.appearances <= *reps.end());
    if !self.can_repeate_more() {
      debug!("~ matched: {}({}) -> no data", self.syntax(), Σ::debug_symbols(items));
      return Ok(Matching::Match(0, None));
    }

    let matcher = if let Primary::Term(_, matcher) = &self.syntax.primary {
      matcher
    } else {
      unreachable!("Current syntax is not Primary::Term(matcher): {:?}", self.syntax)
    };

    let result = match matcher(items)? {
      MatchResult::UnmatchAndCanAcceptMore if eof => MatchResult::Unmatch,
      MatchResult::MatchAndCanAcceptMore(length) if eof => MatchResult::Match(length),
      result => result,
    };

    let result = match result {
      MatchResult::Match(length) => {
        self.match_length = length;
        let values = self.extract(buffer).to_vec();
        debug!("~ matched: {}({}) -> [{}]", self.syntax(), Σ::debug_symbols(items), Σ::debug_symbols(&values));
        Matching::Match(length, Some(self.event(EventKind::Fragments(values))))
      }
      MatchResult::Unmatch => {
        debug!("~ unmatched: {}({})", self.syntax(), Σ::debug_symbols(items));
        Matching::Unmatch
      }
      MatchResult::MatchAndCanAcceptMore(_) | MatchResult::UnmatchAndCanAcceptMore => Matching::More,
    };

    Ok(result)
  }

  pub fn can_repeate_more(&self) -> bool {
    if self.appearances == *self.syntax.repetition.end() {
      false
    } else {
      debug_assert!(self.appearances < *self.syntax.repetition.end());
      true
    }
  }

  fn proceed_along_buffer(&mut self, buffer: &[Σ]) {
    if self.match_length > 0 {
      self.location.increment_with_seq(self.extract(buffer));
      self.match_begin += self.match_length;
      self.match_length = 0;
    }
  }

  pub fn extract<'a>(&self, buffer: &'a [Σ]) -> &'a [Σ] {
    &buffer[self.match_begin..][..self.match_length]
  }

  pub fn event(&self, kind: EventKind<ID, Σ>) -> Event<ID, Σ> {
    Event { location: self.location, kind }
  }
}

#[derive(Debug)]
pub enum Matching<ID, Σ: Symbol>
where
  ID: Clone + Display + Debug + PartialEq + Eq + Hash,
{
  Match(usize, Option<Event<ID, Σ>>),
  More,
  Unmatch,
}
