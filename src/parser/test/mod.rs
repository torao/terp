use itertools::Itertools;

use crate::parser::{
  create_unmatched_label_actual, create_unmatched_label_prefix, Context, Event, EventBuffer, EventKind,
};
use crate::schema::chars::{self, ascii_alphabetic, ascii_digit, ch, one_of_chars, one_of_tokens, token};
use crate::schema::{id, Location, Schema, Syntax};
use crate::{Error, Result};
use std::fmt::{Debug, Display};
use std::hash::Hash;

mod context_free_grammer;
mod json;
mod or;
mod user_guide;
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
#[should_panic]
#[cfg(debug_assertions)]
fn event_buffer_inconsist_begin_end() {
  let location = chars::Location::default();
  let mut events = EventBuffer::new(1);
  for kind in
    vec![EventKind::Begin("FOO"), EventKind::Fragments("XYZ".chars().collect::<Vec<_>>()), EventKind::End("BAR")]
  {
    let event = Event { location, kind };
    events.push(event);
  }
}

#[test]
fn event_buffer_equivalence_when_diffferent_event() {
  let location1 = chars::Location::default();
  let mut location2 = chars::Location::default();
  location2.increment_with('\n');
  let mut events1 = EventBuffer::new(1);
  let mut events2 = EventBuffer::new(1);
  for kind in
    vec![EventKind::Begin("FOO"), EventKind::Fragments("XYZ".chars().collect::<Vec<_>>()), EventKind::End("FOO")]
  {
    events1.push(Event { location: location1, kind: kind.clone() });
    events2.push(Event { location: location2, kind: kind.clone() });
  }
  assert_ne!(events1, events2);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic]
fn event_buffer_inconsistent_stack() {
  let location = chars::Location::default();
  let mut buffer = EventBuffer::new(10);
  buffer.push(Event { location, kind: EventKind::Begin("A") });
  buffer.push(Event { location, kind: EventKind::Fragments(vec!['x']) });
  buffer.push(Event { location, kind: EventKind::End("B") });
}

#[test]
#[cfg(debug_assertions)]
#[should_panic]
fn event_buffer_unexpected_end_event() {
  let location = chars::Location::default();
  let mut buffer = EventBuffer::<_, char>::new(10);
  buffer.push(Event { location, kind: EventKind::End("A") });
}

#[test]
fn event_buffer_forward_matching_length() {
  let buffer1 = Events::new().begin("A").fragments("xyz").end().to_event_buffer();
  let buffer2 = Events::new().begin("A").fragments("xyz").end().to_event_buffer();
  assert_eq!(3, buffer1.len());
  assert_eq!(3, buffer2.len());
  assert_eq!(3, buffer1.forward_matching_length(&buffer2));
  assert_eq!(3, buffer2.forward_matching_length(&buffer1));

  let buffer1 = Events::new().begin("A").fragments("xy").begin("B").fragments("z").end().end().to_event_buffer();
  let buffer2 = Events::new().begin("A").fragments("xy").begin("C").fragments("z").end().end().to_event_buffer();
  assert_eq!(6, buffer1.len());
  assert_eq!(6, buffer2.len());
  assert_eq!(2, buffer1.forward_matching_length(&buffer2));
  assert_eq!(2, buffer2.forward_matching_length(&buffer1));
}

#[test]
fn context_definition_not_found() {
  let schema = Schema::<&str, char>::new("Foo");

  let mut events = Vec::new();
  let handler = |e: &Event<_, _>| events.push(e.clone());
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
  let handler = |e: &Event<_, _>| events.push(e.clone());
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  parser.push('1').unwrap();
  parser.push('2').unwrap();
  parser.finish().unwrap();
  Events::new().begin("A").fragments("012").end().assert_eq(&events);

  let mut events = Vec::new();
  let handler = |e: &Event<_, _>| events.push(e.clone());
  let parser = Context::new(&schema, "A", handler).unwrap();
  assert_unmatch(parser.finish(), location(0, 0, 0), "", "[ASCII_DIGIT{3}]", "[EOF]");
}

