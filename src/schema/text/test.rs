use crate::schema::text::BytesInputSource;

use super::ascii_digit;

#[tokio::test]
async fn ascii_digit_range() {
  let digit = ascii_digit();

  let is = BytesInputSource::from_string("0");
  digit.parse(is).await.unwrap();

  let is = BytesInputSource::from_string("A");
  digit.parse(is).await.unwrap();
}

#[tokio::test]
async fn bytes_stream_source_read() {
  for expected in vec![""].iter() {
    let mut is = BytesInputSource::from_string(expected);
  }
}
