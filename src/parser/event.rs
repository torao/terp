use std::{
  collections::HashSet,
  fmt::{Debug, Display},
  hash::Hash,
};

use crate::schema::Symbol;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Event<ID, Σ: Symbol>
where
  ID: Clone + Display + Debug + PartialEq + Eq + Hash,
{
  pub location: Σ::Location,
  pub kind: EventKind<ID, Σ>,
}

impl<ID, Σ: Symbol> Event<ID, Σ>
where
  ID: Clone + Display + Debug + PartialEq + Eq + Hash,
{
  pub fn normalize(events: &[Event<ID, Σ>]) -> Vec<Event<ID, Σ>> {
    let mut buffer = EventBuffer::new(events.len());
    for e in events {
      buffer.push(e.clone());
    }
    buffer.events
  }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum EventKind<ID, Σ: Symbol>
where
  ID: Clone + Debug,
{
  Begin(ID),
  End(ID),
  Fragments(Vec<Σ>),
}

#[derive(Clone, Debug)]
pub(crate) struct EventBuffer<ID, Σ: Symbol>
where
  ID: Clone + Display + Debug + PartialEq + Eq + Hash,
{
  events: Vec<Event<ID, Σ>>,
  ignore: HashSet<ID>,

  // to verify Begin/End conbinations
  #[cfg(debug_assertions)]
  _event_stack: Vec<ID>,
}

impl<ID, Σ: Symbol> EventBuffer<ID, Σ>
where
  ID: Clone + Display + Debug + PartialEq + Eq + Hash,
{
  pub fn new(capacity: usize) -> Self {
    Self {
      events: Vec::with_capacity(capacity),
      ignore: HashSet::new(),
      #[cfg(debug_assertions)]
      _event_stack: Vec::with_capacity(16),
    }
  }

  pub fn len(&self) -> usize {
    self.events.len()
  }

  pub fn ignore_events_for(&mut self, ids: &[ID]) {
    for id in ids {
      self.ignore.insert(id.clone());
    }
  }

  pub fn push(&mut self, mut e: Event<ID, Σ>) {
    match (&mut e, self.events.last_mut()) {
      (Event { kind: EventKind::Fragments(items), .. }, Some(Event { kind: EventKind::Fragments(current), .. })) => {
        // append items to buffer tail Fragment's sequence
        current.append(items);
      }
      (Event { kind: EventKind::End(i1), .. }, Some(Event { kind: EventKind::Begin(i2), .. })) if i1 == i2 => {
        #[cfg(debug_assertions)]
        debug_assert_eq!(self._event_stack.pop().unwrap(), *i2);

        // delete buffer tail for Begin/End with no content
        self.events.pop();
      }
      _ => {
        #[cfg(debug_assertions)]
        match &e {
          Event { kind: EventKind::Begin(id), .. } => self._event_stack.push(id.clone()),
          Event { kind: EventKind::End(actual), .. } => match self._event_stack.pop() {
            Some(expected) if *actual == expected => (),
            Some(expected) => {
              panic!("inconsisnt event is detected: End({}) expected, but End({}) appeared", expected, actual)
            }
            None => panic!("inconsist event is detected: End({}) appeared on empty stack", actual),
          },
          _ => (),
        }

        match &e {
          Event { kind: EventKind::Begin(id), .. } if self.ignore.contains(id) => (),
          Event { kind: EventKind::End(id), .. } if self.ignore.contains(id) => (),
          _ => self.events.push(e),
        }
      }
    }
  }

  pub fn normalize(mut self) -> Self {
    self.events = Event::normalize(&self.events);
    self.events.shrink_to_fit();
    self
  }

  pub fn flush_to<H: FnMut(Event<ID, Σ>)>(&mut self, n: usize, handler: &mut H) {
    for e in self.events.drain(..n) {
      (handler)(e);
    }
  }

  pub fn forward_matching_length(&self, other: &Self) -> usize {
    let len = std::cmp::min(self.events.len(), other.events.len());
    for i in 0..len {
      if self.events[i] != other.events[i] {
        return i;
      }
    }
    len
  }
}

impl<ID, Σ: Symbol> PartialEq for EventBuffer<ID, Σ>
where
  ID: Clone + Display + Debug + PartialEq + Eq + Hash,
{
  fn eq(&self, other: &Self) -> bool {
    if self.events.len() != other.events.len() {
      false
    } else {
      for i in (0..self.events.len()).rev() {
        if self.events[i] != other.events[i] {
          return false;
        }
      }
      true
    }
  }
}
