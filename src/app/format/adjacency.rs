use serde::{Serialize, Deserialize};

use crate::util::csv::ParseCsv;
use super::ParseError;

use std::str::FromStr;
use std::num::ParseIntError;
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

impl ParseCsv<10> for Adjacency {
  const HEADER_LINE: Option<&'static str> = Some(HEADER_LINE);
  const FOOTER_LINE: Option<&'static str> = Some(FOOTER_LINE);

  fn parse_line(line: [String; 10]) -> Option<Self> {
    let [from_id, to_id, kind, through, start_x, start_y, stop_x, stop_y, rule_name, comment] = line;

    Some(Adjacency {
      from_id: from_id.parse().ok()?,
      to_id: to_id.parse().ok()?,
      kind: kind.to_lowercase().parse().ok()?,
      through: parse_maybe_num(&through).ok()?,
      start: parse_maybe_pos(&start_x, &start_y).ok()?,
      stop: parse_maybe_pos(&stop_x, &stop_y).ok()?,
      rule_name,
      comment
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
  Land = 0,
  River = 1,
  LargeRiver = 2,
  Sea = 3,
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
    match s {
      "" => Ok(AdjacencyKind::Land),
      "river" => Ok(AdjacencyKind::River),
      "large_river" => Ok(AdjacencyKind::LargeRiver),
      "sea" => Ok(AdjacencyKind::Sea),
      "impassable" => Ok(AdjacencyKind::Impassable),
      _ => Err(ParseError)
    }
  }
}

impl TryFrom<String> for AdjacencyKind {
  type Error = ParseError;

  fn try_from(string: String) -> Result<Self, Self::Error> {
    AdjacencyKind::from_str(&string)
  }
}

impl Into<&'static str> for AdjacencyKind {
  fn into(self) -> &'static str {
    self.to_str()
  }
}

impl fmt::Display for AdjacencyKind {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.to_str())
  }
}

fn parse_maybe_num(n: &str) -> Result<Option<u32>, ParseIntError> {
  Ok(if n == "-1" { None } else { Some(n.parse::<u32>()?) })
}

fn stringify_maybe_num(num: Option<u32>) -> String {
  num.map_or("-1".to_owned(), |n| n.to_string())
}

fn parse_maybe_pos(x: &str, y: &str) -> Result<Option<[u32; 2]>, ParseIntError> {
  if x == "-1" || y == "-1" {
    Ok(None)
  } else {
    let x = x.parse::<u32>()?;
    let y = y.parse::<u32>()?;
    Ok(Some([x, y]))
  }
}

fn stringify_maybe_pos(pos: Option<[u32; 2]>) -> String {
  pos.map_or("-1;-1".to_owned(), |[x, y]| format!("{};{}", x, y))
}
