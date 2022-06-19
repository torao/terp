use crate::{schema::Item, Result};

pub trait Matcher<E: Item> {
  fn matches(&self, sequence: &[E]) -> Result<MatchResult>;
  fn next(&self, sequence: &[E]) -> Vec<Box<Matcher<E>>>;
}

pub enum MatchResult {
  Match,
  MatchAndCanAcceptMore,
  Unmatch,
  UnmatchAndCanAcceptMore,
}

struct MatcherState<E: Item> {
  matcher: Box<dyn Matcher<E>>,
  matcher_state: Option<MatchResult>,
  buffer: Vec<E>,
}

impl<E: Item> MatcherState {
  fn push(&mut self, item: E) -> Result<(MatchResult, MatchResult)> {
    self.buffer.push(item);
    let new_state = self.matcher.matches(&self.buffer)?;
    match new_state {
      MatchResult::Unmatch => match prev_state {
        None => MatchResult::Unmatch,
        Some(MatchResult::MatchAndCanAcceptMore) => {
          self.buffer.pop();
          MatchResult::MatchComplete
        }
      },
      MatchResult::MatchComplete => MatchResult::Match,
      MatchResult::MatchAndCanAcceptMore => Ok(Status::Unresolve),
      MatchResult::Unmatch => Ok(Status::Unmatch),
    }
  }
  fn next(&self) -> Vec<MatcherState<E>> {
    todo!()
  }
  fn finish(&mut self) -> Result<Status<E>> {
    todo!()
  }
}

enum Status<E: Item> {
  Match(Box<dyn State<E>>),
  Unmatch,
  Unresolve,
}
pub struct Context {}
