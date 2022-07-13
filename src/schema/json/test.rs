use crate::parser::test::Events;
use crate::parser::{Context, Event};
use crate::schema::json::{schema, ID};

const SAMPLE: &str = r#"
{
  "Image": {
      "Width":  800,
      "Height": 600,
      "Title":  "View from 15th Floor",
      "Thumbnail": {
          "Url":    "http://www.example.com/image/481989943",
          "Height": 125,
          "Width":  100
      },
      "Animated" : false,
      "IDs": [116, 943, 234, 38793]
    }
}"#;

#[test]
fn rfc8259_sample() {
  let mut events = Vec::new();
  let event_handler = |e: Event<ID, char>| {
    println!("> {:?}", e);
    events.push(e);
  };
  let schema = self::schema();
  let mut parser = Context::new(&schema, ID::JsonText, event_handler).unwrap();
  for ch in SAMPLE.chars() {
    parser.push(ch).unwrap();
  }
  parser.finish().unwrap();
  println!("{:?}", events);
}

#[test]
fn rfc8259_string() {
  let sample = r#""foo""#;

  let mut events = Vec::new();
  let event_handler = |e: Event<ID, char>| {
    println!("> {:?}", e);
    events.push(e);
  };
  let schema = self::schema();
  let mut parser = Context::new(&schema, ID::String, event_handler).unwrap();
  for ch in sample.chars() {
    parser.push(ch).unwrap();
  }
  parser.finish().unwrap();
  Events::new()
    .begin(ID::String)
    .begin(ID::QuotationMark)
    .fragments("\"")
    .end()
    .begin(ID::Char)
    .begin(ID::Unescaped)
    .fragments("f")
    .end()
    .end()
    .begin(ID::Char)
    .begin(ID::Unescaped)
    .fragments("o")
    .end()
    .end()
    .begin(ID::Char)
    .begin(ID::Unescaped)
    .fragments("o")
    .end()
    .end()
    .begin(ID::QuotationMark)
    .fragments("\"")
    .end()
    .end()
    .assert_eq(&events);
}

/*
extern crate test;

#[bench]
fn rfc8259_schema_build(b: &mut test::Bencher) {
  b.iter(|| self::schema());
}

#[bench]
fn rfc8259_sample_wikipedia(b: &mut test::Bencher) {
  let schema = self::schema();
  b.iter(|| {
    let event_handler = |_: Event<ID, char>| ();
    let mut parser = Context::new(&schema, ID::JsonText, event_handler).unwrap();
    parser.push_str(SAMPLE).unwrap();
    parser.finish().unwrap();
  });
}
*/
