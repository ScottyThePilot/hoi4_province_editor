//! CSV parsing and serialization utilities designed specifically for HOI4
use csv::{Reader, ReaderBuilder, Writer, WriterBuilder};
use thiserror::Error;

use std::any::type_name;
use std::collections::VecDeque;
use std::fmt;
use std::error::Error as StdError;
use std::io::prelude::*;
use std::str::FromStr;

fn wrap_reader<R: Read>(inner: R) -> Reader<R> {
  ReaderBuilder::new()
    .delimiter(b';')
    .terminator(csv::Terminator::CRLF)
    .comment(Some(b'#'))
    .quote(b'\"')
    .double_quote(false)
    .trim(csv::Trim::All)
    .has_headers(false)
    .flexible(true)
    .from_reader(inner)
}

fn wrap_writer<W: Write>(inner: W) -> Writer<W> {
  WriterBuilder::new()
    .delimiter(b';')
    .terminator(csv::Terminator::CRLF)
    .comment(Some(b'#'))
    .quote(b'\"')
    .double_quote(false)
    .quote_style(csv::QuoteStyle::Necessary)
    .has_headers(false)
    .flexible(true)
    .from_writer(inner)
}

pub trait ParseCsv: Sized {
  const HEADER_RECORD: Option<&'static [&'static str]>;
  const FOOTER_RECORD: Option<&'static [&'static str]>;

  fn deserialize_record(record: csv::StringRecord) -> Result<Self, CsvError>;

  fn read_records<R: Read>(reader: R) -> Result<Vec<Self>, CsvError> {
    let reader = wrap_reader(reader);
    let mut records = reader.into_records()
      .collect::<Result<VecDeque<_>, _>>()?;

    if let Some(defined_header_record) = Self::HEADER_RECORD {
      if let Some(header_record) = records.pop_front() {
        let all_equal = header_record.iter()
          .zip(defined_header_record.into_iter().copied())
          .all(|(field, defined_field)| str::eq_ignore_ascii_case(field, defined_field));

        if !all_equal {
          return Err(CsvError::InvalidHeaderRecord(header_record, defined_header_record));
        };
      };
    };

    if let Some(defined_footer_record) = Self::FOOTER_RECORD {
      if let Some(footer_record) = records.pop_back() {
        let all_equal = footer_record.iter()
          .zip(defined_footer_record.into_iter().copied())
          .all(|(field, defined_field)| str::eq_ignore_ascii_case(field, defined_field));

        if !all_equal {
          return Err(CsvError::InvalidFooterRecord(footer_record, defined_footer_record));
        };
      };
    };

    let entries = records.into_iter()
      .map(Self::deserialize_record)
      .collect::<Result<Vec<Self>, _>>()?;

    Ok(entries)
  }

  fn serialize_record(&self) -> csv::StringRecord;

  fn write_records<W: Write>(entries: &[Self], writer: W) -> Result<(), CsvError> {
    let mut writer = wrap_writer(writer);

    if let Some(header) = Self::HEADER_RECORD {
      writer.write_record(header)?;
    };

    for entry in entries {
      writer.write_record(entry.serialize_record().iter())?;
    };

    if let Some(footer) = Self::FOOTER_RECORD {
      writer.write_record(footer)?;
    };

    Ok(())
  }
}

#[derive(Error, Debug)]
pub enum CsvError {
  #[error(transparent)]
  Inner(#[from] csv::Error),
  #[error("invalid header record, found {:?}, expected {:?}", DebugList(.0), DebugList(*.1))]
  InvalidHeaderRecord(csv::StringRecord, &'static [&'static str]),
  #[error("invalid footer record, found {:?}, expected {:?}", DebugList(.0), DebugList(*.1))]
  InvalidFooterRecord(csv::StringRecord, &'static [&'static str]),
  #[error("{0} at line {}, byte {}, record {}", .1.line(), .1.byte(), .1.record())]
  ParseErrorAt(CsvParseError, csv::Position),
  #[error("{0}")]
  ParseError(CsvParseError)
}

