use std::fmt::{Debug, Display};

use crate::schema::Item;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event<ID, E: Item>
where
  ID: Clone + Display + Debug + PartialEq,
{
  pub location: E::Location,
  pub kind: EventKind<ID, E>,
}

impl<ID, E: Item> Event<ID, E> where ID: Clone + Display + Debug + PartialEq {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind<ID, E: Item>
where
  ID: Clone + Debug,
{
  Begin(ID),
  End(ID),
  Fragments(Vec<E>),
}

#[derive(Clone, Debug)]
pub(crate) struct EventBuffer<ID, E: Item>
where
  ID: Clone + Display + Debug + PartialEq,
{
  events: Vec<Event<ID, E>>,

  // to verify Begin/End conbinations
  #[cfg(test)]
  _event_stack: Vec<ID>,
}

impl<ID, E: Item> EventBuffer<ID, E>
where
  ID: Clone + Display + Debug + PartialEq,
{
  pub fn new(capacity: usize) -> Self {
    Self {
      events: Vec::with_capacity(capacity),
      #[cfg(test)]
      _event_stack: Vec::with_capacity(16),
    }
  }

  pub fn push(&mut self, mut e: Event<ID, E>) {
    match (&mut e, self.events.last_mut()) {
      (Event { kind: EventKind::Fragments(items), .. }, Some(Event { kind: EventKind::Fragments(current), .. })) => {
        // append items to buffer tail Fragment's sequence
        current.append(items);
      }
      (Event { kind: EventKind::End(i1), .. }, Some(Event { kind: EventKind::Begin(i2), .. })) if i1 == i2 => {
        #[cfg(test)]
        debug_assert_eq!(self._event_stack.pop().unwrap(), *i2);

        // delete buffer tail for Begin/End with no content
        self.events.pop();
      }
      _ => {
        #[cfg(test)]
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

        self.events.push(e);
      }
    }
  }

  pub fn normalize(mut self) -> Self {
    for e in self.events.drain(..).collect::<Vec<_>>() {
      self.events.push(e);
    }
    self.events.shrink_to_fit();
    self
  }

  pub fn flush_to<H: FnMut(Event<ID, E>)>(&mut self, handler: &mut H) {
    for e in self.events.drain(..) {
      (handler)(e);
    }
  }
}

impl<ID, E: Item> PartialEq for EventBuffer<ID, E>
where
  ID: Clone + Display + Debug + PartialEq,
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
