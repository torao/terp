extern crate test;
use terp::parser::{Context, Event};
use terp::schema::json::{schema, ID};

pub const SAMPLE_WIKIPEDIA: &str = r#"
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

#[bench]
fn rfc8259_schema_build(b: &mut test::Bencher) {
  b.iter(|| schema());
}

#[bench]
fn rfc8259_sample_wikipedia(b: &mut test::Bencher) {
  let schema = schema();
  b.iter(|| {
    let event_handler = |_: Event<ID, char>| ();
    let mut parser = Context::new(&schema, ID::JsonText, event_handler).unwrap();
    parser.push_str(SAMPLE_WIKIPEDIA).unwrap();
    parser.finish().unwrap();
  });
}
