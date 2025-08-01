//! Utilities for parsing text-based game data files
#[macro_use]
mod csv;
mod adjacency;
mod definition;

use thiserror::Error;

pub use self::csv::{ParseCsv, CsvError};
pub use self::adjacency::*;
pub use self::definition::*;

use std::num::ParseIntError;
use std::str::ParseBoolError;

#[derive(Error, Debug, Clone, Eq, PartialEq)]
pub enum ParseError {
  #[error("expected one of \"river\", \"large_river\", \"sea\", \"impassable\", or an empty string")]
  InvalidAdjacencyKind,
  #[error("expected one of \"land\", \"sea\", or \"lake\"")]
  InvalidDefinitionKind,
  #[error("{0}")]
  ParseIntError(#[from] ParseIntError),
  #[error("{0}")]
  ParseBoolError(#[from] ParseBoolError)
}
