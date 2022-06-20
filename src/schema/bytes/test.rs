use crate::schema::bytes::Location;
use crate::schema::Location as L;

#[test]
#[allow(clippy::clone_on_copy)]
fn bytes_location() {
  let mut l = Location::default();
  assert!(matches!(l, Location(0)));
  l.increment_with(0u8);
  assert!(matches!(l, Location(1)));
  l.increment_with(1u8);
  assert!(matches!(l, Location(2)));
  assert_eq!("@2", l.to_string());

  let _ = format!("{:?}", l);
  let l2 = l;
  assert_eq!(l.0, l2.0);
  assert_eq!(&l.0, &l.clone().0);
}
