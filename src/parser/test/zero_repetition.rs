use crate::parser::test::{assert_unmatch, location, Events};
use crate::parser::{Context, Event};
use crate::schema::chars::{ascii_alphabetic, ascii_digit};
use crate::schema::Schema;

#[test]
fn context_zero_repetition_option() {
  let a = ascii_digit() * (0..=1);
  let schema = Schema::new("Foo").define("A", a);

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let parser = Context::new(&schema, "A", handler).unwrap();
  parser.finish().unwrap();
  Events::new().begin("A").end().assert_eq(&events);

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('7').unwrap();
  parser.finish().unwrap();
  Events::new().begin("A").fragments("7").end().assert_eq(&events);

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  assert_unmatch(parser.push('X'), location(0, 0, 0), "[ASCII_DIGIT?]", "['X']");
}

#[test]
fn context_zero_repetition_precedes() {
  let a = (ascii_digit() * (0..=1)) & ascii_alphabetic();
  let schema = Schema::new("Foo").define("A", a);

  for chars in &["X", "9X"] {
    let mut events = Vec::new();
    let handler = |e: Event<_, _>| events.push(e);
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    for ch in chars.chars() {
      parser.push(ch).unwrap();
    }
    parser.finish().unwrap();
    Events::new().begin("A").fragments(chars).end().assert_eq(&events);
  }

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('9').unwrap();
  assert_unmatch(parser.finish(), location(1, 0, 1), "9[ASCII_ALPHA]", "9[EOF]");

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('9').unwrap();
  assert_unmatch(parser.push('8'), location(1, 0, 1), "9[ASCII_ALPHA]", "9['8']");
}

#[test]
fn context_zero_repetition_trailing() {
  let a = ascii_alphabetic() & (ascii_digit() * (0..=1));
  let schema = Schema::new("Foo").define("A", a);

  for chars in &["X", "X9"] {
    let mut events = Vec::new();
    let handler = |e: Event<_, _>| events.push(e);
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    for ch in chars.chars() {
      parser.push(ch).unwrap();
    }
    parser.finish().unwrap();
    Events::new().begin("A").fragments(chars).end().assert_eq(&events);
  }

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  assert_unmatch(parser.push('9'), location(0, 0, 0), "[ASCII_ALPHA]", "['9']");

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('X').unwrap();
  assert_unmatch(parser.push('Y'), location(1, 0, 1), "X[ASCII_DIGIT?]", "X['Y']");
}

#[test]
fn context_zero_repetition_sequence() {
  let a = (ascii_digit() * (0..=1)) & (ascii_alphabetic() * (0..=1));
  let schema = Schema::new("Foo").define("A", a);

  for chars in &["", "9", "Z", "9Z"] {
    let mut events = Vec::new();
    let handler = |e: Event<_, _>| events.push(e);
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    for ch in chars.chars() {
      parser.push(ch).unwrap();
    }
    parser.finish().unwrap();
    Events::new().begin("A").fragments(chars).end().assert_eq(&events);
  }

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  assert_unmatch(parser.push('!'), location(0, 0, 0), "[ASCII_ALPHA?]", "['!']");

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('9').unwrap();
  assert_unmatch(parser.push('!'), location(1, 0, 1), "9[ASCII_ALPHA?]", "9['!']");

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('Z').unwrap();
  assert_unmatch(parser.push('!'), location(1, 0, 1), "Z[EOF]", "Z['!']");
}

#[test]
fn context_zero_repetition_injected() {
  let a = ascii_alphabetic() & (ascii_digit() * (0..=1)) & ascii_alphabetic();
  let schema = Schema::new("Foo").define("A", a);

  for chars in &["AB", "A0B"] {
    let mut events = Vec::new();
    let handler = |e: Event<_, _>| events.push(e);
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    for ch in chars.chars() {
      parser.push(ch).unwrap();
    }
    parser.finish().unwrap();
    Events::new().begin("A").fragments(chars).end().assert_eq(&events);
  }

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('A').unwrap();
  assert_unmatch(parser.push('!'), location(1, 0, 1), "A[ASCII_ALPHA]", "A['!']");

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('A').unwrap();
  parser.push('0').unwrap();
  assert_unmatch(parser.push('!'), location(2, 0, 2), "A0[ASCII_ALPHA]", "A0['!']");

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('A').unwrap();
  parser.push('0').unwrap();
  assert_unmatch(parser.push('1'), location(2, 0, 2), "A0[ASCII_ALPHA]", "A0['1']");
}

#[test]
fn context_zero_repetition_caught_between() {
  let a = (ascii_digit() * (0..=1)) & ascii_alphabetic() & (ascii_digit() * (0..=1));
  let schema = Schema::new("Foo").define("A", a);

  for chars in &["A", "0A", "A1", "0A1"] {
    let mut events = Vec::new();
    let handler = |e: Event<_, _>| events.push(e);
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    for ch in chars.chars() {
      parser.push(ch).unwrap();
    }
    parser.finish().unwrap();
    Events::new().begin("A").fragments(chars).end().assert_eq(&events);
  }

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let parser = Context::new(&schema, "A", handler).unwrap();
  assert_unmatch(parser.finish(), location(0, 0, 0), "[ASCII_ALPHA]", "[EOF]");

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  assert_unmatch(parser.push('!'), location(0, 0, 0), "[ASCII_ALPHA]", "['!']");

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  assert_unmatch(parser.finish(), location(1, 0, 1), "0[ASCII_ALPHA]", "0[EOF]");

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  assert_unmatch(parser.push('1'), location(1, 0, 1), "0[ASCII_ALPHA]", "0['1']");

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  assert_unmatch(parser.push('!'), location(1, 0, 1), "0[ASCII_ALPHA]", "0['!']");

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  assert_unmatch(parser.push('1'), location(1, 0, 1), "0[ASCII_ALPHA]", "0['1']");

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  parser.push('A').unwrap();
  assert_unmatch(parser.push('B'), location(2, 0, 2), "0A[ASCII_DIGIT?]", "0A['B']");

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  parser.push('A').unwrap();
  assert_unmatch(parser.push('!'), location(2, 0, 2), "0A[ASCII_DIGIT?]", "0A['!']");

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  parser.push('A').unwrap();
  parser.push('1').unwrap();
  assert_unmatch(parser.push('2'), location(3, 0, 3), "0A1[EOF]", "0A1['2']");

  let mut events = Vec::new();
  let handler = |e: Event<_, _>| events.push(e);
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  parser.push('A').unwrap();
  parser.push('1').unwrap();
  assert_unmatch(parser.push('!'), location(3, 0, 3), "0A1[EOF]", "0A1['!']");
}
