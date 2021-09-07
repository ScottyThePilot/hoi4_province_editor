//! An abstract representation of a map's data
mod history;
mod bridge;
mod problems;

use crossbeam::thread::scope;
use image::{Rgb, RgbImage, Rgba, RgbaImage, Pixel};
use fxhash::{FxHashMap, FxHashSet};
use rand::Rng;
use serde::{Serialize, Deserialize};
use vecmath::Vector2;

use crate::config::Config;
use crate::util::fx_hash_map_with_capacity;
use crate::util::XYIter;
use crate::util::uord::UOrd;
use crate::app::format::*;
use crate::error::Error;

pub use self::bridge::{Location, IntoLocation};
pub use self::bridge::{write_rgb_bmp_image, read_rgb_bmp_image};
pub use self::history::History;
pub use self::problems::Problem;

use std::convert::TryFrom;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const CHUNK_SIZE: usize = 1048576;

pub type Color = [u8; 3];

#[derive(Debug)]
pub struct Bundle {
  pub map: Map,
  pub config: Config
}

impl Bundle {
  pub fn load(location: &Location, config: Config) -> Result<Self, Error> {
    self::bridge::load_bundle(location, config)
  }

  pub fn save(&self, location: &Location) -> Result<(), Error> {
    self::bridge::save_bundle(location, self)
  }

  pub fn generate_problems(&self) -> Vec<Problem> {
    self::problems::analyze(self)
  }

  pub fn image_buffer_land(&self) -> Option<RgbImage> {
    self.map.gen_image_buffer(|which| {
      let kind = self.map.get_province(which).kind;
      self.config.land_color(kind)
    })
  }

  pub fn image_buffer_terrain(&self) -> Option<RgbImage> {
    self.map.gen_image_buffer(|which| {
      let terrain = &self.map.get_province(which).terrain;
      self.config.terrain_color(terrain)
    })
  }

  pub fn texture_buffer_color(&self) -> RgbaImage {
    self.map.gen_texture_buffer(|which| which)
  }

  pub fn texture_buffer_selective_color(&self, extents: Extents) -> RgbaImage {
    self.map.gen_texture_buffer_selective(extents, |which| which)
  }

  pub fn texture_buffer_kind(&self) -> RgbaImage {
    self.map.gen_texture_buffer(|which| {
      let kind = self.map.get_province(which).kind;
      self.config.kind_color(kind)
    })
  }

  pub fn texture_buffer_selective_kind(&self, extents: Extents) -> RgbaImage {
    self.map.gen_texture_buffer_selective(extents, |which| {
      let kind = self.map.get_province(which).kind;
      self.config.kind_color(kind)
    })
  }

  pub fn texture_buffer_terrain(&self) -> RgbaImage {
    self.map.gen_texture_buffer(|which| {
      let terrain = &self.map.get_province(which).terrain;
      match self.config.terrain_color(terrain) {
        None => panic!("unknown terrain type, color not found in config: {}", terrain),
        Some(color) => color
      }
    })
  }

  pub fn texture_buffer_selective_terrain(&self, extents: Extents) -> RgbaImage {
    self.map.gen_texture_buffer_selective(extents, |which| {
      let terrain = &self.map.get_province(which).terrain;
      match self.config.terrain_color(terrain) {
        None => panic!("unknown terrain type, color not found in config: {}", terrain),
        Some(color) => color
      }
    })
  }

  // The 4096 continent cap is due to the way `random::sequence_color` works,
  // pre-generating the colors for each ID is much more efficient and avoids the
  // overhead of locking mechanisms of a generate-as-you-go type setup.
  // Besides, a map with 4096 continents sounds a bit absurd.

  pub fn texture_buffer_continent(&self) -> RgbaImage {
    self.map.gen_texture_buffer(|which| {
      let continent = self.map.get_province(which).continent;
      crate::util::random::sequence_color(continent as usize)
        .expect("only a maximum of 4096 provinces are supported")
    })
  }

