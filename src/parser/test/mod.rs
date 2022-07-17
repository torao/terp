use crate::parser::{error_unmatch_labels, Context, Event, EventBuffer, EventKind};
use crate::schema::chars::{self, ascii_alphabetic, ascii_digit, one_of_tokens, token};
use crate::schema::{Item, Location, Schema, Syntax};
use crate::{Error, Result};
use std::fmt::{Debug, Display};
use std::hash::Hash;

mod json;
mod or;
mod zero_repetition;

#[test]
fn event() {
  let location = chars::Location::default();
  for kind in
    vec![EventKind::Begin("FOO"), EventKind::End("BAR"), EventKind::Fragments("XYZ".chars().collect::<Vec<_>>())]
  {
    let event = Event { location, kind };
    assert_eq!(event, event.clone());
    let _ = format!("{:?}", event);
  }
}

#[test]
fn context_definition_not_found() {
  let schema = Schema::<&str, char>::new("Foo");

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  match Context::new(&schema, "A", handler) {
    Err(Error::UndefinedID(a)) => assert_eq!("A", a),
    Ok(_) => unreachable!(),
    Err(unexpected) => unreachable!("{}", unexpected),
  }
}

#[test]
fn context_for_signle_def_single_term() {
  let a = ascii_digit() * 3;
  let schema = Schema::new("Foo").define("A", a);

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  parser.push('1').unwrap();
  parser.push('2').unwrap();
  parser.finish().unwrap();
  Events::new().begin("A").fragments("012").end().assert_eq(&events);

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let parser = Context::new(&schema, "A", handler).unwrap();
  assert_unmatch(parser.finish(), location(0, 0, 0), "[ASCII_DIGIT{3}]", "[EOF]");
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
  // undefined behaviour, not guaranteed after unmatch confirmed
  parser.finish().unwrap();
  Events::new().begin("A").fragments("012").end().assert_eq(&events);
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

  for digits in ["0", "01", "012"] {
    let mut events = Vec::new();
    let handler = |e: Event<_, _>| events.push(e);
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    for ch in digits.chars() {
      parser.push(ch).unwrap();
    }
    parser.finish().unwrap();
    Events::new().begin("A").fragments(digits).end().assert_eq(&events);
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
  assert_unmatch(parser.push('X'), location(0, 0, 0), "[ASCII_DIGIT{1,3}]", "['X']");

  for digits in &["0", "01", "012"] {
    let mut events = Vec::new();
    let handler = |e: Event<_, _>| events.push(e);
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    for ch in digits.chars() {
      parser.push(ch).unwrap();
    }
    parser.push('X').unwrap();
    parser.finish().unwrap();
    Events::new().begin("A").fragments(digits).fragments("X").end().assert_eq(&events);
  }
}

#[test]
fn context_repetition_for_sequence() {
  let a = (((ascii_alphabetic() & ascii_digit()) * 2) & ((ascii_digit() & ascii_digit()) * 3)) * (1..=2);
  let schema = Schema::new("Foo").define("A", a);

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  for ch in "A0B1234567X8Y9012345".chars() {
    parser.push(ch).unwrap();
  }
  parser.finish().unwrap();
  Events::new().begin("A").fragments("A0B1234567X8Y9012345").end().assert_eq(&events);
}

#[test]
fn context_events_nested() {
  let a = ascii_digit() * 3;
  let b = ascii_alphabetic() & Syntax::from_id("A");
  let schema = Schema::new("Foo").define("A", a).define("B", b);

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "B", handler).unwrap();
  parser.push('E').unwrap();
  parser.push('0').unwrap();
  parser.push('1').unwrap();
  parser.push('2').unwrap();
  parser.finish().unwrap();
  Events::new().begin("B").fragments("E").begin("A").fragments("012").end().end().assert_eq(&events);
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
  let a = ascii_digit() * 5;
  let b = ascii_alphabetic() & Syntax::from_id(X::A);
  let schema = Schema::new("Foo").define(X::A, a).define(X::B, b);
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
  Events::new().begin(X::B).fragments("E").begin(X::A).fragments("01234").end().end().assert_eq(&events);
}

