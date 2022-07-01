use crate::parser::{Context, Event, EventKind};
use crate::schema::chars::{self, ascii_alphabetic, ascii_digit, Location};
use crate::schema::{Item, Schema, Syntax};
use crate::{Error, Result};
use std::fmt::{Debug, Display};
use std::hash::Hash;

#[test]
fn event() {
  let location = Location::default();
  for kind in
    vec![EventKind::Begin("FOO"), EventKind::End("BAR"), EventKind::Fragments("XYZ".chars().collect::<Vec<_>>())]
  {
    let event = Event { location, kind };
    assert_eq!(event, event.clone());
    let _ = format!("{:?}", event);
  }
}

#[test]
fn context_with_enum_id() {
  #[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
  enum X {
    A,
    B,
  }
  impl Display for X {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      Debug::fmt(self, f)
    }
  }
  let schema =
    Schema::new("Foo").define(X::A, ascii_digit() * 5).define(X::B, ascii_alphabetic() & Syntax::from_id(X::A));
  let mut events = Vec::new();
  let handler = |e: Event<_, _>| {
    events.push(e);
  };
  let _ = Context::new(&schema, X::B, handler);
}

#[test]
fn context_for_signle_def_single_term() {
  let schema = Schema::new("Foo").define("A", ascii_digit() * 3);

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| {
    println!("  RECEIVED: {:?}", e);
    events.push(e);
  };
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  parser.push('1').unwrap();
  parser.push('2').unwrap();
  parser.finish().unwrap();
  println!("{:?}", events);
}

#[test]
fn context_eof_expected_but_valud_value_arrived() {
  let a = ascii_digit() * 3;
  let schema = Schema::new("Foo").define("A", a);

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  parser.push('1').unwrap();
  parser.push('2').unwrap();
  assert_unmatch(parser.push('3'), location(3, 0, 3), "012[EOF]", "012['3']");
  assert_unmatch(parser.push('4'), location(3, 0, 3), "012[EOF]", "012['4']");
  parser.finish().unwrap();
  println!("{:?}", events);
}

#[test]
fn context_valid_value_expected_but_eof_detected() {
  let a = ascii_digit() * 3;
  let schema = Schema::new("Foo").define("A", a);

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  parser.push('1').unwrap();
  assert_unmatch(parser.finish(), location(2, 0, 2), "01[ASCII_DIGIT{3}]", "01[EOF]");
}

#[test]
fn context_multiple_match() {
  let a = (ascii_digit() * 3) | (ascii_digit() * (3..=4));
  let schema = Schema::new("Foo").define("A", a);

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  parser.push('1').unwrap();
  parser.push('2').unwrap();
  match parser.finish() {
    Err(Error::MultipleMatches { location: l, expecteds, actual }) => {
      assert_eq!(location(3, 0, 3), l);
      assert_eq_without_order(vec!["012[ASCII_DIGIT{3}]", "012[ASCII_DIGIT{3,4}]"], expecteds);
      assert_eq!("012[EOF]", actual);
    }
    unexpected => panic!("{:?}", unexpected),
  }
}

#[test]
fn context_match_within_repetition_range() {
  let a = ascii_digit() * (1..=3);
  let schema = Schema::new("Foo").define("A", a);

  for digits in &["0", "01", "012"] {
    let mut events = Vec::new();
    let handler = |e: Event<_, _>| events.push(e);
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    for ch in digits.chars() {
      parser.push(ch).unwrap();
    }
    parser.finish().unwrap();
    println!("{:?}", events);
  }

  // if less than the repetition range
  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let parser = Context::new(&schema, "A", handler).unwrap();
  assert_unmatch(parser.finish(), location(0, 0, 0), "[ASCII_DIGIT{1,3}]", "[EOF]");

  // if the repetition range is exceeded
  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  parser.push('1').unwrap();
  parser.push('2').unwrap();
  assert_unmatch(parser.push('3'), location(3, 0, 3), "012[EOF]", "012['3']");
}

