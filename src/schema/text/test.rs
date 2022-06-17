use crate::schema::text::BufferInputSource;

use super::ascii_digit;

#[tokio::test]
async fn ascii_digit_range() {
  let digit = ascii_digit();

  let is = BufferInputSource::from_string("0");
  assert!(digit.parse(is).await.unwrap());

  let is = BufferInputSource::from_string("A");
  assert!(!digit.parse(is).await.unwrap());
}
