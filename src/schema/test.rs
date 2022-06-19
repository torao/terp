use crate::schema::Item;

#[test]
fn item_for_char_to_single_debug() {
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
    assert_eq!(*expected, Item::to_single_debug(*sample));
  }
}

#[test]
fn item_for_char_to_sampling_debug() {
  for (expected, sample) in vec![("", ""), ("ABC", "ABC"), ("A\\tB\\nC\\0", "A\tB\nC\0")].iter() {
    assert_eq!(format!("{}...", *expected), Item::to_sampling_debug((*sample).chars().collect::<Vec<_>>(), false));
    assert_eq!(format!("{}[EOF]", *expected), Item::to_sampling_debug((*sample).chars().collect::<Vec<_>>(), true));
  }
}

#[test]
fn item_for_u8_to_single_debug() {
  for b in 0u8..=0xFFu8 {
    assert_eq!(format!("{b:02X}"), Item::to_single_debug(b));
  }
}

#[test]
fn item_for_u8_to_sampling_debug() {
  for b1 in 0u8..=0xFFu8 {
    for b2 in 0u8..=0xFFu8 {
      assert_eq!(format!("{b1:02X}{b2:02X}..."), Item::to_sampling_debug(vec![b1, b2], false));
      assert_eq!(format!("{b1:02X}{b2:02X}[EOF]"), Item::to_sampling_debug(vec![b1, b2], true));
    }
  }
}