  pub fn texture_buffer_selective_continent(&self, extents: Extents) -> RgbaImage {
    self.map.gen_texture_buffer_selective(extents, |which| {
      let continent = self.map.get_province(which).continent;
      crate::util::random::sequence_color(continent as usize)
        .expect("only a maximum of 4096 provinces are supported")
    })
  }

  pub fn texture_buffer_coastal(&self) -> RgbaImage {
    self.map.gen_texture_buffer(|which| {
      let ProvinceData { coastal, kind, .. } = *self.map.get_province(which);
      self.config.coastal_color(coastal, kind)
    })
  }

  pub fn texture_buffer_selective_coastal(&self, extents: Extents) -> RgbaImage {
    self.map.gen_texture_buffer_selective(extents, |which| {
      let ProvinceData { coastal, kind, .. } = *self.map.get_province(which);
      self.config.coastal_color(coastal, kind)
    })
  }

  /// Search for terrains that are not included in the config
  pub fn search_unknown_terrains(&self) -> Option<FxHashSet<String>> {
    let mut unknown_terrains = FxHashSet::default();
    for province_data in self.map.province_data_map.values() {
      if !self.config.terrains.contains_key(&province_data.terrain) {
        unknown_terrains.insert(province_data.terrain.clone());
      };
    };

    if !unknown_terrains.is_empty() {
      Some(unknown_terrains)
    } else {
      None
    }
  }

  pub fn random_color_pure(&self, kind: ProvinceKind) -> Color {
    random_color_pure(&self.map.province_data_map, kind)
  }
}



#[derive(Debug)]
pub struct Map {
  color_buffer: RgbImage,
  province_data_map: FxHashMap<Color, Arc<ProvinceData>>,
  connection_data_map: FxHashMap<UOrd<Color>, ConnectionData>,
  preserved_id_count: Option<u32>
}

impl Map {
  pub fn dimensions(&self) -> Vector2<u32> {
    [self.color_buffer.width(), self.color_buffer.height()]
  }

  pub fn width(&self) -> u32 {
    self.color_buffer.width()
  }

  pub fn height(&self) -> u32 {
    self.color_buffer.height()
  }

  pub fn provinces_count(&self) -> usize {
    self.province_data_map.len()
  }

  pub fn connections_count(&self) -> usize {
    self.connection_data_map.len()
  }

  /// Generates a texture buffer, a buffer to be consumed by the canvas to display the map
  pub fn gen_texture_buffer<F>(&self, f: F) -> RgbaImage
  where F: Fn(Color) -> Color + Send + Sync {
    const CHUNK_SIZE_BYTES: usize = CHUNK_SIZE * 4;
    let (width, height) = self.color_buffer.dimensions();
    let mut buffer = RgbaImage::new(width, height);
    scope(|s| {
      for (i, scope_chunk) in buffer.chunks_mut(CHUNK_SIZE_BYTES).enumerate() {
        let so = i * CHUNK_SIZE;
        let f = &f;
        s.spawn(move |_| {
          for (lo, pixel) in scope_chunk.chunks_mut(4).enumerate() {
            let pixel = <Rgba<u8> as Pixel>::from_slice_mut(pixel);
            let pos = pos_from_offset(so, lo, width as usize);
            let color = f(self.get_color_at(pos));
            *pixel = Rgba(p4(color));
          };
        });
      };
    }).unwrap();

    buffer
  }

  /// Generates a fragment of a texture buffer, based on a bounding box
  pub fn gen_texture_buffer_selective<F>(&self, extents: Extents, f: F) -> RgbaImage
  where F: Fn(Color) -> Color {
    let (offset, size) = extents.to_offset_size();
    let mut buffer = RgbaImage::new(size[0], size[1]);
    for (x, y, pixel) in buffer.enumerate_pixels_mut() {
      let pos = vecmath::vec2_add(offset, [x, y]);
      let color = f(self.get_color_at(pos));
      *pixel = Rgba(p4(color));
    };

    buffer
  }

