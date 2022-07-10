use crate::schema::{Item, MatchResult, Syntax};
use crate::Result;
use std::collections::HashSet;
use std::hash::Hash;
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
    Syntax::from_fn(stringify!($name), _single_pattern)
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
  Syntax::from_fn(&E::debug_symbol(item), move |values: &[E]| -> Result<E, MatchResult> {
    if values.is_empty() {
      Ok(MatchResult::UnmatchAndCanAcceptMore)
    } else if values[0] == item {
      Ok(MatchResult::Match(1))
    } else {
      Ok(MatchResult::Unmatch)
    }
  })
}

pub fn range<ID, E: Item + PartialOrd>(r: RangeInclusive<E>) -> Syntax<ID, E> {
  let label = format!("{{{},{}}}", E::debug_symbol(*r.start()), E::debug_symbol(*r.end()));
  Syntax::from_fn(&label, move |values: &[E]| -> Result<E, MatchResult> {
    if values.is_empty() {
      Ok(MatchResult::UnmatchAndCanAcceptMore)
    } else if r.contains(&values[0]) {
      Ok(MatchResult::Match(1))
    } else {
      Ok(MatchResult::Unmatch)
    }
  })
}

pub fn seq<ID, E: Item>(items: &[E]) -> Syntax<ID, E> {
  let items = items.to_vec();
  Syntax::from_fn(&E::debug_symbols(&items), move |buffer: &[E]| -> Result<E, MatchResult> {
    let min = std::cmp::min(items.len(), buffer.len());
    for (i, value) in buffer.iter().take(min).enumerate() {
      if *value != items[i] {
        return Ok(MatchResult::Unmatch);
      }
    }
    Ok(if min < items.len() { MatchResult::UnmatchAndCanAcceptMore } else { MatchResult::Match(min) })
  })
}

pub fn one_of<ID, E: Item + Hash>(items: &[E]) -> Syntax<ID, E> {
  let label = items.iter().map(|i| E::debug_symbol(*i)).collect::<Vec<_>>().join("|");
  let items = items.iter().fold(HashSet::with_capacity(items.len()), |mut items, item| {
    items.insert(*item);
    items
  });
  Syntax::from_fn(&label, move |buffer: &[E]| -> Result<E, MatchResult> {
    if buffer.is_empty() {
      Ok(MatchResult::UnmatchAndCanAcceptMore)
    } else if items.contains(&buffer[0]) {
      Ok(MatchResult::Match(1))
    } else {
      Ok(MatchResult::Unmatch)
    }
  })
}

pub fn one_of_seqs<ID, E: Item + PartialEq>(items: &[Vec<E>]) -> Syntax<ID, E> {
  let label = items.iter().map(|i| E::debug_symbols(i)).collect::<Vec<_>>().join("|");
  let items = items.iter().map(|i| i.to_vec()).collect::<Vec<_>>();
  Syntax::from_fn(&label, move |buffer: &[E]| -> Result<E, MatchResult> {
    use MatchResult::*;
    let result = items
      .iter()
      .map(|i| {
        let len = std::cmp::min(i.len(), buffer.len());
        if buffer[..len] == i[..len] {
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
  })
}