#[test]
fn context_eof_expected_but_valud_value_arrived() {
  let a = ascii_digit() * 3;
  let schema = Schema::new("Foo").define("A", a);

  let mut events = Vec::new();
  let handler = |e: &Event<_, _>| events.push(e.clone());
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  parser.push('1').unwrap();
  parser.push('2').unwrap();
  assert_unmatch(parser.push('3'), location(3, 0, 3), "012", "[EOF]", "['3']...");
  assert_prev_err(parser.push('4'));
  assert_prev_err(parser.finish());
}

#[test]
fn context_valid_value_expected_but_eof_detected() {
  let a = ascii_digit() * 3;
  let schema = Schema::new("Foo").define("A", a);

  let mut events = Vec::new();
  let handler = |e: &Event<_, _>| events.push(e.clone());
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  parser.push('1').unwrap();
  assert_unmatch(parser.finish(), location(2, 0, 2), "01", "[ASCII_DIGIT{3}]", "[EOF]");
}

#[test]
fn context_multiple_match() {
  let a = (ascii_digit() * 3) | (ascii_digit() * (3..=4));
  let schema = Schema::new("Foo").define("A", a);

  let mut events = Vec::new();
  let handler = |e: &Event<_, _>| events.push(e.clone());
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  parser.push('1').unwrap();
  parser.push('2').unwrap();
  match parser.finish() {
    Err(Error::MultipleMatches { location: l, prefix, expecteds, actual }) => {
      assert_eq!(location(3, 0, 3), l);
      assert_eq!("012", prefix);
      assert_eq_without_order(&["[ASCII_DIGIT{3}]", "[ASCII_DIGIT{3,4}]"], &expecteds);
      assert_eq!("[EOF]", actual);
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
    let handler = |e: &Event<_, _>| events.push(e.clone());
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    for ch in digits.chars() {
      parser.push(ch).unwrap();
    }
    parser.finish().unwrap();
    Events::new().begin("A").fragments(digits).end().assert_eq(&events);
  }

  // if less than the repetition range
  let mut events = Vec::new();
  let handler = |e: &Event<_, _>| events.push(e.clone());
  let parser = Context::new(&schema, "A", handler).unwrap();
  assert_unmatch(parser.finish(), location(0, 0, 0), "", "[ASCII_DIGIT{1,3}]", "[EOF]");

  // if the repetition range is exceeded
  let mut events = Vec::new();
  let handler = |e: &Event<_, _>| events.push(e.clone());
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push('0').unwrap();
  parser.push('1').unwrap();
  parser.push('2').unwrap();
  assert_unmatch(parser.push('3'), location(3, 0, 3), "012", "[EOF]", "['3']...");
}

#[test]
fn context_match_following_match_within_repetition_range() {
  let a = (ascii_digit() * (1..=3)) & ascii_alphabetic();
  let schema = Schema::new("Foo").define("A", a);

  let mut events = Vec::new();
  let handler = |e: &Event<_, _>| events.push(e.clone());
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  assert_unmatch(parser.push('X'), location(0, 0, 0), "", "[ASCII_DIGIT{1,3}]", "['X']...");

  for digits in &["0", "01", "012"] {
    let mut events = Vec::new();
    let handler = |e: &Event<_, _>| events.push(e.clone());
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
  let handler = |e: &Event<_, _>| events.push(e.clone());
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
  let handler = |e: &Event<_, _>| events.push(e.clone());
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
  let handler = |e: &Event<_, _>| {
    println!("  RECEIVED: {:?}", e);
    events.push(e.clone());
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
fn context_create_unmatched_label_prefix() {
  let buffer = "0123456789".chars().collect::<Vec<_>>();
  for (buf_offset, match_length, expected) in [
    (0, 0, ""),
    (0, 1, "0"),
    (0, 2, "01"),
    (1, 0, "."),
    (1, 1, ".0"),
    (1, 2, ".01"),
    (2, 0, ".."),
    (2, 1, "..0"),
    (2, 2, "..01"),
    (3, 0, "..."),
    (3, 1, "...0"),
    (3, 2, "...01"),
    (4, 0, "..."),
    (4, 1, "...0"),
    (4, 2, "...01"),
  ] {
    let actual = create_unmatched_label_prefix(&buffer, buf_offset, match_length);
    assert_eq!(expected, actual);
  }
}

#[test]
fn context_create_unmatched_label_actual() {
  let buffer = "01234567890123456789".chars().collect::<Vec<_>>();
  for (match_length, expected) in
    [(0, "['0']123456789012..."), (1, "['1']234567890123..."), (18, "['8']9..."), (19, "['9']..."), (20, "[EOF]")]
  {
    let actual = create_unmatched_label_actual(&buffer, match_length);
    assert_eq!(expected, actual);
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

  let a = keywords.iter().map(|kwd| token(kwd)).reduce(|a, b| a | b).unwrap();
  let schema = Schema::new("Foo").define("A", a);
  for kwd in &keywords {
    eprintln!("[{}] ---------------------", kwd);
    let mut events = Vec::new();
    let handler = |e: &Event<_, _>| {
      eprintln!("> Event[{}] {:?}", e.location, e.kind);
      events.push(e.clone());
    };
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    parser.push_str(kwd).unwrap();
    parser.finish().unwrap();
    Events::new().begin("A").fragments(kwd).end().assert_eq(&events);

    let mut events = Vec::new();
    let handler = |e: &Event<_, _>| events.push(e.clone());
    let parser = Context::new(&schema, "A", handler).unwrap();
    let expecteds = keywords.iter().map(|kwd| format!("[{}]", kwd)).collect::<Vec<_>>();
    assert_unmatches(parser.finish(), location(0, 0, 0), "", &expecteds, "[EOF]");

    let mut events = Vec::new();
    let handler = |e: &Event<_, _>| events.push(e.clone());
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    let expecteds = keywords.iter().map(|kwd| format!("[{}]", kwd)).collect::<Vec<_>>();
    assert_unmatches(parser.push('X'), location(0, 0, 0), "", &expecteds, "['X']...");

    let mut events = Vec::new();
    let handler = |e: &Event<_, _>| events.push(e.clone());
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
    let handler = |e: &Event<_, _>| events.push(e.clone());
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    parser.push_str(kwd).unwrap();
    parser.finish().unwrap();
    Events::new().begin("A").fragments(kwd).end().assert_eq(&events);

    let mut events = Vec::new();
    let handler = |e: &Event<_, _>| events.push(e.clone());
    let parser = Context::new(&schema, "A", handler).unwrap();
    let expected = format!("[{}]", keywords.to_vec().join("|"));
    assert_unmatch(parser.finish(), location(0, 0, 0), "", &expected, "[EOF]");

    let mut events = Vec::new();
    let handler = |e: &Event<_, _>| events.push(e.clone());
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    let expected = format!("[{}]", keywords.to_vec().join("|"));
    assert_unmatch(parser.push('X'), location(0, 0, 0), "", &expected, "['X']...");

    let mut events = Vec::new();
    let handler = |e: &Event<_, _>| events.push(e.clone());
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    parser.push_str(kwd).unwrap();
    assert!(matches!(parser.push('X'), Err(Error::<char>::Unmatched { .. }))); // various errors
  }
}

#[test]
fn context_push_seq() {
  let a = ascii_digit() * 3;
  let schema = Schema::new("Foo").define("A", a);

  for splitted_sample in combination_div("012") {
    let mut events = Vec::new();
    let handler = |e: &Event<_, _>| events.push(e.clone());
    let mut parser = Context::new(&schema, "A", handler).unwrap();
    for fragment in splitted_sample {
      parser.push_str(&fragment).unwrap();
    }
    parser.finish().unwrap();
    Events::new().begin("A").fragments("012").end().assert_eq(&events);
  }

  let mut events = Vec::new();
  let handler = |e: &Event<_, _>| events.push(e.clone());
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push_seq(&[]).unwrap(); // empty sequence
  parser.push_str("012").unwrap();
  assert_unmatch(parser.push_str("3"), location(3, 0, 3), "012", "[EOF]", "['3']...");
}

#[test]
fn context_fit_buffer_to_min_size() {
  let a = ascii_digit() * (0..);
  let schema = Schema::new("Foo").define("A", a);
  let mut events = Vec::new();
  let handler = |e: &Event<_, _>| events.push(e.clone());
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  let mut expected = Events::new().begin("A");
  for i in 0..=1024 {
    let ch = char::from_digit(i % 10, 10).unwrap();
    parser.push(ch).unwrap();
    expected = expected.fragments(&ch.to_string());
  }
  parser.finish().unwrap();
  expected.end().assert_eq(&events);
}

#[test]
fn check_whether_possible_to_proceed() {
  let a = ascii_digit() * 3;
  let schema = Schema::new("Foo").define("A", a);

  let mut events = Vec::new();
  let handler = |e: &Event<_, _>| events.push(e.clone());
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push_str("012").unwrap(); // completed
  parser.push_str("").unwrap(); // OK
  parser.finish().unwrap();
  Events::new().begin("A").fragments("012").end().assert_eq(&events);

  let mut events = Vec::new();
  let handler = |e: &Event<_, _>| events.push(e.clone());
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push_str("012").unwrap(); // completed
  assert_unmatch(parser.push_str("3"), location(3, 0, 3), "012", "[EOF]", "['3']...");

  let mut events = Vec::new();
  let handler = |e: &Event<_, _>| events.push(e.clone());
  let mut parser = Context::new(&schema, "A", handler).unwrap();
  parser.push_str("0").unwrap();
  parser.push_str("12").unwrap(); // completed
  parser.finish().unwrap();
  Events::new().begin("A").fragments("012").end().assert_eq(&events);
}

#[test]
fn schema_named_syntax() {
  // matches "♥A", "♠Q"...
  let schema = Schema::new("Foo")
    .define("CARD", id("SUIT") & id("RANK"))
    .define("SUIT", one_of_chars("♠♣♦♥"))
    .define("RANK", one_of_chars("A233456789XJQK"));

  for suit in "♠♣♦♥".chars() {
    for rank in "A233456789XJQK".chars() {
      let sample = format!("{}{}", suit, rank);
      let mut events = Vec::new();
      let handler = |e: &Event<_, _>| events.push(e.clone());
      let mut parser = Context::new(&schema, "CARD", handler).unwrap();
      parser.push_str(&sample).unwrap();
      parser.finish().unwrap();
      Events::new()
        .begin("CARD")
        .begin("SUIT")
        .fragments(&suit.to_string())
        .end()
        .begin("RANK")
        .fragments(&rank.to_string())
        .end()
        .end()
        .assert_eq(&events);
    }
  }
}

#[test]
fn schema_named_syntax_recursive() {
  // This will matches "terp", "(terp)", "((terp))", "(((terp)))", ...
  let schema = Schema::new("Foo").define("P", (ch('(') & id("P") & ch(')')) | token("terp"));

  for i in 0..=10 {
    let sample = format!("{}terp{}", (0..i).map(|_| '(').collect::<String>(), (0..i).map(|_| ')').collect::<String>());
    println!("sample: {}", sample);
    let mut events = Vec::new();
    let handler = |e: &Event<_, _>| events.push(e.clone());
    let mut parser = Context::new(&schema, "P", handler).unwrap();
    parser.push_str(&sample).unwrap();
    parser.finish().unwrap();
    let expected = (0..i).fold(Events::new().begin("P"), |es, _| es.fragments("(").begin("P"));
    let expected = expected.fragments("terp");
    let expected = (0..i).fold(expected, |es, _| es.end().fragments(")")).end();
    expected.assert_eq(&events);
  }
}

fn assert_prev_err<T: Debug + PartialEq>(r: Result<char, T>) {
  assert_eq!(Err(Error::Previous), r);
}

fn assert_unmatch<T: Debug>(r: Result<char, T>, l: chars::Location, p: &str, e: &str, a: &str) {
  assert_unmatches(r, l, p, &[String::from(e)], a)
}

fn assert_unmatches<T: Debug>(r: Result<char, T>, l: chars::Location, p: &str, e: &[String], a: &str) {
  if let Err(Error::<char>::Unmatched { location, prefix, expecteds, actual, .. }) = &r {
    assert_eq!((&l, p, a), (location, prefix.as_str(), actual.as_str()));
    assert_eq!(e.len(), expecteds.len());
    assert_eq_without_order(e, expecteds);
  } else {
    panic!("Err(Error::Unmatched{{expected: {:?}, actual: {:?}}}) expected, but {:?}", e, a, r);
  }
}

fn assert_eq_without_order<T: AsRef<str>, U: AsRef<str>>(expected: &[U], actual: &[T]) {
  let mut expected = expected.iter().map(|e| e.as_ref().to_string()).collect::<Vec<String>>();
  let mut actual = actual.iter().map(|a| a.as_ref().to_string()).collect::<Vec<String>>();
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

fn combination_div(s: &str) -> Vec<Vec<String>> {
  let chars = s.chars().collect::<Vec<_>>();
  let mut result = Vec::new();
  for divs in combination_sum(chars.len()) {
    for divs_len in divs.iter().permutations(divs.len()) {
      let mut offset = 0;
      let mut splitted = Vec::with_capacity(divs.len());
      for len in divs_len {
        splitted.push(chars[offset..offset + len].iter().collect::<String>());
        offset += len;
      }
      result.push(splitted);
    }
  }
  let result = result.into_iter().unique().collect::<Vec<_>>();
  println!("combination_div({:?}) = {:?}", s, result);
  result
}

fn combination_sum(sum: usize) -> Vec<Vec<usize>> {
  fn _cs(target: usize, nums: &[usize], curr: &mut Vec<usize>, result: &mut Vec<Vec<usize>>) {
    if target == 0 {
      result.push(curr.clone());
      return;
    } else if nums.is_empty() {
      return;
    } else if target >= nums[0] {
      curr.push(nums[0]);
      _cs(target - nums[0], nums, curr, result);
      curr.pop();
    }
    _cs(target, &nums[1..], curr, result);
  }
  let mut result = Vec::new();
  _cs(sum, &(1..=sum).collect::<Vec<_>>(), &mut Vec::new(), &mut result);
  result
}

fn normalize<ID: Clone + Display + Debug + Eq + Eq + Hash>(events: &[Event<ID, char>]) -> Vec<Event<ID, char>> {
  let mut buffer = EventBuffer::new(events.len());
  for e in events {
    buffer.push(e.clone());
  }
  let mut events = Vec::with_capacity(events.len());
  buffer.flush_to(buffer.len(), &mut |e| events.push(e.clone()));
  events
}

fn location(chars: u64, lines: u64, columns: u64) -> chars::Location {
  chars::Location { chars, lines, columns }
}

pub(crate) struct Events<ID: Clone + Display + Debug + Eq + Eq + Hash> {
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
  pub fn to_event_buffer(&self) -> EventBuffer<ID, char> {
    let mut buffer = EventBuffer::new(self.events.len());
    for e in &self.events {
      buffer.push(e.clone());
    }
    buffer
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