  /// Generates an image buffer, a 24 bit RGB image to be exported and used outside of the program
  pub fn gen_image_buffer<F>(&self, f: F) -> Option<RgbImage>
  where F: Fn(Color) -> Option<Color> + Send + Sync {
    const CHUNK_SIZE_BYTES: usize = CHUNK_SIZE * 3;
    let (width, height) = self.color_buffer.dimensions();
    let mut buffer = RgbImage::new(width, height);
    let cancel = AtomicBool::new(false);
    scope(|s| {
      for (i, scope_chunk) in buffer.chunks_mut(CHUNK_SIZE_BYTES).enumerate() {
        let so = i * CHUNK_SIZE;
        let (f, cancel) = (&f, &cancel);
        s.spawn(move |_| {
          if cancel.load(Ordering::Relaxed) { return };
          for (lo, pixel) in scope_chunk.chunks_mut(3).enumerate() {
            let pixel = <Rgb<u8> as Pixel>::from_slice_mut(pixel);
            let pos = pos_from_offset(so, lo, width as usize);
            if let Some(color) = f(self.get_color_at(pos)) {
              *pixel = Rgb(color);
            } else {
              cancel.store(true, Ordering::Relaxed);
              break;
            };
          };
        });
      };
    }).unwrap();

    if cancel.into_inner() {
      None
    } else {
      Some(buffer)
    }
  }

  pub fn extract(&self, extents: Extents) -> RgbImage {
    use image::GenericImageView;
    let ([x, y], [width, height]) = extents.to_offset_size();
    self.color_buffer.view(x, y, width, height).to_image()
  }

  /// Sets the color of a single pixel in `color_buffer` and manages the
  /// pixel counts of all relevant provinces, returning Some(Color) if the
  /// province whos pixel was replaced no longer has any pixels left
  fn put_pixel_raw(&mut self, pos: Vector2<u32>, color: Color) -> Option<Color> {
    let pixel = self.color_buffer.get_pixel_mut(pos[0], pos[1]);
    let Rgb(previous_color) = std::mem::replace(pixel, Rgb(color));

    let entry = self.province_data_map.entry(color).or_default();
    Arc::make_mut(entry).pixel_count += 1;

    let previous_province = self.get_province_mut(previous_color);
    previous_province.pixel_count -= 1;

    if previous_province.pixel_count == 0 {
      Some(previous_color)
    } else {
      None
    }
  }

  /// Sets the color of a single pixel in `color_buffer`, checks included
  fn put_pixel(&mut self, pos: Vector2<u32>, color: Color) {
    if let Some(erased_color) = self.put_pixel_raw(pos, color) {
      self.erase_province_data(erased_color);
    };
  }

  /// Sets the color of multiple pixels in `color_buffer`, checks included
  fn put_many_pixels(&mut self, color: Color, pixels: &[Vector2<u32>]) {
    for &pos in pixels {
      self.put_pixel(pos, color);
    };
  }

  fn erase_province_data(&mut self, color: Color) {
    self.province_data_map.remove(&color);
    self.remove_related_connections(color);
  }

  /// Removes all connections which contain the given color
  fn remove_related_connections(&mut self, which: Color) {
    self.connection_data_map.retain(|rel, _| !rel.contains(&which));
  }

  /// Copies another image buffer into the image without any checks
  fn put_selective_raw(&mut self, buffer: &RgbImage, offset: Vector2<u32>) {
    use image::GenericImage;
    self.color_buffer.copy_from(buffer, offset[0], offset[1]).expect("error");
  }

  pub fn validate_pixel_counts(&self) -> bool {
    let mut pixel_counts = FxHashMap::<Color, u64>::default();
    for &Rgb(pixel) in self.color_buffer.pixels() {
      *pixel_counts.entry(pixel).or_insert(0) += 1;
    };

    for (color, province_data) in self.province_data_map.iter() {
      if pixel_counts.get(color) != Some(&province_data.pixel_count) {
        return false;
      };
    };

    true
  }