#[test]
fn context_error_unmatch_labels() {
  let buffer = (0..=100).map(|i| char::from_digit(i % 10, 10).unwrap()).collect::<Vec<_>>();

  for (buffer_length, buffer_offset, offset, lead, sample) in &[
    (0, 0, 0, "", ""),
    (1, 0, 0, "", "0"),
    (2, 0, 0, "", "01"),
    (27, 0, 0, "", "012345678901234567890123456"),
    (28, 0, 0, "", "0123456789012345...01234567"),
    (29, 0, 0, "", "0123456789012345...12345678"),
    (0, 1, 0, ".", ""),
    (1, 1, 0, ".", "0"),
    (27, 1, 0, ".", "012345678901234567890123456"),
    (28, 1, 0, ".", "0123456789012345...01234567"),
    (0, 2, 0, "..", ""),
    (1, 2, 0, "..", "0"),
    (27, 2, 0, "..", "012345678901234567890123456"),
    (28, 2, 0, "..", "0123456789012345...01234567"),
    (0, 3, 0, "...", ""),
    (1, 3, 0, "...", "0"),
    (27, 3, 0, "...", "012345678901234567890123456"),
    (28, 3, 0, "...", "0123456789012345...01234567"),
    (0, 4, 0, "...", ""),
    (1, 4, 0, "...", "0"),
    (27, 4, 0, "...", "012345678901234567890123456"),
    (28, 4, 0, "...", "0123456789012345...01234567"),
    (1, 0, 1, "0", ""),
    (27, 0, 1, "0", "12345678901234567890123456"),
    (28, 0, 1, "0", "123456789012345...01234567"),
    (1, 3, 1, "...0", ""),
    (27, 3, 1, "...0", "12345678901234567890123456"),
    (28, 3, 1, "...0", "123456789012345...01234567"),
    (11, 3, 11, "...34567890", ""),
    (12, 3, 11, "...34567890", "1"),
    (30, 3, 11, "...34567890", "1234567890123456789"),
    (31, 3, 11, "...34567890", "12345678...34567890"),
    (32, 3, 11, "...34567890", "12345678...45678901"),
  ] {
    let input = &buffer[..*buffer_length];
    let i_expected = Some((*offset, String::from("DIGIT*")));
    let i_actual = Some('X');
    let expected = format!("{}[{}]", lead, i_expected.as_ref().unwrap().1);
    let actual = format!("{}{}[{:?}]", lead, sample, i_actual.as_ref().unwrap());
    assert_eq!((expected, actual), error_unmatch_labels::<char>(input, *buffer_offset, i_expected, i_actual));
  }
}

#[test]
fn context_seq_keywords() {
  let keywords = [
    "Self", "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate", "do", "dyn",
    "else", "enum", "extern", "false", "final", "fn", "for", "if", "impl", "in", "let", "loop", "macro", "match",
    "mod", "move", "mut", "override", "priv", "pub", "ref", "return", "self", "static", "struct", "super", "trait",
    "true", "try", "type", "typeof", "union", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
  ];

  let a = keywords.iter().map(|kwd| token(*kwd)).reduce(|a, b| a | b).unwrap();
  let schema = Schema::new("Foo").define("A", a);
  for kwd in &keywords {
    eprintln!("[{}] ---------------------", kwd);
    let mut events = Vec::new();
    let handler = |e: Event<_, _>| {
      eprintln!("> Event[{}] {:?}", e.location, e.kind);
      events.push(e);
    };
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    parser.push_str(kwd).unwrap();
    parser.finish().unwrap();
    Events::new().begin("A").fragments(kwd).end().assert_eq(&events);

    let mut events = Vec::new();
    let handler = |e: Event<_, _>| events.push(e);
    let parser = Context::new(&schema, "A", handler).unwrap();
    let expecteds = keywords.iter().map(|kwd| format!("[{}]", kwd)).collect::<Vec<_>>();
    assert_multiple_unmatches(parser.finish(), location(0, 0, 0), &expecteds, "[EOF]");

    let mut events = Vec::new();
    let handler = |e: Event<_, _>| events.push(e);
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    let expecteds = keywords.iter().map(|kwd| format!("[{}]", kwd)).collect::<Vec<_>>();
    assert_multiple_unmatches(parser.push('X'), location(0, 0, 0), &expecteds, "['X']");

    let mut events = Vec::new();
    let handler = |e: Event<_, _>| events.push(e);
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    parser.push_str(kwd).unwrap();
    assert!(matches!(parser.push('X'), Err(Error::<char>::Unmatched { .. }))); // various errors
  }
}

