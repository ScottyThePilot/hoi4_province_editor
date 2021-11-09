use fxhash::FxHashMap;
use itertools::Itertools;
use serde::Deserialize;
use thiserror::Error;

use crate::app::map::Color;
use crate::app::map::ProvinceKind;
use crate::util::fx_hash_map_with_capacity;

use std::fs;

const DEFAULT_CONFIG: &[u8] = include_bytes!("../assets/hoi4pe_config_default.toml");

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
  pub max_undo_states: usize,
  pub preserve_ids: bool,
  pub change_view_mode_on_undo: bool,
  pub generate_coastal_on_save: bool,
  #[serde(alias = "terrain")]
  pub terrains: FxHashMap<String, Terrain>,
  pub extra_warnings: ExtraWarnings
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
}

impl Default for Config {
  fn default() -> Config {
    Config {
      max_undo_states: 24,
      preserve_ids: false,
      change_view_mode_on_undo: true,
      generate_coastal_on_save: false,
      terrains: default_terrains(),
      extra_warnings: ExtraWarnings {
        enabled: false,
        lone_pixels: false,
        few_shared_borders: false,
        few_shared_borders_threshold: 4
      }
    }
  }
}

#[derive(Debug, Copy, Clone, Deserialize)]
pub struct Terrain {
  #[serde(alias = "colour")]
  pub color: Color,
  #[serde(rename = "type")]
  pub kind: ProvinceKind
}

#[derive(Debug, Copy, Clone, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ExtraWarnings {
  #[serde(skip_deserializing)]
  pub enabled: bool,
  pub lone_pixels: bool,
  pub few_shared_borders: bool,
  pub few_shared_borders_threshold: usize
}

impl Default for ExtraWarnings {
  fn default() -> ExtraWarnings {
    ExtraWarnings {
      enabled: true,
      lone_pixels: true,
      few_shared_borders: true,
      few_shared_borders_threshold: 3
    }
  }
}

#[derive(Error, Debug)]
pub enum LoadConfigError {
  #[error(transparent)]
  IoError(#[from] std::io::Error),
  #[error(transparent)]
  FormatError(#[from] toml::de::Error)
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
