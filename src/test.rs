use crate::schema::chars::Location;
use crate::Error;

#[test]
fn error_attributes() {
  let err = Error::<char>::Unmatched {
    location: Location::default(),
    prefix: String::default(),
    expecteds: Vec::default(),
    expected_syntaxes: Vec::default(),
    actual: String::default(),
  };
  let _ = format!("{:?}", err);
  let _ = format!("{}", err);
  assert_eq!(err, err);
  assert!(err.eq(&err));
  assert!(!err.ne(&err));
}
