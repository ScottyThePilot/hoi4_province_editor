//! CSV parsing and serialization utilities designed specifically for HOI4
use thiserror::Error;

use std::error::Error as StdError;
use std::io::prelude::*;
use std::io;
use std::str::FromStr;

pub const LINE_BREAK: &str = "\r\n";
pub const SEPARATOR: char = ';';

#[derive(Debug, Clone)]
pub struct CsvLine<'a> {
  index: usize,
  line: &'a str,
  iter: std::str::Split<'a, char>
}

impl<'a> CsvLine<'a> {
  pub fn new(line: &'a str, index: usize) -> Self {
    CsvLine { index, line, iter: line.split(SEPARATOR) }
  }

  pub fn next_parse<T: ParseCsvCell>(&mut self) -> Result<T, CsvError> {
    T::parse_from(self.next()).map_err(|err| match err {
      CsvError::IncorrectColumnCount => CsvError::IncorrectColumnCountAt(self.index),
      CsvError::ParsingCellFailed(err) => CsvError::ParsingLineFailed(err, self.line.to_owned()),
      err => err
    })
  }

  pub fn parse<T: ParseCsvLine>(self) -> Result<T, CsvError> {
    T::parse_from(self)
  }
}

impl<'a> Iterator for CsvLine<'a> {
  type Item = &'a str;

  fn next(&mut self) -> Option<Self::Item> {
    self.iter.next()
  }

  fn fold<A, F>(self, init: A, f: F) -> A
  where Self: Sized, F: FnMut(A, Self::Item) -> A, {
    self.iter.fold(init, f)
  }
}

fn should_ignore_line(line: &str) -> bool {
  line.is_empty() || line.starts_with('#')
}

fn lines(reader: impl BufRead) -> impl Iterator<Item = Result<String, io::Error>> {
  reader.lines().filter_map(|result| {
    result.map(|line| (!should_ignore_line(&line)).then_some(line)).transpose()
  })
}

pub trait ParseCsv: Sized + ToString {
  const HEADER_LINE: Option<&'static str>;
  const FOOTER_LINE: Option<&'static str>;

  fn parse_line(line: CsvLine<'_>) -> Result<Self, CsvError>;

  fn parse_all<R: BufRead>(reader: R) -> Result<Vec<Self>, CsvError> {
    let mut out = Vec::new();
    let mut iter = lines(reader).enumerate().peekable();
    while let Some((i, raw_line)) = iter.next() {
      let is_first = i == 0;
      let is_last = iter.peek().is_none();

      let raw_line = raw_line.map_err(CsvError::IoError)?;
      if Self::HEADER_LINE.is_some() && is_first { continue };
      if Self::FOOTER_LINE.is_some() && is_last { continue };

      let line = Self::parse_line(CsvLine::new(&raw_line, i))?;
      out.push(line);
    };

    Ok(out)
  }

  fn stringify_line(&self) -> String {
    let mut line = self.to_string();
    line.push_str(LINE_BREAK);
    line
  }

  fn stringify_all(entries: &[Self]) -> String {
    let mut out = String::new();

    if let Some(header) = Self::HEADER_LINE {
      out.push_str(header);
      out.push_str(LINE_BREAK);
    };

    for entry in entries {
      out.push_str(&entry.stringify_line());
    };

    if let Some(footer) = Self::FOOTER_LINE {
      out.push_str(footer);
      out.push_str(LINE_BREAK);
    };

    out
  }
}

#[derive(Error, Debug)]
pub enum CsvError {
  #[error("{0}")]
  IoError(#[from] io::Error),
  #[error("failed to parse csv cell: {0}")]
  ParsingCellFailed(Box<dyn StdError + Sync + Send + 'static>),
  #[error("failed to parse csv line ({1:?}): {0}")]
  ParsingLineFailed(Box<dyn StdError + Sync + Send + 'static>, String),
  #[error("too few or too many columns were found")]
  IncorrectColumnCount,
  #[error("too few or too many columns were found at row {0}")]
  IncorrectColumnCountAt(usize)
}

pub trait ParseCsvCell: Sized {
  fn parse_from(cell: Option<&str>) -> Result<Self, CsvError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Parsed<T>(pub T);

impl<T> ParseCsvCell for Parsed<T>
where T: FromStr, T::Err: StdError + Send + Sync + 'static {
  fn parse_from(cell: Option<&str>) -> Result<Self, CsvError> {
    if let Some(cell) = cell {
      match cell.parse::<T>() {
        Ok(cell) => Ok(Parsed(cell)),
        Err(err) => Err(CsvError::ParsingCellFailed(Box::new(err)))
      }
    } else {
      Err(CsvError::IncorrectColumnCount)
    }
  }
}

impl<T> ParseCsvCell for Option<T> where T: ParseCsvCell {
  fn parse_from(cell: Option<&str>) -> Result<Self, CsvError> {
    match cell {
      Some(cell) => T::parse_from(Some(cell)).map(Some),
      None => Ok(None)
    }
  }
}

impl ParseCsvCell for String {
  fn parse_from(cell: Option<&str>) -> Result<Self, CsvError> {
    cell.map(str::to_owned).ok_or(CsvError::IncorrectColumnCount)
  }
}

pub trait ParseCsvLine: Sized {
  fn parse_from(line: CsvLine<'_>) -> Result<Self, CsvError>;
}

macro_rules! impl_parse_csv_line_tuple {
  ($($g:ident: $G:ident),* $(,)?) => {
    impl<$($G: ParseCsvCell),*> ParseCsvLine for ($($G,)*) {
      fn parse_from(mut line: CsvLine<'_>) -> Result<Self, CsvError> {
        $(let $g = line.next_parse::<$G>()?;)*
        Ok(($($g,)*))
      }
    }
  };
}

impl_parse_csv_line_tuple!(a: A);
impl_parse_csv_line_tuple!(a: A, b: B);
impl_parse_csv_line_tuple!(a: A, b: B, c: C);
impl_parse_csv_line_tuple!(a: A, b: B, c: C, d: D);
impl_parse_csv_line_tuple!(a: A, b: B, c: C, d: D, e: E);
impl_parse_csv_line_tuple!(a: A, b: B, c: C, d: D, e: E, f: F);
impl_parse_csv_line_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G);
impl_parse_csv_line_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H);
impl_parse_csv_line_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I);
impl_parse_csv_line_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J);
impl_parse_csv_line_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J, k: K);
impl_parse_csv_line_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J, k: K, l: L);
impl_parse_csv_line_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J, k: K, l: L, m: M);
impl_parse_csv_line_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J, k: K, l: L, m: M, n: N);
impl_parse_csv_line_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J, k: K, l: L, m: M, n: N, o: O);
impl_parse_csv_line_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J, k: K, l: L, m: M, n: N, o: O, p: P);
