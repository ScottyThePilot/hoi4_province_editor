use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[error(transparent)]
  Zip(#[from] zip::result::ZipError),
  #[error(transparent)]
  Image(#[from] image::ImageError),
  #[error(transparent)]
  Csv(#[from] crate::util::csv::CsvError),
  #[error("{0}")]
  Custom(String)
}

impl From<String> for Error {
  fn from(s: String) -> Error {
    Error::Custom(s.to_owned())
  }
}

impl From<&str> for Error {
  fn from(s: &str) -> Error {
    Error::Custom(s.to_owned())
  }
}

pub type Result<T = ()> = std::result::Result<T, Error>;
