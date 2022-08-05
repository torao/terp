use nom::error::convert_error;
use nom::Finish;

#[test]
fn json_text() {
  for sample in vec![
    "true",
    "\"\"",
    "0",
    "1234567",
    r#"{
    "Image": {
      "Width": 800,
      "Height": 600
    }
}"#,
    r#"{
  "Image": {
      "Width": 800,
      "Height": 600,
      "Title": "View from 15th Floor",
      "Thumbnail": {
          "Url": "http://www.example.com/image/481989943",
          "Height": 125,
          "Width": 100
      },
      "Animated": false,
      "IDs": [
          116,
          943,
          234,
          38793
      ]
  }
}"#,
  ] {
    if let Err(err) = super::json_text(sample).finish() {
      panic!("{}", convert_error(sample, err));
    }
  }
}
