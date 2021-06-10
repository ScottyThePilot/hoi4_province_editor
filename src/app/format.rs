//! Utilities for parsing text-based game data files
mod adjacency;
mod definition;

pub use crate::util::csv::ParseCsv;
pub use self::adjacency::*;
pub use self::definition::*;

use std::fmt;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ParseError;

impl fmt::Display for ParseError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "unable to parse")
  }
}
