use crate::schema::chars::{ascii_alphabetic, ascii_digit};
use crate::schema::MatchResult;
use crate::schema::{Item, Schema, Syntax};

#[test]
fn create_new_schema() {
  let schema = Schema::new("Foo").define("X", ascii_digit() * (1..=3)).define("Y", ascii_digit() * 4);
  assert_eq!("Foo", schema.name());
  assert!(matches!(schema.get(&"X"), Some(_)));
  assert!(matches!(schema.get(&"Y"), Some(_)));
  assert!(matches!(schema.get(&"Z"), None));
  let mut names = schema.ids().map(|s| s.to_string()).collect::<Vec<_>>();
  names.sort();
  assert_eq!(2, names.len());
  assert_eq!("X", names[0]);
  assert_eq!("Y", names[1]);

  assert_eq!(
    r#"Foo
  "X" := ASCII_DIGIT{1,3}
  "Y" := ASCII_DIGIT{4}
"#,
    schema.to_string()
  );
  let _ = format!("{:?}", schema);
}

#[test]
fn syntax() {
  let syntax = ascii_digit::<String>();
  assert_eq!(1..=1, *syntax.repetition());

  let syntax = syntax * 2;
  assert_eq!(2..=2, *syntax.repetition());

  let syntax = syntax * (1..=2);
  assert_eq!(2..=4, *syntax.repetition());

  let syntax = syntax & (ascii_digit() * 3);
  let syntax = (ascii_alphabetic() * 2) | syntax;
  assert_eq!("ASCII_ALPHA{2} | ASCII_DIGIT{2,4}, ASCII_DIGIT{3}", syntax.to_string());
}

#[test]
fn syntax_and_concat() {
  let s1 = ascii_digit::<String>();
  let s2 = ascii_alphabetic();
  let s3 = ascii_digit();
  let s = (s1 & s2) & s3;
  let s1 = Syntax::from_id_str("FOO");
  let s2 = ascii_alphabetic();
  let s3 = ascii_digit();
  let s = s1 & (s2 & s3) & s;
  assert_eq!("FOO, ASCII_ALPHA, ASCII_DIGIT, ASCII_DIGIT, ASCII_ALPHA, ASCII_DIGIT", s.to_string());
  let _ = format!("{:?}", s);
}

#[test]
fn syntax_or_concat() {
  let s1 = ascii_digit::<String>();
  let s2 = ascii_alphabetic();
  let s3 = ascii_digit();
  let s = (s1 | s2) | s3;
  let s1 = Syntax::from_id_str("FOO");
  let s2 = ascii_alphabetic();
  let s3 = ascii_digit();
  let s = s1 | (s2 | s3) | s;
  assert_eq!("FOO | ASCII_ALPHA | ASCII_DIGIT | ASCII_DIGIT | ASCII_ALPHA | ASCII_DIGIT", s.to_string());
  let _ = format!("{:?}", s);
}

#[test]
fn syntax_repetition() {
  let s = ((ascii_digit::<String>() * 5) * (2..=3)) * (1..4);
  assert_eq!("ASCII_DIGIT{10,60}", s.to_string());
}

#[test]
fn syntax_repetition_for_sequence() {
  let s = (((ascii_alphabetic::<String>() & ascii_digit()) * 2) & ((ascii_digit() & ascii_digit()) * 3)) * (1..=2);
  assert_eq!("((ASCII_ALPHA, ASCII_DIGIT){2}, (ASCII_DIGIT, ASCII_DIGIT){3}){1,2}", s.to_string());
}

#[test]
fn syntax_display() {
  for (reps, expected) in vec![
    (0..=0, "{0}"),
    (0..=1, "?"),
    (0..=2, "{0,2}"),
    (0..=usize::MAX, "*"),
    (1..=1, ""),
    (1..=2, "{1,2}"),
    (1..=usize::MAX, "+"),
    (2..=2, "{2}"),
    (2..=3, "{2,3}"),
    (2..=usize::MAX, "{2,}"),
  ] {
    let s = ascii_alphabetic::<String>() * reps;
    assert_eq!(format!("ASCII_ALPHA{}", expected), s.to_string());
  }
}

#[test]
fn item_for_char_debug_symbol() {
  for (expected, sample) in vec![
    ("'A'", 'A'),
    ("'\\0'", '\u{0}'),
    ("'\\u{1}'", '\u{1}'),
    ("'\\u{7f}'", '\u{7F}'),
    ("'\\t'", '\t'),
    ("'\\r'", '\r'),
    ("'\\n'", '\n'),
    ("'桜'", '桜'),
    ("'💕'", '💕'),
  ]
  .iter()
  {
    assert_eq!(*expected, Item::debug_symbol(*sample));
  }
}

#[test]
fn match_result() {
  fn eq(m1: &MatchResult, m2: &MatchResult) {
    match (m1, m2) {
      (MatchResult::Match(x), MatchResult::Match(y)) => assert_eq!(x, y),
      (MatchResult::Unmatch, MatchResult::Unmatch) => (),
      (MatchResult::MatchAndCanAcceptMore(_), MatchResult::MatchAndCanAcceptMore(_)) => (),
      (MatchResult::UnmatchAndCanAcceptMore, MatchResult::UnmatchAndCanAcceptMore) => (),
      _ => panic!(),
    }
  }
  let x = MatchResult::Match(1234);
  let y = x;
  eq(&x, &y);
  eq(&x, &x.clone());

  assert!(MatchResult::Match(0).is_match());
  assert!(!MatchResult::Unmatch.is_match());
  assert!(MatchResult::MatchAndCanAcceptMore(0).is_match());
  assert!(!MatchResult::UnmatchAndCanAcceptMore.is_match());
}

#[test]
fn item_for_char_debug_symbols() {
  for (expected, sample) in vec![("", ""), ("ABC", "ABC"), ("A\\tB\\nC\\0", "A\tB\nC\0")].iter() {
    let sample = (*sample).chars().collect::<Vec<_>>();
    assert_eq!(*expected, Item::debug_symbols(&sample));
  }
}

#[test]
fn item_for_u8_to_single_debug() {
  for b in 0u8..=0xFFu8 {
    assert_eq!(format!("{b:02X}"), Item::debug_symbol(b));
  }
}

#[test]
fn item_for_u8_to_sampling_debug() {
  for b1 in 0u8..=0xFFu8 {
    for b2 in 0u8..=0xFFu8 {
      assert_eq!(format!("{b1:02X}{b2:02X}"), Item::debug_symbols(&[b1, b2]));
    }
  }
}