  pub fn calculate_coastal_provinces(&self) -> FxHashMap<Color, Option<bool>> {
    let mut coastal_provinces = self.province_data_map.keys()
      .map(|&color| (color, Some(false)))
      .collect::<FxHashMap<Color, Option<bool>>>();

    let coastal_neighbors = UOrd::new(ProvinceKind::Land, ProvinceKind::Sea);
    for neighboring in self.calculate_neighbors() {
      if neighboring.map(|n| self.get_province(n).kind) == coastal_neighbors {
        let (a, b) = neighboring.into_tuple();
        coastal_provinces.insert(a, Some(true));
        coastal_provinces.insert(b, Some(true));
      };
    };

    coastal_provinces
  }

  /// Returns a hashset of uords describing which provinces are touching each other
  fn calculate_neighbors(&self) -> FxHashSet<UOrd<Color>> {
    let mut neighbors = FxHashSet::default();
    let [width, height] = self.dimensions();

    let mut check = |pos, pos_xm, pos_ym| {
      let color = self.get_color_at(pos);
      let color_xm = self.get_color_at(pos_xm);
      let color_ym = self.get_color_at(pos_ym);
      if color != color_xm { neighbors.insert(UOrd::new(color, color_xm)); };
      if color != color_ym { neighbors.insert(UOrd::new(color, color_ym)); };
    };

    // Loop through the image, comparing pixels to each other to find adjacent sea and land pixels
    for pos in XYIter::new(0..width-1, 0..height-1) {
      check(pos, [pos[0] + 1, pos[1]], [pos[0], pos[1] + 1]);
    };

    // The above loop misses two comparisons with the bottom-right-most pixel, this calculates it manually
    let pos = [width - 1, height - 1];
    check(pos, [pos[0] - 1, pos[1]], [pos[0], pos[1] - 1]);

    neighbors
  }

  /// If the map has any provinces where the type is `Unknown`
  pub fn has_unknown_provinces(&self) -> bool {
    self.province_data_map.values()
      .any(|province_data| province_data.kind == ProvinceKind::Unknown)
  }

  /// Replaces all of one color in `color_buffer` without any checks
  fn replace_color_raw(&mut self, which: Color, color: Color) -> Extents {
    let mut out: Option<Extents> = None;
    for (x, y, Rgb(pixel)) in self.color_buffer.enumerate_pixels_mut() {
      if *pixel == which {
        *pixel = color;
        out = Some(match out {
          Some(extents) => extents.join_point([x, y]),
          None => Extents::new_point([x, y])
        });
      };
    };

    out.expect("color not found in map")
  }

  /// Replaces the key of one province with a new color in `province_data_map` without any checks
  fn rekey_province_raw(&mut self, which: Color, color: Color) {
    let province_data = self.province_data_map.remove(&which)
      .expect("province not found with color");
    let result = self.province_data_map.insert(color, province_data);
    debug_assert_eq!(result, None);
  }

  /// Replaces the keys of all connections containing one color with another color without any checks
  fn rekey_connections_raw(&mut self, which: Color, color: Color) {
    if !self.connection_data_map.is_empty() {
      let mut new_connection_data_map = fx_hash_map_with_capacity(self.connections_count());
      for (rel, connection_data) in self.connection_data_map.drain() {
        new_connection_data_map.insert(rel.replace(which, color), connection_data);
      };

      self.connection_data_map = new_connection_data_map;
    };
  }

  /// Completely replace all of one color in the map with another
  pub fn recolor_province(&mut self, which: Color, color: Color) -> Extents {
    assert_ne!(which, color, "Attempted to recolor a province when it is already the desired color");
    self.rekey_province_raw(which, color);
    self.rekey_connections_raw(which, color);
    self.replace_color_raw(which, color)
  }

  pub fn flood_fill_province(&mut self, pos: Vector2<u32>, color: Color) -> Extents {
    let which = self.get_color_at(pos);
    assert_ne!(which, color, "Attempted to flood-fill a province when it is already the desired color");
    let (extents, erased) = self.flood_fill_raw(pos, which, color);

    if erased {
      self.erase_province_data(which);
    };

    extents
  }

