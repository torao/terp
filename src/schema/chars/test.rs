use crate::schema::chars::Location;
use crate::schema::{Item, Location as L, MatchResult, Primary, Syntax};
use crate::Result;

#[test]
fn char_location() {
  let mut l = Location::default();
  assert!(matches!(l, Location { chars: 0, lines: 0, columns: 0 }));
  l.increment_with('A');
  assert!(matches!(l, Location { chars: 1, lines: 0, columns: 1 }));
  l.increment_with('あ');
  assert!(matches!(l, Location { chars: 2, lines: 0, columns: 2 }));
  l.increment_with('\n');
  assert!(matches!(l, Location { chars: 3, lines: 1, columns: 0 }));
  l.increment_with('😊');
  assert!(matches!(l, Location { chars: 4, lines: 1, columns: 1 }));
  l.increment_with('\r');
  assert!(matches!(l, Location { chars: 5, lines: 1, columns: 0 }));
  l.increment_with('\n');
  assert!(matches!(l, Location { chars: 6, lines: 2, columns: 0 }));
  l.increment_with('🗿'); // surrogate pairs
  assert!(matches!(l, Location { chars: 7, lines: 2, columns: 1 }));
  assert_eq!("(3,2)", l.to_string());

  fn assert_equals(l1: &Location, l2: &Location) {
    assert_eq!(l1.chars, l2.chars);
    assert_eq!(l1.lines, l2.lines);
    assert_eq!(l1.columns, l2.columns);
  }
  let _ = format!("{:?}", l);
  let l2 = l;
  assert_equals(&l, &l2);
  assert_equals(&l, &l.clone());
}

#[test]
fn ascii_digit() {
  test_all(super::ascii_digit(), "ASCII_DIGIT", '\0', '\x7F', &|ch: char| ch.is_ascii_digit());
}

#[test]
fn ascii_lower_alphabetic() {
  test_all(super::ascii_lower_alphabetic(), "ASCII_LOWER", '\0', '\x7F', &|ch: char| ch.is_ascii_lowercase());
}

#[test]
fn ascii_upper_alphabetic() {
  test_all(super::ascii_upper_alphabetic(), "ASCII_UPPER", '\0', '\x7F', &|ch: char| ch.is_ascii_uppercase());
}

#[test]
fn ascii_alphabetic() {
  test_all(super::ascii_alphabetic(), "ASCII_ALPHA", '\0', '\x7F', &|ch: char| ch.is_ascii_alphabetic());
}

fn test_all(syntax: Syntax<String, char>, label: &str, t0: char, t1: char, pred: &dyn Fn(char) -> bool) {
  assert_eq!(label, syntax.to_string());
  let _ = format!("{:?}", syntax);
  let matcher = get_matcher(syntax);
  assert!(matches!(matcher(&[]), Ok(MatchResult::UnmatchAndCanAcceptMore)));
  for ch in t0..=t1 {
    match (pred(ch), matcher(&[ch])) {
      (true, Ok(MatchResult::Match(1))) => (),
      (false, Ok(MatchResult::Unmatch)) => (),
      _ => panic!("{} => {:?}", pred(ch), matcher(&[ch])),
    }
  }
}

fn get_matcher<ID, E: Item>(s: Syntax<ID, E>) -> Box<dyn Fn(&[E]) -> Result<E, MatchResult>> {
  match s {
    Syntax { primary: Primary::Term(_, matcher), .. } => matcher,
    _ => panic!(),
  }
}
