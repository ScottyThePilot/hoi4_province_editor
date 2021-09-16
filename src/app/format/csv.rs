//! CSV parsing and serialization utilities designed specifically for HOI4
use thiserror::Error;

use std::error::Error as StdError;
use std::io::prelude::*;
use std::io;

pub const LINE_BREAK: &str = "\r\n";
pub const SEPARATOR: char = ';';

fn split_line<const COLUMNS: usize>(line: &str) -> Option<[String; COLUMNS]> {
  use std::convert::TryInto;
  let line = line.split(SEPARATOR)
    .map(str::to_owned)
    .take(COLUMNS)
    .collect::<Vec<String>>();
  line.try_into().ok()
}

fn should_ignore_line<P: ParseCsv<COLUMNS>, const COLUMNS: usize>(s: &str, i: usize) -> bool {
  s.is_empty() || s.starts_with('#') || (i == 0 && P::HEADER_LINE.is_some()) || Some(s) == P::HEADER_LINE || Some(s) == P::FOOTER_LINE
}

pub trait ParseCsv<const COLUMNS: usize>: Sized + ToString {
  const HEADER_LINE: Option<&'static str>;
  const FOOTER_LINE: Option<&'static str>;

  type Error: StdError;

  fn parse_line(line: [String; COLUMNS]) -> Result<Self, Self::Error>;

  fn parse_all<R: BufRead>(reader: R) -> Result<Vec<Self>, CsvError<Self::Error>> {
    let mut out = Vec::with_capacity(COLUMNS);
    for (i, raw_line) in reader.lines().enumerate() {
      let raw_line = raw_line?;
      if should_ignore_line::<Self, COLUMNS>(&raw_line, i) {
        continue;
      } else {
        let line = split_line::<COLUMNS>(&raw_line)
          .ok_or(CsvError::IncorrectColumnCount)?;
        let line = Self::parse_line(line)
          .map_err(|err| CsvError::ParsingFailed(raw_line, err))?;
        out.push(line);
      };
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
pub enum CsvError<E: StdError> {
  #[error("{0}")]
  IoError(#[from] io::Error),
  #[error("failed to parse csv line ({0}): {1}")]
  ParsingFailed(String, E),
  #[error("too few or too many columns were found")]
  IncorrectColumnCount
}
