use thiserror::Error;

use crate::util::csv::CsvError;
use crate::app::format::ParseError;

#[derive(Debug, Error)]
pub enum Error {
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[error(transparent)]
  Zip(#[from] zip::result::ZipError),
  #[error(transparent)]
  Image(#[from] image::ImageError),
  #[error(transparent)]
  Csv(#[from] CsvError<ParseError>),
  #[error("{0}")]
  Custom(String)
}

impl From<String> for Error {
  fn from(s: String) -> Error {
    Error::Custom(s)
  }
}

impl From<&str> for Error {
  fn from(s: &str) -> Error {
    Error::Custom(s.to_owned())
  }
}
