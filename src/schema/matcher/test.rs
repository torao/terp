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

#[test]
fn one_of_seqs() {
  use itertools::Itertools;
  use MatchResult::*;
  for seqs in [vec!['a'], vec!['a', 'a'], vec!['b', 'a']].iter().permutations(3) {
    let seqs = seqs.into_iter().cloned().collect::<Vec<_>>();
    let syntax = super::one_of_seqs::<String, _>(&seqs);
    assert_match_str(&syntax, "", Ok(UnmatchAndCanAcceptMore));
    assert_match_str(&syntax, "a", Ok(MatchAndCanAcceptMore(1)));
    assert_match_str(&syntax, "b", Ok(UnmatchAndCanAcceptMore));
    assert_match_str(&syntax, "c", Ok(Unmatch));
    assert_match_str(&syntax, "aa", Ok(Match(2)));
    assert_match_str(&syntax, "ab", Ok(Match(1)));
    assert_match_str(&syntax, "ac", Ok(Match(1)));
    assert_match_str(&syntax, "ba", Ok(Match(2)));
    assert_match_str(&syntax, "bc", Ok(Unmatch));
  }
}

fn assert_match_str<ID>(syntax: &Syntax<ID, char>, values: &str, expected: Result<char, MatchResult>) {
  let values = values.chars().collect::<Vec<_>>();
  assert_match(syntax, &values, expected);
}

fn assert_match<ID, E: Item>(syntax: &Syntax<ID, E>, values: &[E], expected: Result<E, MatchResult>) {
  let result = if let Syntax { primary: Primary::Term(_, matcher), .. } = syntax { matcher(values) } else { panic!() };
  assert_eq!(expected, result);
}
