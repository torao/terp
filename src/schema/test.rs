use crate::schema::Item;

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
fn item_for_char_debug_symbols() {
  for (expected, sample) in vec![("", ""), ("ABC", "ABC"), ("A\\tB\\nC\\0", "A\tB\nC\0")].iter() {
    let sample = (*sample).chars().collect::<Vec<_>>();
    assert_eq!(*expected, Item::debug_symbols(&sample));
    assert_eq!(format!("{}...", *expected), Item::debug_symbols_with_ellipsis(&sample, true));
    assert_eq!(format!("{}[EOF]", *expected), Item::debug_symbols_with_ellipsis(&sample, false));
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
      assert_eq!(format!("{b1:02X}{b2:02X}..."), Item::debug_symbols_with_ellipsis(&[b1, b2], true));
      assert_eq!(format!("{b1:02X}{b2:02X}[EOF]"), Item::debug_symbols_with_ellipsis(&[b1, b2], false));
    }
  }
}
