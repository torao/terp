use crate::schema::{Item, MatchResult, Matcher, Primary, Syntax};
use crate::Result;
use std::collections::HashSet;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::RangeInclusive;

#[cfg(test)]
mod test;

#[macro_export]
macro_rules! patterned_single_item {
  ($name:ident, $p:pat) => {
    patterned_single_item!(char, $name, $p)
  };
  ($t:tt, $name:ident, $p:pat) => {{
    fn _single_pattern<E: $crate::schema::Item>(values: &[$t]) -> Result<E, MatchResult> {
      Ok(if values.is_empty() {
        MatchResult::UnmatchAndCanAcceptMore
      } else {
        if matches!(values[0], $p) {
          MatchResult::Match(1)
        } else {
          MatchResult::Unmatch
        }
      })
    }
    Syntax::from_matcher(stringify!($name), _single_pattern)
  }};
}

pub use patterned_single_item;

pub fn id<ID, E: Item>(id: ID) -> Syntax<ID, E> {
  Syntax::from_id(id)
}

pub fn id_str<S: Into<String>>(id: S) -> Syntax<String, char> {
  Syntax::from_id_str(id)
}

pub fn single<ID, E: Item>(item: E) -> Syntax<ID, E> {
  #[derive(Debug)]
  struct SingleMatcher<E: Item> {
    item: E,
  }
  impl<E: Item> Matcher<E> for SingleMatcher<E> {
    fn matches(&self, values: &[E]) -> Result<E, MatchResult> {
      if values.is_empty() {
        Ok(MatchResult::UnmatchAndCanAcceptMore)
      } else if values[0] == self.item {
        Ok(MatchResult::Match(1))
      } else {
        Ok(MatchResult::Unmatch)
      }
    }
  }
  impl<E: Item> Display for SingleMatcher<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.write_str(&E::debug_symbol(self.item))
    }
  }
  Syntax::with_primary(Primary::Term(Box::new(SingleMatcher { item })))
}

pub fn range<ID, E: Item + PartialOrd>(r: RangeInclusive<E>) -> Syntax<ID, E> {
  named_matcher(
    format!("{{{},{}}}", E::debug_symbol(*r.start()), E::debug_symbol(*r.end())),
    move |values: &[E]| -> Result<E, MatchResult> {
      if values.is_empty() {
        Ok(MatchResult::UnmatchAndCanAcceptMore)
      } else if r.contains(&values[0]) {
        Ok(MatchResult::Match(1))
      } else {
        Ok(MatchResult::Unmatch)
      }
    },
  )
}

pub fn seq<ID, E: Item>(items: &[E]) -> Syntax<ID, E> {
  #[derive(Debug)]
  struct SeqMatcher<E: Item> {
    items: Vec<E>,
  }
  impl<E: Item> Matcher<E> for SeqMatcher<E> {
    fn matches(&self, values: &[E]) -> Result<E, MatchResult> {
      let min = std::cmp::min(self.items.len(), values.len());
      for (i, value) in values.iter().take(min).enumerate() {
        if *value != self.items[i] {
          return Ok(MatchResult::Unmatch);
        }
      }
      Ok(if min < self.items.len() { MatchResult::UnmatchAndCanAcceptMore } else { MatchResult::Match(min) })
    }
  }
  impl<E: Item> Display for SeqMatcher<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.write_str(&E::debug_symbols(&self.items))
    }
  }
  let items = items.to_vec();
  Syntax::with_primary(Primary::Term(Box::new(SeqMatcher { items })))
}

pub fn one_of<ID, E: Item + Hash>(items: &[E]) -> Syntax<ID, E> {
  #[derive(Debug)]
  struct OneOfMatcher<E: Item + Hash> {
    items: HashSet<E>,
  }
  impl<E: Item + Hash> Matcher<E> for OneOfMatcher<E> {
    fn matches(&self, values: &[E]) -> Result<E, MatchResult> {
      if values.is_empty() {
        Ok(MatchResult::UnmatchAndCanAcceptMore)
      } else if self.items.contains(&values[0]) {
        Ok(MatchResult::Match(1))
      } else {
        Ok(MatchResult::Unmatch)
      }
    }
  }
  impl<E: Item + Hash> Display for OneOfMatcher<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.write_str(&self.items.iter().map(|i| E::debug_symbol(*i)).collect::<Vec<_>>().join("|"))
    }
  }
  let items = items.iter().fold(HashSet::with_capacity(items.len()), |mut items, item| {
    items.insert(*item);
    items
  });
  Syntax::with_primary(Primary::Term(Box::new(OneOfMatcher { items })))
}