  /// Recursively attempts to replace the given color with the
  /// given replacement color at the given position, and then repeats the process
  /// with each pixel in each cardinal direction
  fn flood_fill_raw(&mut self, pos: Vector2<u32>, which: Color, color: Color) -> (Extents, bool) {
    const CARDINAL: [Vector2<i32>; 4] = [[0, 1], [0, -1], [1, 0], [-1, 0]];
    let mut extents = Extents::new_point(pos);

    if let Some(_) = self.put_pixel_raw(pos, color) {
      return (extents, true);
    };

    let [width, height] = self.dimensions();
    for &diff in CARDINAL.iter() {
      let x = pos[0].wrapping_add(diff[0] as u32);
      let y = pos[1].wrapping_add(diff[1] as u32);
      if x < width && y < height && self.get_color_at([x, y]) == which {
        let (ext, erased) = self.flood_fill_raw([x, y], which, color);
        extents = extents.join(ext);
        if erased {
          return (extents, true)
        };
      };
    };

    (extents, false)
  }

  pub fn get_color_extents(&self, which: Color) -> Extents {
    let mut out: Option<Extents> = None;
    for (x, y, &Rgb(pixel)) in self.color_buffer.enumerate_pixels() {
      if pixel == which {
        out = Some(match out {
          Some(extents) => extents.join_point([x, y]),
          None => Extents::new_point([x, y])
        });
      };
    };

    out.expect("color not found in map")
  }

  pub fn get_color_at(&self, pos: Vector2<u32>) -> Color {
    self.color_buffer.get_pixel(pos[0], pos[1]).0
  }

  pub fn get_province(&self, color: Color) -> &ProvinceData {
    self.province_data_map.get(&color).expect("province not found with color")
  }

  fn get_province_mut(&mut self, color: Color) -> &mut ProvinceData {
    let province = self.province_data_map.get_mut(&color)
      .expect("province not found with color");
    Arc::make_mut(province)
  }

  pub fn get_province_at(&self, pos: Vector2<u32>) -> &ProvinceData {
    self.get_province(self.get_color_at(pos))
  }

  pub fn get_connection(&self, rel: UOrd<Color>) -> &ConnectionData {
    self.connection_data_map.get(&rel).expect("connection not found with rel")
  }
}

/// Represents a simple bounding box
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Extents {
  // These bounds are inclusive
  pub upper: Vector2<u32>,
  pub lower: Vector2<u32>
}

impl Extents {
  pub fn new(upper: Vector2<u32>, lower: Vector2<u32>) -> Self {
    Extents { upper, lower }
  }

  pub fn new_point(pos: Vector2<u32>) -> Self {
    Extents { upper: pos, lower: pos }
  }

  pub fn new_entire_map(map: &Map) -> Self {
    let [width, height] = map.dimensions();
    Extents { upper: [width - 1, height - 1], lower: [0, 0] }
  }

  pub fn from_pos_radius(pos: Vector2<f64>, radius: f64, max: Vector2<u32>) -> Self {
    let x_lower = ((pos[0] - radius).floor() as u32).min(max[0] - 1);
    let y_lower = ((pos[1] - radius).floor() as u32).min(max[1] - 1);
    let x_upper = ((pos[0] + radius).ceil() as u32).min(max[0] - 1);
    let y_upper = ((pos[1] + radius).ceil() as u32).min(max[1] - 1);
    Extents { upper: [x_upper, y_upper], lower: [x_lower, y_lower] }
  }

  pub fn from_points(points: &[Vector2<f64>]) -> Self {
    let mut lower = vec2_floor(points[0]);
    let mut upper = vec2_ceil(points[0]);
    for &[x, y] in &points[1..] {
      lower[0] = lower[0].min(x.floor() as u32);
      lower[1] = lower[1].min(y.floor() as u32);
      upper[0] = upper[0].max(x.ceil() as u32);
      upper[1] = upper[1].max(y.ceil() as u32);
    };

    Extents { upper, lower }
  }

  pub fn join(self, other: Self) -> Self {
    Extents {
      upper: [self.upper[0].max(other.upper[0]), self.upper[1].max(other.upper[1])],
      lower: [self.lower[0].min(other.lower[0]), self.lower[1].min(other.lower[1])]
    }
  }

