use fxhash::FxHashMap;
use itertools::Itertools;
use serde::{Serialize, Deserialize};
use serde_multi::formats::toml;
use thiserror::Error;

use crate::app::map::Color;
use crate::app::map::ProvinceKind;
use crate::app::format::DefinitionKind;
use crate::util::fx_hash_map_with_capacity;

use std::fs;

const DEFAULT_CONFIG: &[u8] = include_bytes!("../assets/hoi4pe_config_default.toml");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
  #[serde(rename = "max-undo-states", alias = "max_undo_states")]
  pub max_undo_states: usize,
  #[serde(rename = "preserve-ids", alias = "preserve_ids")]
  pub preserve_ids: bool,
  #[serde(rename = "terrain", skip_serializing_if = "FxHashMap::is_empty")]
  pub terrains: FxHashMap<String, Terrain>
}

impl Config {
  pub fn load() -> Result<Config, LoadConfigError> {
    use std::io::ErrorKind;
    let mut config = match fs::read("hoi4pe_config.toml") {
      Ok(data) => toml::from_slice::<Config>(&data)?,
      Err(err) if err.kind() == ErrorKind::NotFound => {
        fs::write("hoi4pe_config.toml", DEFAULT_CONFIG)?;
        Config::default()
      },
      Err(err) => return Err(err.into())
    };

    add_default_terrains(&mut config.terrains);
    Ok(config)
  }

  pub fn terrain_color(&self, terrain: &str) -> Option<Color> {
    self.terrains.get(terrain).map(|t| t.color)
  }

  pub fn terrain_kind(&self, terrain: &str) -> Option<ProvinceKind> {
    self.terrains.get(terrain).map(|t| t.kind)
  }

  pub fn cycle_kinds<P>(&self, kind: Option<P>) -> DefinitionKind
  where P: Into<ProvinceKind> {
    match kind.map(P::into) {
      Some(ProvinceKind::Land) => DefinitionKind::Sea,
      Some(ProvinceKind::Sea) => DefinitionKind::Lake,
      Some(ProvinceKind::Lake) => DefinitionKind::Land,
      Some(ProvinceKind::Unknown) | None => DefinitionKind::Land
    }
  }

  pub fn cycle_terrains(&self, terrain: Option<&str>) -> String {
    if let Some(target_terrain) = terrain {
      for (terrain, next_terrain) in self.terrains.keys().tuple_windows() {
        if terrain == target_terrain {
          return next_terrain.clone();
        };
      };
    };

    self.terrains.keys().next()
      .expect("infallible")
      .clone()
  }

  pub fn cycle_continents(&self, continent: Option<u16>) -> u16 {
    continent.map_or(0, |continent| (continent + 1) % 16)
  }

  pub fn kind_color(&self, kind: impl Into<ProvinceKind>) -> Color {
    match kind.into() {
      ProvinceKind::Land => [0x0a, 0xae, 0x3d],
      ProvinceKind::Sea => [0x00, 0x4c, 0x9e],
      ProvinceKind::Lake => [0x24, 0xab, 0xff],
      ProvinceKind::Unknown => [0x22, 0x22, 0x22]
    }
  }

  pub fn coastal_color(&self, coastal: Option<bool>, kind: impl Into<ProvinceKind>) -> Color {
    match (coastal, kind.into()) {
      (Some(false), ProvinceKind::Land) => [0x00, 0x33, 0x11],
      (Some(true), ProvinceKind::Land) => [0x33, 0x99, 0x55],
      (Some(false), ProvinceKind::Sea) => [0x00, 0x11, 0x33],
      (Some(true), ProvinceKind::Sea) => [0x33, 0x55, 0x99],
      (Some(false), ProvinceKind::Lake) => [0x00, 0x33, 0x33],
      (Some(true), ProvinceKind::Lake) => [0x33, 0x99, 0x99],
      _ => [0x22, 0x22, 0x22]
    }
  }

  pub fn default_terrain(&self, kind: impl Into<ProvinceKind>) -> String {
    match kind.into() {
      ProvinceKind::Unknown => "unknown".to_owned(),
      ProvinceKind::Land => "plains".to_owned(),
      ProvinceKind::Sea => "ocean".to_owned(),
      ProvinceKind::Lake => "lakes".to_owned()
    }
  }
}

impl Default for Config {
  fn default() -> Config {
    Config {
      max_undo_states: 24,
      preserve_ids: false,
      terrains: default_terrains()
    }
  }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Terrain {
  #[serde(alias = "colour")]
  pub color: Color,
  #[serde(rename = "type")]
  pub kind: ProvinceKind
}

#[derive(Error, Debug)]
pub enum LoadConfigError {
  #[error(transparent)]
  IoError(#[from] std::io::Error),
  #[error(transparent)]
  FormatError(#[from] serde_multi::Error),
  #[error("{0}")]
  Custom(String)
}

fn default_terrains() -> FxHashMap<String, Terrain> {
  let mut terrains = fx_hash_map_with_capacity(DEFAULT_TERRAINS.len());
  for &(color, name, kind) in DEFAULT_TERRAINS {
    terrains.insert(name.to_owned(), Terrain {  color, kind });
  };

  terrains
}

fn add_default_terrains(terrains: &mut FxHashMap<String, Terrain>) {
  // The 'unknown' terrain should not be user-overloadable
  terrains.remove("unknown");
  for &(color, name, kind) in DEFAULT_TERRAINS {
    let terrain = Terrain { color, kind };
    terrains.entry(name.to_owned())
      .or_insert(terrain);
  };
}

const DEFAULT_TERRAINS: &[(Color, &str, ProvinceKind)] = &[
  ([0x00, 0x00, 0x00], "unknown", ProvinceKind::Unknown),
  ([0xff, 0x81, 0x42], "plains", ProvinceKind::Land),
  ([0xff, 0x3f, 0x00], "desert", ProvinceKind::Land),
  ([0x59, 0xc7, 0x55], "forest", ProvinceKind::Land),
  ([0xf8, 0xff, 0x99], "hills", ProvinceKind::Land),
  ([0x7f, 0xbf, 0x00], "jungle", ProvinceKind::Land),
  ([0x4c, 0x60, 0x23], "marsh", ProvinceKind::Land),
  ([0x7c, 0x87, 0x7d], "mountain", ProvinceKind::Land),
  ([0x00, 0xff, 0xff], "lakes", ProvinceKind::Lake),
  ([0x00, 0x00, 0xff], "ocean", ProvinceKind::Sea),
  ([0x9b, 0x00, 0xff], "urban", ProvinceKind::Land)
];
