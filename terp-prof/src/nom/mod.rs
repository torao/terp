use nom::branch::*;
use nom::bytes::complete::tag;
use nom::character::complete::*;
use nom::combinator::*;
use nom::error::VerboseError;
use nom::multi::*;
use nom::IResult;

type Result<'a> = IResult<&'a str, (), VerboseError<&'a str>>;

#[cfg(test)]
mod test;

pub fn hex_dig(input: &str) -> Result {
  one_of("0123456789abcdefABCDEF")(input).map(|(i, _)| (i, ()))
}

pub fn digit(input: &str) -> Result {
  one_of("0123456789")(input).map(|(i, _)| (i, ()))
}

pub fn digit1_9(input: &str) -> Result {
  one_of("123456789")(input).map(|(i, _)| (i, ()))
}

pub fn unescaped(input: &str) -> Result {
  none_of(('\x00'..'\x20').chain('\x22'..'\x23').chain('\x5C'..'\x5D').collect::<String>().as_str())(input)
    .map(|(i, _)| (i, ()))
}

pub fn quoteting_mark(input: &str) -> Result {
  char('\"')(input).map(|(i, _)| (i, ()))
}

pub fn escape(input: &str) -> Result {
  char('\\')(input).map(|(i, _)| (i, ()))
}

pub fn _char(input: &str) -> Result {
  let escaped1 = map_res(one_of("\"\\/bfnrt"), |_| Ok::<(), ()>(()));
  let escaped2 = map_res(permutation((char('u'), count(hex_dig, 4))), |_| Ok::<(), ()>(()));
  let escaped = map_res(permutation((escape, alt((escaped1, escaped2)))), |_| Ok::<(), ()>(()));
  alt((unescaped, escaped))(input).map(|(i, _)| (i, ()))
}

pub fn string(input: &str) -> Result {
  let (input, _) = quoteting_mark(input)?;
  let (input, _) = many0(_char)(input)?;
  quoteting_mark(input)
}

pub fn zero(input: &str) -> Result {
  char('0')(input).map(|(i, _)| (i, ()))
}

pub fn plus(input: &str) -> Result {
  char('+')(input).map(|(i, _)| (i, ()))
}

pub fn minus(input: &str) -> Result {
  char('-')(input).map(|(i, _)| (i, ()))
}

pub fn decimal_point(input: &str) -> Result {
  char('.')(input).map(|(i, _)| (i, ()))
}

pub fn e(input: &str) -> Result {
  one_of("eE")(input).map(|(i, _)| (i, ()))
}

pub fn int(input: &str) -> Result {
  let many_digits = map_res(many0(digit), |_| Ok::<(), ()>(()));
  let digits = map_res(permutation((digit1_9, many_digits)), |_| Ok::<(), ()>(()));
  alt((zero, digits))(input).map(|(i, _)| (i, ()))
}

pub fn frac(input: &str) -> Result {
  let (input, _) = decimal_point(input)?;
  many1(digit)(input).map(|(i, _)| (i, ()))
}

pub fn exp(input: &str) -> Result {
  let (input, _) = e(input)?;
  let (input, _) = opt(alt((minus, plus)))(input)?;
  many1(digit)(input).map(|(i, _)| (i, ()))
}

pub fn number(input: &str) -> Result {
  let (input, _) = opt(minus)(input)?;
  let (input, _) = int(input)?;
  let (input, _) = opt(frac)(input)?;
  opt(exp)(input).map(|(i, _)| (i, ()))
}

pub fn ws(input: &str) -> Result {
  many0(one_of(" \t\x0A\x0D"))(input).map(|(i, _)| (i, ()))
}

pub fn _false(input: &str) -> Result {
  tag("false")(input).map(|(i, _)| (i, ()))
}

pub fn _true(input: &str) -> Result {
  tag("true")(input).map(|(i, _)| (i, ()))
}

pub fn null(input: &str) -> Result {
  tag("null")(input).map(|(i, _)| (i, ()))
}

pub fn value(input: &str) -> Result {
  alt((_false, null, _true, object, array, number, string))(input).map(|(i, _)| (i, ()))
}

pub fn begin_array(input: &str) -> Result {
  let (input, _) = ws(input)?;
  let (input, _) = char('[')(input)?;
  ws(input)
}

pub fn end_array(input: &str) -> Result {
  let (input, _) = ws(input)?;
  let (input, _) = char(']')(input)?;
  ws(input)
}

pub fn value_separator(input: &str) -> Result {
  let (input, _) = ws(input)?;
  let (input, _) = char(',')(input)?;
  ws(input)
}

pub fn array(input: &str) -> Result {
  let (input, _) = begin_array(input)?;
  let (input, _) = opt(permutation((value, many0(permutation((value_separator, value))))))(input)?;
  end_array(input)
}

pub fn begin_object(input: &str) -> Result {
  let (input, _) = ws(input)?;
  let (input, _) = char('{')(input)?;
  ws(input)
}

pub fn end_object(input: &str) -> Result {
  let (input, _) = ws(input)?;
  let (input, _) = char('}')(input)?;
  ws(input)
}

pub fn name_separator(input: &str) -> Result {
  let (input, _) = ws(input)?;
  let (input, _) = char(':')(input)?;
  ws(input)
}

pub fn member(input: &str) -> Result {
  let (input, _) = string(input)?;
  let (input, _) = name_separator(input)?;
  value(input)
}

pub fn object(input: &str) -> Result {
  permutation((begin_object, opt(permutation((member, many0(permutation((value_separator, member)))))), end_object))(
    input,
  )
  .map(|(i, _)| (i, ()))
}

pub fn json_text(input: &str) -> Result {
  let (input, _) = ws(input)?;
  let (input, _) = value(input)?;
  ws(input)
}