  pub fn join_point(self, pos: Vector2<u32>) -> Self {
    Extents {
      upper: [self.upper[0].max(pos[0]), self.upper[1].max(pos[1])],
      lower: [self.lower[0].min(pos[0]), self.lower[1].min(pos[1])]
    }
  }

  pub fn to_offset(self) -> Vector2<u32> {
    self.lower
  }

  pub fn to_offset_size(self) -> (Vector2<u32>, Vector2<u32>) {
    (self.lower, [self.upper[0] - self.lower[0] + 1, self.upper[1] - self.lower[1] + 1])
  }
}

fn vec2_floor(point: Vector2<f64>) -> Vector2<u32> {
  [point[0].floor() as u32, point[1].floor() as u32]
}

fn vec2_ceil(point: Vector2<f64>) -> Vector2<u32> {
  [point[0].ceil() as u32, point[1].ceil() as u32]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvinceData {
  pub preserved_id: Option<u32>,
  pub kind: ProvinceKind,
  pub terrain: String,
  pub continent: u16,
  pub coastal: Option<bool>,
  pub pixel_count: u64
}

impl ProvinceData {
  pub fn from_definition_config(definition: Definition, config: &Config) -> Self {
    ProvinceData {
      preserved_id: config.preserve_ids.then(|| definition.id),
      kind: definition.kind.into(),
      terrain: definition.terrain,
      continent: definition.continent,
      coastal: Some(definition.coastal),
      pixel_count: 0
    }
  }

  pub fn to_definition(&self, color: Color) -> Result<Definition, &'static str> {
    Ok(Definition {
      id: self.preserved_id.expect("no id provided for definition"),
      rgb: color,
      kind: self.kind.to_definition_kind()
        .ok_or("province data exists with 'unknown' type")?,
      coastal: self.coastal
        .ok_or("province data exists with unknown coastal status")?,
      terrain: match self.terrain.as_str() {
        "unknown" => return Err("province data exists with unknown terrain"),
        terrain => terrain.to_owned()
      },
      continent: self.continent
    })
  }

  pub fn to_definition_with_id(&self, color: Color, id: u32) -> Result<Definition, &'static str> {
    Ok(Definition {
      id,
      rgb: color,
      kind: self.kind.to_definition_kind()
        .ok_or("Province exists with unknown type")?,
      coastal: self.coastal
        .ok_or("Province exists with unknown coastal status")?,
      terrain: match self.terrain.as_str() {
        "unknown" => return Err("Province exists with unknown terrain"),
        terrain => terrain.to_owned()
      },
      continent: self.continent
    })
  }

  fn set_meta(&mut self, kind: ProvinceKind, terrain: String, continent: u16) {
    self.kind = kind;
    self.terrain = terrain;
    self.continent = continent;
  }
}

impl Default for ProvinceData {
  fn default() -> ProvinceData {
    ProvinceData {
      preserved_id: None,
      kind: ProvinceKind::Unknown,
      terrain: "unknown".to_owned(),
      continent: 0,
      coastal: None,
      pixel_count: 0
    }
  }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(into = "&str", try_from = "String")]
pub enum ProvinceKind {
  Land = 0,
  Sea = 1,
  Lake = 2,
  Unknown = 3
}

impl ProvinceKind {
  pub fn to_str(self) -> &'static str {
    match self {
      ProvinceKind::Land => "land",
      ProvinceKind::Sea => "sea",
      ProvinceKind::Lake => "lake",
      ProvinceKind::Unknown => "unknown"
    }
  }

  pub fn to_definition_kind(self) -> Option<DefinitionKind> {
    match self {
      ProvinceKind::Land => Some(DefinitionKind::Land),
      ProvinceKind::Sea => Some(DefinitionKind::Sea),
      ProvinceKind::Lake => Some(DefinitionKind::Lake),
      ProvinceKind::Unknown => None
    }
  }

  pub fn valid_continent_id(self, continent: u16) -> bool {
    match self {
      ProvinceKind::Land if continent == 0 => false,
      ProvinceKind::Sea if continent != 0 => false,
      // Lakes and Unknown can belong to any continent
      _ => true
    }

  }

  pub fn correct_continent_id(self, continent: u16) -> u16 {
    match self {
      ProvinceKind::Land if continent == 0 => 1,
      ProvinceKind::Sea => 0,
      _ => continent
    }
  }
}

