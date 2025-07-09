use serde::{Serialize, Deserialize};

use super::csv::{ParseCsv, Parsed, CsvError};
use super::ParseError;

use std::str::FromStr;
use std::cmp::{Ord, PartialOrd, Ordering};
use std::convert::TryFrom;
use std::fmt;

/// I don't know what this line does, but my map breaks if I remove it
const HEADER_LINE: &[&str] = &["0", "0", "0", "0", "land", "false", "unknown", "0"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
  /// Province ID; unlikely to go over 100,000
  pub id: u32,
  /// Province color, corresponds with color on `provinces.bmp`
  pub rgb: [u8; 3],
  /// Province type
  pub kind: DefinitionKind,
  /// Whether this province is coastal or not
  pub coastal: bool,
  /// Province terrain, what type of 'biome' this province is; supports custom biomes
  pub terrain: String,
  /// Province continent ID
  pub continent: u16
}

impl ParseCsv for Definition {
  const HEADER_RECORD: Option<&'static [&'static str]> = Some(HEADER_LINE);
  const FOOTER_RECORD: Option<&'static [&'static str]> = None;

  fn deserialize_record(line: csv::StringRecord) -> Result<Self, CsvError> {
    parse_record!(let (
      Parsed(id) => Parsed<u32>,
      Parsed(r) => Parsed<u8>,
      Parsed(g) => Parsed<u8>,
      Parsed(b) => Parsed<u8>,
      Parsed(kind) => Parsed<DefinitionKind>,
      Parsed(Bool(coastal)) => Parsed<Bool>,
      terrain => String,
      Parsed(continent) => Parsed<u16>
    ) = &line);

    Ok(Definition {
      id, rgb: [r, g, b], kind, coastal,
      terrain: terrain.to_lowercase(),
      continent
    })
  }

  fn serialize_record(&self) -> csv::StringRecord {
    csv::StringRecord::from(vec![
      self.id.to_string(),
      self.rgb[0].to_string(),
      self.rgb[1].to_string(),
      self.rgb[2].to_string(),
      self.kind.to_string(),
      self.coastal.to_string(),
      self.terrain.clone(),
      self.continent.to_string()
    ])
  }
}

impl PartialOrd for Definition {
  #[inline]
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(Self::cmp(self, other))
  }
}

impl Ord for Definition {
  #[inline]
  fn cmp(&self, other: &Self) -> Ordering {
    self.kind.cmp(&other.kind)
      .then_with(|| self.continent.cmp(&other.continent))
      .then_with(|| self.terrain.cmp(&other.terrain))
  }
}

impl fmt::Display for Definition {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "{};{};{};{};{};{};{};{}",
      self.id,
      self.rgb[0],
      self.rgb[1],
      self.rgb[2],
      self.kind,
      self.coastal,
      self.terrain,
      self.continent
    )
  }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(into = "&str", try_from = "String")]
pub enum DefinitionKind {
  Land = 0,
  Sea = 1,
  Lake = 2
}

impl DefinitionKind {
  pub fn to_str(self) -> &'static str {
    match self {
      DefinitionKind::Land => "land",
      DefinitionKind::Sea => "sea",
      DefinitionKind::Lake => "lake"
    }
  }
}

impl FromStr for DefinitionKind {
  type Err = ParseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.to_ascii_lowercase().as_str() {
      "land" => Ok(DefinitionKind::Land),
      "sea" => Ok(DefinitionKind::Sea),
      "lake" => Ok(DefinitionKind::Lake),
      _ => Err(ParseError::InvalidDefinitionKind)
    }
  }
}

impl TryFrom<String> for DefinitionKind {
  type Error = ParseError;

  fn try_from(string: String) -> Result<Self, Self::Error> {
    DefinitionKind::from_str(&string)
  }
}

impl From<DefinitionKind> for &'static str {
  fn from(kind: DefinitionKind) -> &'static str {
    kind.to_str()
  }
}

impl fmt::Display for DefinitionKind {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.write_str(self.to_str())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bool(bool);

impl FromStr for Bool {
  type Err = <bool as FromStr>::Err;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "0" => Ok(Bool(false)),
      "1" => Ok(Bool(true)),
      s => s.to_lowercase()
        .parse::<bool>().map(Bool)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{Definition, ParseCsv};

  #[test]
  fn test_definition() {
    const SAMPLE: &str = include_str!("./samples/definition.csv");
    let definition = Definition::read_records(SAMPLE.as_bytes()).unwrap();
    assert_eq!(definition.len(), 13375);
  }
}
