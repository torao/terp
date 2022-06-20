use crate::schema::Schema;

use super::ascii_digit;

#[test]
fn schema_display() {
  let schema = Schema::new().define("D", ascii_digit()).define("DN", ascii_digit() * (1..=100));
  assert_eq!("D := '0'..='9'\nDN := '0'..='9' * 1..=100\n", schema.to_string());
}