impl From<DefinitionKind> for ProvinceKind {
  fn from(kind: DefinitionKind) -> ProvinceKind {
    match kind {
      DefinitionKind::Land => ProvinceKind::Land,
      DefinitionKind::Sea => ProvinceKind::Sea,
      DefinitionKind::Lake => ProvinceKind::Lake
    }
  }
}

impl FromStr for ProvinceKind {
  type Err = ParseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "land" => Ok(ProvinceKind::Land),
      "sea" => Ok(ProvinceKind::Sea),
      "lake" => Ok(ProvinceKind::Lake),
      "unknown" => Ok(ProvinceKind::Unknown),
      _ => Err(ParseError::InvalidDefinitionKind)
    }
  }
}

impl TryFrom<String> for ProvinceKind {
  type Error = ParseError;

  fn try_from(string: String) -> Result<Self, Self::Error> {
    ProvinceKind::from_str(&string)
  }
}

impl From<ProvinceKind> for &'static str {
  fn from(kind: ProvinceKind) -> &'static str {
    kind.to_str()
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionData {
  pub kind: AdjacencyKind,
  pub through: Option<u32>,
  pub start: Option<[u32; 2]>,
  pub stop: Option<[u32; 2]>,
  pub rule_name: String,
  pub comment: String
}

impl ConnectionData {
  pub fn from_adjacency(adjacency: Adjacency) -> Self {
    ConnectionData {
      kind: adjacency.kind,
      through: adjacency.through,
      start: adjacency.start,
      stop: adjacency.stop,
      rule_name: adjacency.rule_name,
      comment: adjacency.comment
    }
  }

  pub fn to_adjacency(&self, rel: UOrd<u32>) -> Adjacency {
    let (from_id, to_id) = rel.into_tuple();
    Adjacency {
      from_id,
      to_id,
      kind: self.kind,
      through: self.through,
      start: self.start,
      stop: self.stop,
      rule_name: self.rule_name.clone(),
      comment: self.comment.clone()
    }
  }
}

fn p4(color: Color) -> [u8; 4] {
  [color[0], color[1], color[2], 0xff]
}

fn random_color<R: Rng>(rng: &mut R, kind: ProvinceKind) -> Color {
  use crate::util::hsl::hsl_to_rgb;

  let lightness: f64 = match kind {
    ProvinceKind::Unknown => return [rng.gen::<u8>(); 3],
    ProvinceKind::Land => rng.gen_range(0.5..1.0),
    ProvinceKind::Lake => rng.gen_range(0.2..0.5),
    ProvinceKind::Sea  => rng.gen_range(0.04..0.2)
  };

  let saturation = (lightness - 0.5).abs() + 0.5;
  hsl_to_rgb([
    rng.gen_range(0.0..360.0),
    rng.gen_range(saturation..1.0),
    lightness
  ])
}

fn random_color_pure(collection: &impl ColorKeyable, kind: ProvinceKind) -> Color {
  let mut rng = rand::thread_rng();
  let mut color = random_color(&mut rng, kind);
  while collection.contains_color(color) || color == [0x00; 3] {
    color = random_color(&mut rng, kind);
  };

  color
}

pub trait ColorKeyable {
  fn contains_color(&self, color: Color) -> bool;
}

impl<T> ColorKeyable for FxHashMap<Color, T> {
  fn contains_color(&self, color: Color) -> bool {
    self.contains_key(&color)
  }
}

impl ColorKeyable for FxHashSet<Color> {
  fn contains_color(&self, color: Color) -> bool {
    self.contains(&color)
  }
}

#[inline(always)]
fn pos_from_offset(so: usize, lo: usize, width: usize) -> Vector2<u32> {
  let o = so + lo;
  [(o % width) as u32, (o / width) as u32]
}
