use serde::{Serialize, Deserialize};

use super::csv::{ParseCsv, Parsed, CsvError, CsvLine};
use super::ParseError;

use std::str::FromStr;
use std::cmp::{Ord, PartialOrd, Ordering};
use std::convert::TryFrom;
use std::fmt;

const HEADER_LINE: &str = "From;To;Type;Through;start_x;start_y;stop_x;stop_y;adjacency_rule_name;Comment";

/// I don't know what this line is supposed to do, but every `adjacencies.csv` I've looked at has it
const FOOTER_LINE: &str = "-1;-1;;-1;-1;-1;-1;-1;-1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adjacency {
  /// First province ID
  pub from_id: u32,
  /// Second province ID
  pub to_id: u32,
  /// Adjacency type
  pub kind: AdjacencyKind,
  /// Defines a province that can block this adjacency, optional
  pub through: Option<u32>,
  /// Precise location of the beginning, optional
  pub start: Option<[u32; 2]>,
  /// Precise location of the end, optional
  pub stop: Option<[u32; 2]>,
  /// The name of this adjacency, for use in `adjacency_rules.txt`, optional
  pub rule_name: String,
  /// A comment describing this adjacency, optional
  pub comment: String
}

impl ParseCsv for Adjacency {
  const HEADER_LINE: Option<&'static str> = Some(HEADER_LINE);
  const FOOTER_LINE: Option<&'static str> = Some(FOOTER_LINE);

  fn parse_line(line: CsvLine<'_>) -> Result<Self, CsvError> {
    let (Parsed(from_id), Parsed(to_id), Parsed(kind), Parsed(Num(through)), Parsed(Num(start_x)), Parsed(Num(start_y)), Parsed(Num(stop_x)), Parsed(Num(stop_y)), rule_name, comment) =
      line.parse::<(Parsed<u32>, Parsed<u32>, Parsed<AdjacencyKind>, Parsed<Num>, Parsed<Num>, Parsed<Num>, Parsed<Num>, Parsed<Num>, Option<String>, Option<String>)>()?;

    Ok(Adjacency {
      from_id, to_id, kind, through,
      start: Option::zip(start_x, start_y).map(|(x, y)| [x, y]),
      stop: Option::zip(stop_x, stop_y).map(|(x, y)| [x, y]),
      rule_name: rule_name.map_or_else(String::new, |s| s.to_lowercase()),
      comment: comment.map_or_else(String::new, |s| s.to_lowercase())
    })
  }
}

impl PartialOrd for Adjacency {
  #[inline]
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(Self::cmp(self, other))
  }
}

impl Ord for Adjacency {
  #[inline]
  fn cmp(&self, other: &Self) -> Ordering {
    self.rule_name.cmp(&other.rule_name)
      .then_with(|| self.comment.cmp(&other.comment))
      .then_with(|| self.kind.cmp(&other.kind))
  }
}

impl fmt::Display for Adjacency {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "{};{};{};{};{};{};{};{}",
      self.from_id,
      self.to_id,
      self.kind.to_str(),
      stringify_maybe_num(self.through),
      stringify_maybe_pos(self.start),
      stringify_maybe_pos(self.stop),
      self.rule_name,
      self.comment
    )
  }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(into = "&str", try_from = "String")]
pub enum AdjacencyKind {
  /// An adjacency between two sea provinces that passes through land (canal)
  Land = 0,
  /// Unknown
  River = 1,
  /// Unknown
  LargeRiver = 2,
  /// An adjacency between two land provinces that passes through sea (strait)
  Sea = 3,
  /// An adjacency that prevents passage
  Impassable = 4
}

impl AdjacencyKind {
  pub fn to_str(self) -> &'static str {
    match self {
      AdjacencyKind::Land => "",
      AdjacencyKind::River => "river",
      AdjacencyKind::LargeRiver => "large_river",
      AdjacencyKind::Sea => "sea",
      AdjacencyKind::Impassable => "impassable"
    }
  }
}

impl FromStr for AdjacencyKind {
  type Err = ParseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.to_ascii_lowercase().as_str() {
      "" => Ok(AdjacencyKind::Land),
      "river" => Ok(AdjacencyKind::River),
      "large_river" => Ok(AdjacencyKind::LargeRiver),
      "sea" => Ok(AdjacencyKind::Sea),
      "impassable" => Ok(AdjacencyKind::Impassable),
      _ => Err(ParseError::InvalidAdjacencyKind)
    }
  }
}

impl TryFrom<String> for AdjacencyKind {
  type Error = ParseError;

  fn try_from(string: String) -> Result<Self, Self::Error> {
    AdjacencyKind::from_str(&string)
  }
}

impl From<AdjacencyKind> for &'static str {
  fn from(kind: AdjacencyKind) -> &'static str {
    kind.to_str()
  }
}

impl fmt::Display for AdjacencyKind {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.write_str(self.to_str())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Num(Option<u32>);

impl FromStr for Num {
  type Err = <u32 as FromStr>::Err;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(Num(if s.is_empty() || s == "-1" { None } else { Some(s.parse::<u32>()?) }))
  }
}

fn stringify_maybe_num(num: Option<u32>) -> String {
  num.map_or("-1".to_owned(), |n| n.to_string())
}

fn stringify_maybe_pos(pos: Option<[u32; 2]>) -> String {
  pos.map_or("-1;-1".to_owned(), |[x, y]| format!("{};{}", x, y))
}