#[test]
fn context_match_following_match_within_repetition_range() {
  let a = (ascii_digit() * (1..=3)) & ascii_alphabetic();
  let schema = Schema::new("Foo").define("A", a);

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  assert_unmatch(parser.push('X'), location(1, 0, 1), "[ASCII_DIGIT{1,3}]", "['X']");

  for digits in &["0", "01", "012"] {
    let mut events = Vec::new();
    let handler = |e: Event<_, _>| events.push(e);
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    for ch in digits.chars() {
      parser.push(ch).unwrap();
    }
    parser.push('X').unwrap();
    parser.finish().unwrap();
    println!("{:?}", events);
  }
}

#[test]
fn context_repetition_for_sequence() {
  let a = (((ascii_alphabetic() & ascii_digit()) * 2) & ((ascii_digit() & ascii_digit()) * 3)) * (1..=2);
  println!("{}", a);
  println!("{:?}", a);
  let schema = Schema::new("Foo").define("A", a);

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  for ch in "A0B1234567X8Y9012345".chars() {
    parser.push(ch).unwrap();
  }
  parser.finish().unwrap();
}

#[test]
fn context() {
  #[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
  enum X {
    A,
    B,
  }
  impl Display for X {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      Debug::fmt(self, f)
    }
  }
  let schema =
    Schema::new("Foo").define(X::A, ascii_digit() * 5).define(X::B, ascii_alphabetic() & Syntax::from_id(X::A));
  println!("{}", schema);

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| {
    println!("  RECEIVED: {:?}", e);
    events.push(e);
  };
  let mut parser = Context::new(&schema, X::B, handler).unwrap();
  assert_eq!(X::B, *parser.id());
  parser.push('E').unwrap();
  parser.push('0').unwrap();
  parser.push('1').unwrap();
  parser.push('2').unwrap();
  parser.push('3').unwrap();
  parser.push('4').unwrap();
  parser.finish().unwrap();
  println!("{:?}", events);
}

fn assert_unmatch<T: Debug>(r: Result<char, T>, l: chars::Location, e: &str, a: &str) {
  if let Err(Error::<char>::Unmatched { location, expected, actual }) = &r {
    assert_eq!((l, e, a), (*location, expected.as_str(), actual.as_str()));
  } else {
    panic!("Err(Error::Unmatched{{expected:{:?}, actual: {:?}}}) expected, but {:?}", e, a, r);
  }
}

fn assert_eq_without_order<T: Clone + Eq + Debug + From<U>, U>(expected: Vec<U>, mut actual: Vec<T>) {
  let mut expected = expected.into_iter().map(|x| x.into()).collect::<Vec<_>>();
  let e = expected.clone();
  let a = actual.clone();
  while let Some(expected) = expected.pop() {
    let i = actual.iter().position(|a| *a == expected);
    let i = i.unwrap_or_else(|| panic!("expected {:?}, but not exist in {:?}", expected, a));
    actual.remove(i);
  }
  assert!(actual.is_empty(), "expected {:?}, but {:?} exists", e, actual);
}

fn location(chars: u64, lines: u64, columns: u64) -> chars::Location {
  chars::Location { chars, lines, columns }
}

#[allow(dead_code)]
pub fn dump_context<ID, E: Item, H: FnMut(Event<ID, E>)>(parser: &Context<ID, E, H>)
where
  ID: Clone + Hash + Eq + Ord + Display + Debug,
{
  println!("Parser[{}]", parser.id);
  println!("  Location: {}", parser.location);
  println!("  Offset  : {}", parser.offset_of_buffer_head);
  println!("  Buffer  : {}", E::debug_symbols(&parser.buffer));
  print!("  Ongoing :");
  if !parser.ongoing.is_empty() {
    println!();
    for s in parser.ongoing.iter().map(|p| p.current()) {
      println!("    {:?}", s);
    }
  } else {
    println!(" none");
  }
  print!("  Completed:");
  if !parser.prev_completed.is_empty() {
    println!();
    for s in parser.prev_completed.iter().map(|p| p.current()) {
      println!("    {:?}", s);
    }
  } else {
    println!(" none");
  }
  print!("  Unmatched:");
  if !parser.prev_unmatched.is_empty() {
    println!();
    for s in parser.prev_unmatched.iter().map(|p| p.current()) {
      println!("    {:?}", s);
    }
  } else {
    println!(" none");
  }
}
