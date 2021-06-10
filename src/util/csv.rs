//! CSV parsing and serialization utilities designed specifically for HOI4
use std::io::prelude::*;
use std::io;
use std::fmt;

pub const LINE_BREAK: &str = "\r\n";
pub const SEPARATOR: char = ';';

fn split_line<const COLUMNS: usize>(line: &str) -> Result<[String; COLUMNS], CsvError> {
  use std::convert::TryInto;
  let line = line.split(SEPARATOR)
    .map(str::to_owned)
    .collect::<Vec<String>>();
  match line.try_into() {
    Ok(line) => Ok(line),
    Err(_) => Err(CsvError::IncorrectColumnCount)
  }
}

fn should_ignore_line<P: ParseCsv<COLUMNS>, const COLUMNS: usize>(s: &str, i: usize) -> bool {
  i == 0 || s.is_empty() || Some(s) == P::HEADER_LINE || Some(s) == P::FOOTER_LINE
}

pub trait ParseCsv<const COLUMNS: usize>: Sized + ToString {
  const HEADER_LINE: Option<&'static str>;
  const FOOTER_LINE: Option<&'static str>;

  fn parse_line(line: [String; COLUMNS]) -> Option<Self>;

  fn parse_all<R: BufRead>(reader: R) -> Result<Vec<Self>, CsvError> {
    let mut out = Vec::with_capacity(COLUMNS);
    for (i, raw_line) in reader.lines().enumerate() {
      let raw_line = raw_line?;
      if should_ignore_line::<Self, COLUMNS>(&raw_line, i) {
        continue;
      } else {
        let line = split_line::<COLUMNS>(&raw_line)?;
        let line = Self::parse_line(line)
          .ok_or(CsvError::ParsingFailed(raw_line))?;
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

#[derive(Debug)]
pub enum CsvError {
  IoError(io::Error),
  ParsingFailed(String),
  IncorrectColumnCount
}

impl fmt::Display for CsvError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      CsvError::IoError(err) => fmt::Display::fmt(err, f),
      CsvError::ParsingFailed(line) => write!(f, "failed to parse csv line: {:?}", line),
      CsvError::IncorrectColumnCount => write!(f, "too few or too many columns were found")
    }
  }
}

impl std::error::Error for CsvError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      CsvError::IoError(err) => err.source(),
      CsvError::ParsingFailed(_) => None,
      CsvError::IncorrectColumnCount => None
    }
  }
}

impl From<io::Error> for CsvError {
  fn from(err: io::Error) -> CsvError {
    CsvError::IoError(err)
  }
}