#[derive(Error, Debug)]
pub enum CsvParseError {
  #[error("failed to parse csv field of type {0} ({1})")]
  ParsingFieldFailed(&'static str, Box<dyn StdError + Sync + Send + 'static>),
  #[error("too few or too many columns were found to field of type {0}")]
  IncorrectColumnCount(&'static str)
}

#[inline]
pub fn parse_field<T: ParseCsvField>(cell: Option<&str>) -> Result<T, CsvParseError> {
  T::parse_from(cell)
}

pub trait ParseCsvField: Sized {
  fn parse_from(cell: Option<&str>) -> Result<Self, CsvParseError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Parsed<T>(pub T);

impl<T> ParseCsvField for Parsed<T>
where T: FromStr, T::Err: StdError + Send + Sync + 'static {
  fn parse_from(cell: Option<&str>) -> Result<Self, CsvParseError> {
    if let Some(cell) = cell {
      match cell.trim().parse::<T>() {
        Ok(cell) => Ok(Parsed(cell)),
        Err(err) => Err(CsvParseError::ParsingFieldFailed(type_name::<Self>(), Box::new(err)))
      }
    } else {
      Err(CsvParseError::IncorrectColumnCount(type_name::<Self>()))
    }
  }
}

impl<T> ParseCsvField for Option<T> where T: ParseCsvField {
  fn parse_from(cell: Option<&str>) -> Result<Self, CsvParseError> {
    match cell {
      Some(cell) => T::parse_from(Some(cell)).map(Some),
      None => Ok(None)
    }
  }
}

impl ParseCsvField for String {
  fn parse_from(cell: Option<&str>) -> Result<Self, CsvParseError> {
    cell.map(str::to_owned).ok_or(CsvParseError::IncorrectColumnCount(type_name::<Self>()))
  }
}

#[inline]
pub fn parse_record<T: ParseCsvRecord>(record: &csv::StringRecord) -> Result<T, CsvError> {
  T::parse_from(record.iter()).map_err(|parse_err| match record.position() {
    Some(position) => CsvError::ParseErrorAt(parse_err, position.clone()),
    None => CsvError::ParseError(parse_err)
  })
}

pub trait ParseCsvRecord: Sized {
  fn parse_from(record: csv::StringRecordIter<'_>) -> Result<Self, CsvParseError>;
}

macro_rules! impl_parse_csv_record_tuple {
  ($($g:ident: $G:ident),* $(,)?) => {
    impl<$($G: ParseCsvField),*> ParseCsvRecord for ($($G,)*) {
      fn parse_from(mut record: csv::StringRecordIter<'_>) -> Result<Self, CsvParseError> {
        $(let $g = parse_field::<$G>(record.next())?;)*
        Ok(($($g,)*))
      }
    }
  };
}

impl_parse_csv_record_tuple!(a: A);
impl_parse_csv_record_tuple!(a: A, b: B);
impl_parse_csv_record_tuple!(a: A, b: B, c: C);
impl_parse_csv_record_tuple!(a: A, b: B, c: C, d: D);
impl_parse_csv_record_tuple!(a: A, b: B, c: C, d: D, e: E);
impl_parse_csv_record_tuple!(a: A, b: B, c: C, d: D, e: E, f: F);
impl_parse_csv_record_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G);
impl_parse_csv_record_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H);
impl_parse_csv_record_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I);
impl_parse_csv_record_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J);
impl_parse_csv_record_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J, k: K);
impl_parse_csv_record_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J, k: K, l: L);
impl_parse_csv_record_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J, k: K, l: L, m: M);
impl_parse_csv_record_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J, k: K, l: L, m: M, n: N);
impl_parse_csv_record_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J, k: K, l: L, m: M, n: N, o: O);
impl_parse_csv_record_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J, k: K, l: L, m: M, n: N, o: O, p: P);

#[macro_export]
macro_rules! parse_record {
  (let ($($pat:pat => $Type:ty),* $(,)?) = $expr:expr) => (
    let ($($pat,)*) = crate::app::format::csv::parse_record::<($($Type,)*)>($expr)?;
  );
}

struct DebugList<I: Copy + IntoIterator<Item = T>, T: fmt::Debug>(I);

impl<I, T> fmt::Debug for DebugList<I, T>
where I: Copy + IntoIterator<Item = T>, T: fmt::Debug {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_list().entries(self.0).finish()
  }
}
