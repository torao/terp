use crate::schema::{MatchResult, Symbol, Syntax};
use crate::Result;
use std::collections::HashSet;
use std::hash::Hash;
use std::ops::RangeInclusive;

#[cfg(test)]
mod test;

pub fn id<ID, Σ: Symbol>(id: ID) -> Syntax<ID, Σ> {
  Syntax::from_id(id)
}

pub fn id_str<S: Into<String>>(id: S) -> Syntax<String, char> {
  Syntax::from_id_str(id)
}

pub fn single<ID, Σ: Symbol>(item: Σ) -> Syntax<ID, Σ> {
  Syntax::from_fn(&Σ::debug_symbol(item), move |values: &[Σ]| -> Result<Σ, MatchResult> {
    if values.is_empty() {
      Ok(MatchResult::UnmatchAndCanAcceptMore)
    } else if values[0] == item {
      Ok(MatchResult::Match(1))
    } else {
      Ok(MatchResult::Unmatch)
    }
  })
}

pub fn range<ID, Σ: Symbol + PartialOrd>(r: RangeInclusive<Σ>) -> Syntax<ID, Σ> {
  let label = format!("{{{},{}}}", Σ::debug_symbol(*r.start()), Σ::debug_symbol(*r.end()));
  range_with_label(&label, r)
}

pub fn range_with_label<ID, Σ: Symbol + PartialOrd>(label: &str, r: RangeInclusive<Σ>) -> Syntax<ID, Σ> {
  any_of_ranges_with_label(label, vec![r])
}

pub fn any_of_ranges_with_label<ID, Σ: Symbol + PartialOrd>(
  label: &str, rs: Vec<RangeInclusive<Σ>>,
) -> Syntax<ID, Σ> {
  Syntax::from_fn(label, move |values: &[Σ]| -> Result<Σ, MatchResult> {
    if values.is_empty() {
      Ok(MatchResult::UnmatchAndCanAcceptMore)
    } else if rs.iter().any(|r| r.contains(&values[0])) {
      Ok(MatchResult::Match(1))
    } else {
      Ok(MatchResult::Unmatch)
    }
  })
}

pub fn seq<ID, Σ: Symbol>(items: &[Σ]) -> Syntax<ID, Σ> {
  let items = items.to_vec();
  Syntax::from_fn(&Σ::debug_symbols(&items), move |buffer: &[Σ]| -> Result<Σ, MatchResult> {
    let min = std::cmp::min(items.len(), buffer.len());
    for (i, value) in buffer.iter().take(min).enumerate() {
      if *value != items[i] {
        return Ok(MatchResult::Unmatch);
      }
    }
    Ok(if min < items.len() { MatchResult::UnmatchAndCanAcceptMore } else { MatchResult::Match(min) })
  })
}

pub fn one_of<ID, Σ: Symbol + Hash>(items: &[Σ]) -> Syntax<ID, Σ> {
  let label = items.iter().map(|i| Σ::debug_symbol(*i)).collect::<Vec<_>>().join("|");
  let items = items.iter().fold(HashSet::with_capacity(items.len()), |mut items, item| {
    items.insert(*item);
    items
  });
  Syntax::from_fn(&label, move |buffer: &[Σ]| -> Result<Σ, MatchResult> {
    if buffer.is_empty() {
      Ok(MatchResult::UnmatchAndCanAcceptMore)
    } else if items.contains(&buffer[0]) {
      Ok(MatchResult::Match(1))
    } else {
      Ok(MatchResult::Unmatch)
    }
  })
}

pub fn one_of_seqs<ID, Σ: Symbol + PartialEq>(items: &[Vec<Σ>]) -> Syntax<ID, Σ> {
  let label = items.iter().map(|i| Σ::debug_symbols(i)).collect::<Vec<_>>().join("|");
  let items = items.iter().map(|i| i.to_vec()).collect::<Vec<_>>();
  Syntax::from_fn(&label, move |buffer: &[Σ]| -> Result<Σ, MatchResult> {
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
