use crate::schema::chars::Location;
use crate::schema::Location as L;
use crate::Error;

#[test]
fn error_attributes() {
  let err =
    Error::<char>::Unmatched { location: Location::default(), expected: String::default(), actual: String::default() };
  let _ = format!("{:?}", err);
  let _ = format!("{}", err);
  assert_eq!(err, err);
  assert!(err.eq(&err));
  assert!(!err.ne(&err));
}

#[test]
fn error_multiple() {
  let location = Location::default();
  let err1 = Error::<char>::Unmatched { location, expected: String::default(), actual: String::default() };
  assert!(matches!(Error::errors::<char>(vec![err1]), Err(Error::Unmatched { .. })));

  let mut location = Location::default();
  let err1 = Error::<char>::Unmatched { location, expected: String::default(), actual: String::default() };
  location.increment_with_seq(&"hello\nworld".chars().collect::<Vec<_>>());
  let err2 = Error::<char>::Unmatched { location, expected: String::default(), actual: String::default() };
  assert!(matches!(Error::errors::<char>(vec![err1, err2]), Err(Error::Multi(..))));
}