pub fn one_of_seqs<ID, E: Item + PartialEq>(items: &[Vec<E>]) -> Syntax<ID, E> {
  #[derive(Debug)]
  struct OneOfSeqMatcher<E: Item + PartialEq> {
    items: Vec<Vec<E>>,
  }
  impl<E: Item + PartialEq> Matcher<E> for OneOfSeqMatcher<E> {
    fn matches(&self, values: &[E]) -> Result<E, MatchResult> {
      use MatchResult::*;
      let result = self
        .items
        .iter()
        .map(|i| {
          let len = std::cmp::min(i.len(), values.len());
          if values[..len] == i[..len] {
            if len == i.len() {
              Match(len)
            } else {
              UnmatchAndCanAcceptMore
            }
          } else {
            Unmatch
          }
        })
        .reduce(|accum, result| match (accum, result) {
          (MatchAndCanAcceptMore(a), Match(b)) if b > a => MatchAndCanAcceptMore(b),
          (MatchAndCanAcceptMore(a), _) => {
            debug_assert!(!matches!(result, MatchAndCanAcceptMore(_)));
            MatchAndCanAcceptMore(a)
          }
          (Match(a), Match(b)) => Match(std::cmp::max(a, b)),
          (Match(a), UnmatchAndCanAcceptMore) => MatchAndCanAcceptMore(a),
          (Match(a), _) => {
            debug_assert!(!matches!(result, MatchAndCanAcceptMore(_)));
            Match(a)
          }
          (UnmatchAndCanAcceptMore, Match(b)) => MatchAndCanAcceptMore(b),
          (UnmatchAndCanAcceptMore, _) => {
            debug_assert!(!matches!(result, MatchAndCanAcceptMore(_)));
            UnmatchAndCanAcceptMore
          }
          (Unmatch, b) => {
            debug_assert!(!matches!(result, MatchAndCanAcceptMore(_)));
            b
          }
        })
        .unwrap_or(Unmatch);
      Ok(result)
    }
  }
  impl<E: Item + PartialEq> Display for OneOfSeqMatcher<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.write_str(&self.items.iter().map(|i| E::debug_symbols(i)).collect::<Vec<_>>().join("|"))
    }
  }
  let items = items.iter().map(|i| i.to_vec()).collect::<Vec<_>>();
  Syntax::with_primary(Primary::Term(Box::new(OneOfSeqMatcher { items })))
}

fn named_matcher<ID, E: Item, NAME, FN>(name: NAME, eval: FN) -> Syntax<ID, E>
where
  NAME: Into<String>,
  FN: Sync + Send + Fn(&[E]) -> Result<E, MatchResult> + 'static,
{
  struct NamedMatcher<E, FN>
  where
    E: Item,
    FN: Sync + Send + Fn(&[E]) -> Result<E, MatchResult>,
  {
    name: String,
    eval: Box<FN>,
    phantom: PhantomData<E>,
  }

  impl<E: Item, FN> Matcher<E> for NamedMatcher<E, FN>
  where
    FN: Sync + Send + Fn(&[E]) -> Result<E, MatchResult>,
  {
    fn matches(&self, values: &[E]) -> Result<E, MatchResult> {
      (self.eval)(values)
    }
  }

  impl<E: Item, FN> Debug for NamedMatcher<E, FN>
  where
    FN: Sync + Send + Fn(&[E]) -> Result<E, MatchResult>,
  {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.debug_struct("NamedMatcher").field("name", &self.name).finish()
    }
  }

  impl<E: Item, FN> Display for NamedMatcher<E, FN>
  where
    FN: Sync + Send + Fn(&[E]) -> Result<E, MatchResult>,
  {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.write_str(&self.name)
    }
  }

  let name = name.into();
  Syntax::with_primary(Primary::Term(Box::new(NamedMatcher { name, eval: Box::new(eval), phantom: PhantomData })))
}
