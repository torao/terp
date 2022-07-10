use crate::schema::{Item, MatchResult, Primary, Syntax};
use crate::Result;

#[test]
fn id_str() {
  let syntax = super::id_str("");
  let _ = format!("{:?}", syntax);
  let _ = format!("{}", syntax);
}

#[test]
fn single() {
  let syntax = super::single::<String, _>('X');
  assert_match_str(&syntax, "X", Ok(MatchResult::Match(1)));
  assert_match_str(&syntax, "XX", Ok(MatchResult::Match(1)));
  assert_match_str(&syntax, "XA", Ok(MatchResult::Match(1)));
  assert_match_str(&syntax, "", Ok(MatchResult::UnmatchAndCanAcceptMore));
  assert_match_str(&syntax, "A", Ok(MatchResult::Unmatch));
  assert_match_str(&syntax, "AX", Ok(MatchResult::Unmatch));
  let _ = format!("{:?}", syntax);
  let _ = format!("{}", syntax);
}

#[test]
fn range() {
  let syntax = super::range::<String, _>('A'..='F');
  for ch in 'A'..='F' {
    assert_match_str(&syntax, &ch.to_string(), Ok(MatchResult::Match(1)));
    assert_match_str(&syntax, &format!("{}{}", ch, ch), Ok(MatchResult::Match(1)));
    assert_match_str(&syntax, &format!("{}Z", ch), Ok(MatchResult::Match(1)));
    assert_match_str(&syntax, &format!("G{}", ch), Ok(MatchResult::Unmatch));
  }
  assert_match_str(&syntax, "", Ok(MatchResult::UnmatchAndCanAcceptMore));
  assert_match_str(&syntax, "G", Ok(MatchResult::Unmatch));
  let _ = format!("{:?}", syntax);
  let _ = format!("{}", syntax);
}

fn assert_match_str<ID>(syntax: &Syntax<ID, char>, values: &str, expected: Result<char, MatchResult>) {
  let values = values.chars().collect::<Vec<_>>();
  assert_match(syntax, &values, expected);
}

fn assert_match<ID, E: Item>(syntax: &Syntax<ID, E>, values: &[E], expected: Result<E, MatchResult>) {
  let result =
    if let Syntax { primary: Primary::Term(matcher), .. } = syntax { matcher.matches(values) } else { panic!() };
  assert_eq!(expected, result);
}
