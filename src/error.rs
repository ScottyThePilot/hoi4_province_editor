use thiserror::Error;

use crate::app::format::CsvError;
use crate::config::LoadConfigError;

#[derive(Debug, Error)]
pub enum Error {
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[error(transparent)]
  Zip(#[from] zip::result::ZipError),
  #[error(transparent)]
  Image(#[from] image::ImageError),
  #[error("csv parse error ({1}): {0}")]
  Csv(CsvError, &'static str),
  #[error("config error: {0}")]
  ConfigError(#[from] LoadConfigError),
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
