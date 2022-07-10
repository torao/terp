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

impl<ID, E: Item> Event<ID, E>
where
  ID: Clone + Display + Debug + PartialEq,
{
  pub fn append(events: &mut Vec<Event<ID, E>>, mut e: Event<ID, E>) {
    match (&mut e, events.last_mut()) {
      (Event { kind: EventKind::Fragments(items), .. }, Some(Event { kind: EventKind::Fragments(current), .. })) => {
        current.append(items);
      }
      (Event { kind: EventKind::End(i1), .. }, Some(Event { kind: EventKind::Begin(i2), .. })) if i1 == i2 => {
        events.pop();
      }
      _ => {
        events.push(e);
      }
    }
  }

  pub fn normalize(mut events: Vec<Event<ID, E>>) -> Vec<Event<ID, E>> {
    let mut norm = Vec::with_capacity(events.len());
    for e in events.drain(..) {
      Self::append(&mut norm, e);
    }
    norm.shrink_to_fit();
    norm
  }
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