#[test]
fn context_one_of_tokens() {
  let keywords = [
    "Self", "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate", "do", "dyn",
    "else", "enum", "extern", "false", "final", "fn", "for", "if", "impl", "in", "let", "loop", "macro", "match",
    "mod", "move", "mut", "override", "priv", "pub", "ref", "return", "self", "static", "struct", "super", "trait",
    "true", "try", "type", "typeof", "union", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
  ];

  let a = one_of_tokens(&keywords);
  let schema = Schema::new("Foo").define("A", a);
  for kwd in &keywords {
    let mut events = Vec::new();
    let handler = |e: Event<_, _>| events.push(e);
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    parser.push_str(kwd).unwrap();
    parser.finish().unwrap();
    Events::new().begin("A").fragments(kwd).end().assert_eq(&events);

    let mut events = Vec::new();
    let handler = |e: Event<_, _>| events.push(e);
    let parser = Context::new(&schema, "A", handler).unwrap();
    let expected = format!("[{}]", keywords.to_vec().join("|"));
    assert_unmatch(parser.finish(), location(0, 0, 0), &expected, "[EOF]");

    let mut events = Vec::new();
    let handler = |e: Event<_, _>| events.push(e);
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    let expected = format!("[{}]", keywords.to_vec().join("|"));
    assert_unmatch(parser.push('X'), location(0, 0, 0), &expected, "['X']");

    let mut events = Vec::new();
    let handler = |e: Event<_, _>| events.push(e);
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    parser.push_str(kwd).unwrap();
    assert!(matches!(parser.push('X'), Err(Error::<char>::Unmatched { .. }))); // various errors
  }
}

fn assert_unmatch<T: Debug>(r: Result<char, T>, l: chars::Location, e: &str, a: &str) {
  if let Err(Error::<char>::Unmatched { location, expected, actual }) = &r {
    assert_eq!((l, e, a), (*location, expected.as_str(), actual.as_str()));
  } else {
    panic!("Err(Error::Unmatched{{expected: {:?}, actual: {:?}}}) expected, but {:?}", e, a, r);
  }
}

fn assert_multiple_unmatches<T: Debug>(r: Result<char, T>, l: chars::Location, e: &[String], a: &str) {
  if let Err(Error::Multi(errs)) = r {
    let expecteds = errs
      .iter()
      .map(|err| {
        if let Error::Unmatched { location, expected, actual } = err {
          assert_eq!((&l, a), (location, actual.as_str()));
          expected.to_string()
        } else {
          panic!("{:?}", err);
        }
      })
      .collect::<Vec<_>>();
    assert_eq_without_order(e.to_vec(), expecteds);
  } else {
    panic!("{:?}", r);
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

fn assert_events_eq<ID: Clone + Display + Debug + Eq + Eq + Hash>(
  expected: &[Event<ID, char>], actual: &[Event<ID, char>],
) {
  let expected = normalize(expected);
  let actual = normalize(actual);
  let len = std::cmp::max(expected.len(), actual.len());
  for i in 0..len {
    assert_eq!(expected.get(i), actual.get(i), "unexpected event @{}:\n  {:?}\n  {:?}", i, expected, actual);
  }
}

fn normalize<ID: Clone + Display + Debug + Eq + Eq + Hash>(events: &[Event<ID, char>]) -> Vec<Event<ID, char>> {
  let mut buffer = EventBuffer::new(events.len());
  for e in events {
    buffer.push(e.clone());
  }
  let mut events = Vec::with_capacity(events.len());
  buffer.flush_to(&mut |e| events.push(e));
  events
}

fn location(chars: u64, lines: u64, columns: u64) -> chars::Location {
  chars::Location { chars, lines, columns }
}

pub struct Events<ID: Clone + Display + Debug + Eq + Eq + Hash> {
  location: chars::Location,
  events: Vec<Event<ID, char>>,
  stack: Vec<ID>,
}

impl<ID: Clone + Display + Debug + Eq + Eq + Hash> Events<ID> {
  pub fn new() -> Self {
    let events = Vec::with_capacity(16);
    let stack = Vec::with_capacity(4);
    Self { location: chars::Location::default(), events, stack }
  }
  pub fn begin(mut self, id: ID) -> Self {
    self.stack.push(id.clone());
    self.events.push(Event { location: self.location, kind: EventKind::Begin(id) });
    self
  }
  pub fn end(mut self) -> Self {
    let id = self.stack.pop().unwrap();
    self.events.push(Event { location: self.location, kind: EventKind::End(id) });
    self
  }
  pub fn fragments(mut self, text: &str) -> Self {
    for ch in text.chars() {
      self.events.push(Event { location: self.location, kind: EventKind::Fragments(vec![ch]) });
      self.location.increment_with(ch);
    }
    self
  }
  pub fn to_vec(&self) -> Vec<Event<ID, char>> {
    assert!(self.stack.is_empty(), "`end()` missing in expected events building: {:?}", self.stack);
    self.events.clone()
  }
  pub fn assert_eq(&self, actual: &[Event<ID, char>]) {
    assert_events_eq(&self.to_vec(), actual);
  }
}

impl<ID: Clone + Display + Debug + Eq + Eq + Hash> Default for Events<ID> {
  fn default() -> Self {
    Self::new()
  }
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
