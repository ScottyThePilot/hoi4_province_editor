use util_macros::error_enum;

error_enum!{
  pub enum Error {
    Io(std::io::Error),
    Zip(zip::result::ZipError),
    Image(image::ImageError),
    Csv(crate::util::csv::CsvError),
    Custom(String)
  }
}

impl From<&str> for Error {
  fn from(s: &str) -> Error {
    Error::Custom(s.to_owned())
  }
}

pub type Result<T = ()> = std::result::Result<T, Error>;
