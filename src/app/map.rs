//! An abstract representation of a map's data
mod history;
mod bridge;
mod problems;

use graphics::types::Color as DrawColor;
use image::{Rgb, RgbImage, Rgba, RgbaImage};
use fxhash::{FxHashMap, FxHashSet};
use rand::Rng;
use rayon::iter::ParallelIterator;
use serde::{Serialize, Deserialize};
use vecmath::Vector2;

use crate::config::Config;
use crate::util::fx_hash_map_with_capacity;
use crate::util::XYIter;
use crate::util::uord::UOrd;
use crate::app::colors;
use crate::app::format::*;
use crate::error::Error;

pub use self::bridge::{Location, IntoLocation};
pub use self::bridge::{write_rgb_bmp_image, read_rgb_bmp_image};
pub use self::history::History;
pub use self::problems::Problem;

use std::convert::TryFrom;
use std::str::FromStr;
use std::sync::Arc;

const CARDINAL: [Vector2<i32>; 4] = [[0, 1], [0, -1], [1, 0], [-1, 0]];

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

  pub fn image_buffer_mapgen_land(&self) -> Option<RgbImage> {
    self.map.gen_image_buffer(|which| {
      self.map.get_province(which).kind.color_mapgen()
    })
  }

  pub fn image_buffer_mapgen_terrain(&self) -> Option<RgbImage> {
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
      self.map.get_province(which).kind.color()
    })
  }

  pub fn texture_buffer_selective_kind(&self, extents: Extents) -> RgbaImage {
    self.map.gen_texture_buffer_selective(extents, |which| {
      self.map.get_province(which).kind.color()
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
      kind.color_coastal(coastal)
    })
  }

  pub fn texture_buffer_selective_coastal(&self, extents: Extents) -> RgbaImage {
    self.map.gen_texture_buffer_selective(extents, |which| {
      let ProvinceData { coastal, kind, .. } = *self.map.get_province(which);
      kind.color_coastal(coastal)
    })
  }

  /// Search for terrains that are not included in the config
  pub fn search_unknown_terrains(&self) -> Option<FxHashSet<String>> {
    let mut unknown_terrains = FxHashSet::default();
    for province_data in self.map.base.province_data_map.values() {
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
    random_color_pure(&*self.map.base.province_data_map, kind)
  }
}



#[derive(Clone)]
pub struct MapBase {
  color_buffer: Arc<RgbImage>,
  province_data_map: Arc<FxHashMap<Color, Arc<ProvinceData>>>,
  connection_data_map: Arc<FxHashMap<UOrd<Color>, Arc<ConnectionData>>>
}

impl std::fmt::Debug for MapBase {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("MapBase")
      .field("color_buffer", &format_args!("{:p}", self.color_buffer))
      .field("province_data_map", &format_args!("{:p}", self.province_data_map))
      .field("connection_data_map", &format_args!("{:p}", self.connection_data_map))
      .finish()
  }
}

#[derive(Debug)]
pub struct Map {
  base: MapBase,
  boundaries: FxHashMap<UOrd<Vector2<u32>>, bool>,
  preserved_id_count: Option<u32>
}

impl Map {
  pub fn dimensions(&self) -> Vector2<u32> {
    [self.width(), self.height()]
  }

  pub fn width(&self) -> u32 {
    self.base.color_buffer.width()
  }

  pub fn height(&self) -> u32 {
    self.base.color_buffer.height()
  }

  pub fn provinces_count(&self) -> usize {
    self.base.province_data_map.len()
  }

  pub fn connections_count(&self) -> usize {
    self.base.connection_data_map.len()
  }

  /// Generates a texture buffer, a buffer to be consumed by the canvas to display the map
  pub fn gen_texture_buffer<F>(&self, f: F) -> RgbaImage
  where F: Fn(Color) -> Color + Send + Sync {
    let [width, height] = self.dimensions();
    RgbaImage::from_par_fn(width, height, |x, y| {
      Rgba(p4(f(self.get_color_at([x, y]))))
    })
  }

  /// Generates a fragment of a texture buffer, based on a bounding box
  pub fn gen_texture_buffer_selective<F>(&self, extents: Extents, f: F) -> RgbaImage
  where F: Fn(Color) -> Color {
    let (offset, [width, height]) = extents.to_offset_size();
    RgbaImage::from_fn(width, height, |x, y| {
      let pos = vecmath::vec2_add(offset, [x, y]);
      Rgba(p4(f(self.get_color_at(pos))))
    })
  }

  /// Generates an image buffer, a 24 bit RGB image to be exported and used outside of the program
  pub fn gen_image_buffer<F>(&self, f: F) -> Option<RgbImage>
  where F: Fn(Color) -> Option<Color> + Send + Sync {
    let [width, height] = self.dimensions();
    let mut buffer = RgbImage::new(width, height);
    buffer.par_enumerate_pixels_mut().try_for_each(|(x, y, pixel)| {
      if let Some(color) = f(self.get_color_at([x, y])) {
        *pixel = Rgb(color);
        Some(())
      } else {
        None
      }
    })?;

    Some(buffer)
  }

  pub fn extract(&self, extents: Extents) -> RgbImage {
    use image::GenericImageView;
    let ([x, y], [width, height]) = extents.to_offset_size();
    self.base.color_buffer.view(x, y, width, height).to_image()
  }

  /// Sets the color of a single pixel in `color_buffer` and manages the
  /// pixel counts of all relevant provinces, returning Some(Color) if the
  /// province whos pixel was replaced no longer has any pixels left
  fn put_pixel_raw(&mut self, pos: Vector2<u32>, color: Color) -> Option<Color> {
    let pixel = Arc::make_mut(&mut self.base.color_buffer).get_pixel_mut(pos[0], pos[1]);
    let Rgb(previous_color) = std::mem::replace(pixel, Rgb(color));

    let entry = Arc::make_mut(&mut self.base.province_data_map).entry(color).or_default();
    let entry = Arc::make_mut(entry);
    entry.add_pixel(pos);

    let previous_province = self.get_province_mut(previous_color);
    previous_province.sub_pixel(pos);

    if previous_province.pixel_count == 0 {
      Some(previous_color)
    } else {
      None
    }
  }

  /// Sets the color of a single pixel in `color_buffer`, checks included
  fn put_pixel(&mut self, pos: Vector2<u32>, color: Color) {
    self.recalculate_boundaries_at(pos);
    if let Some(erased_color) = self.put_pixel_raw(pos, color) {
      self.erase_province_data(erased_color);
    };
  }

  /// Sets the color of multiple pixels in `color_buffer`, checks included
  fn put_many_pixels(&mut self, color: Color, pixels: &[Vector2<u32>]) {
    for &pos in pixels {
      self.put_pixel(pos, color);
      self.recalculate_boundaries_at(pos);
    };
  }

  fn erase_province_data(&mut self, color: Color) {
    Arc::make_mut(&mut self.base.province_data_map).remove(&color);
    self.remove_related_connections(color);
  }

  /// Removes all connections which contain the given color
  fn remove_related_connections(&mut self, which: Color) {
    Arc::make_mut(&mut self.base.connection_data_map).retain(|rel, conn| {
      if conn.through == Some(which) {
        Arc::make_mut(conn).through = None;
      };

      !rel.contains(&which)
    });
  }

  /// Copies another image buffer into the image without any checks
  fn put_selective_raw(&mut self, buffer: &RgbImage, offset: Vector2<u32>) {
    use image::GenericImage;
    Arc::make_mut(&mut self.base.color_buffer)
      .copy_from(buffer, offset[0], offset[1]).expect("error");
  }

  pub fn validate_pixel_counts(&self) -> bool {
    let mut pixel_counts = FxHashMap::<Color, u64>::default();
    for &Rgb(pixel) in self.base.color_buffer.pixels() {
      *pixel_counts.entry(pixel).or_insert(0) += 1;
    };

    for (color, province_data) in self.base.province_data_map.iter() {
      if pixel_counts.get(color) != Some(&province_data.pixel_count) {
        return false;
      };
    };

    true
  }

  pub fn calculate_coastal_provinces(&self) -> FxHashMap<Color, Option<bool>> {
    let mut coastal_provinces = self.base.province_data_map.keys()
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
    for (pos_a, pos_b) in self.iter_pixel_pairs() {
      let color_a = self.get_color_at(pos_a);
      let color_b = self.get_color_at(pos_b);
      if color_a != color_b {
        neighbors.insert(UOrd::new(color_a, color_b));
      };
    };

    neighbors
  }

  pub fn recalculate_all_boundaries(&mut self) {
    self.boundaries = FxHashMap::default();
    for (pos_a, pos_b) in self.iter_pixel_pairs() {
      let b = UOrd::new(pos_a, pos_b);
      let rel = b.map(|pos| self.get_color_at(pos));
      if rel.is_distinct() {
        let is_special = self.has_connection(rel);
        self.boundaries.insert(b, is_special);
      };
    };
  }

  pub fn recalculate_boundaries_extents(&mut self, extents: Extents) {
    for (pos_a, pos_b) in self.iter_pixel_pairs_extents(extents) {
      let b = UOrd::new(pos_a, pos_b);
      let rel = b.map(|pos| self.get_color_at(pos));
      if rel.is_distinct() {
        let is_special = self.has_connection(rel);
        self.boundaries.insert(b, is_special);
      } else {
        self.boundaries.remove(&b);
      };
    };
  }

  pub fn recalculate_boundaries_at(&mut self, pos: Vector2<u32>) {
    for other in self.iter_pixels_adjacent(pos) {
      let b = UOrd::new(pos, other);
      let rel = b.map(|pos| self.get_color_at(pos));
      if rel.is_distinct() {
        let is_special = self.has_connection(rel);
        self.boundaries.insert(b, is_special);
      } else {
        self.boundaries.remove(&b);
      };
    };
  }

  // Loops through every boundary, and determines whether or not they are special (have a connection) or not
  pub fn recalculate_specialness(&mut self) {
    for (b, _) in std::mem::take(&mut self.boundaries) {
      let rel = b.map(|pos| self.get_color_at(pos));
      self.boundaries.insert(b, self.has_connection(rel));
    };
  }

  fn iter_pixels_adjacent(&self, pos: Vector2<u32>) -> impl Iterator<Item = Vector2<u32>> {
    let [width, height] = self.dimensions();
    CARDINAL.into_iter()
      .filter_map(move |diff| {
        let x = pos[0].wrapping_add(diff[0] as u32);
        let y = pos[1].wrapping_add(diff[1] as u32);
        if x < width && y < height {
          Some([x, y])
        } else {
          None
        }
      })
  }

  fn iter_pixel_pairs(&self) -> impl Iterator<Item = (Vector2<u32>, Vector2<u32>)> {
    let [width, height] = self.dimensions();
    XYIter::new(0..width, 0..height)
      .flat_map(move |pos| {
        let a = (pos[0] + 1 < width).then(|| (pos, [pos[0] + 1, pos[1]]));
        let b = (pos[1] + 1 < height).then(|| (pos, [pos[0], pos[1] + 1]));
        Iterator::chain(a.into_iter(), b.into_iter())
      })
  }

  fn iter_pixel_pairs_extents(&self, mut extents: Extents) -> impl Iterator<Item = (Vector2<u32>, Vector2<u32>)> {
    let [width, height] = self.dimensions();
    extents.lower[0] = extents.lower[0].saturating_sub(1);
    extents.lower[1] = extents.lower[1].saturating_sub(1);
    extents.upper[0] = (extents.upper[0] + 1).min(width - 1);
    extents.upper[1] = (extents.upper[1] + 1).min(height - 1);
    let [width, height] = extents.upper;
    XYIter::from_extents(extents)
      .flat_map(move |pos| {
        let a = (pos[0] < width).then(|| (pos, [pos[0] + 1, pos[1]]));
        let b = (pos[1] < height).then(|| (pos, [pos[0], pos[1] + 1]));
        Iterator::chain(a.into_iter(), b.into_iter())
      })
  }

  /// If the map has any provinces where the type is `Unknown`
  pub fn has_unknown_provinces(&self) -> bool {
    self.base.province_data_map.values()
      .any(|province_data| province_data.kind == ProvinceKind::Unknown)
  }

  pub fn has_connection(&self, rel: UOrd<Color>) -> bool {
    self.base.connection_data_map.contains_key(&rel)
  }

  /// Replaces all of one color in `color_buffer`
  fn replace_color_raw(&mut self, which: Color, color: Color) -> Extents {
    Arc::make_mut(&mut self.base.color_buffer)
      .enumerate_pixels_mut()
      .fold(None, |out: Option<Extents>, (x, y, Rgb(pixel))| {
        if *pixel == which {
          *pixel = color;

          Some(match out {
            Some(extents) => extents.join_point([x, y]),
            None => Extents::new_point([x, y])
          })
        } else {
          out
        }
      })
      .expect("color not found in map")
  }

  /// Replaces the key of one province with a new color in `province_data_map`
  fn rekey_province_raw(&mut self, which: Color, color: Color) {
    let province_data_map = Arc::make_mut(&mut self.base.province_data_map);
    let province_data = province_data_map.remove(&which)
      .expect("province not found with color");
    let result = province_data_map.insert(color, province_data);
    debug_assert_eq!(result, None);
  }

  /// Replaces the keys of all connections containing one color with another color
  fn rekey_connections_raw(&mut self, which: Color, color: Color) {
    if !self.base.connection_data_map.is_empty() {
      let mut new_connection_data_map = fx_hash_map_with_capacity(self.connections_count());
      for (&rel, connection_data) in self.base.connection_data_map.iter() {
        // `through` does not get replaced here because it should
        // not be one of the colors that compose `rel`
        debug_assert_ne!(connection_data.through, Some(which));
        new_connection_data_map.insert(rel.replace(which, color), connection_data.clone());
      };

      self.base.connection_data_map = Arc::new(new_connection_data_map);
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
    self.recalculate_boundaries_extents(extents);

    if erased {
      self.erase_province_data(which);
    };

    extents
  }

  /// Recursively attempts to replace the given color with the
  /// given replacement color at the given position, and then repeats the process
  /// with each pixel in each cardinal direction
  fn flood_fill_raw(&mut self, pos: Vector2<u32>, which: Color, color: Color) -> (Extents, bool) {
    let mut extents = Extents::new_point(pos);

    if let Some(_) = self.put_pixel_raw(pos, color) {
      return (extents, true);
    };

    let [width, height] = self.dimensions();
    for diff in CARDINAL {
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
    for (x, y, &Rgb(pixel)) in self.base.color_buffer.enumerate_pixels() {
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
    self.base.color_buffer.get_pixel(pos[0], pos[1]).0
  }

  pub fn get_province(&self, color: Color) -> &ProvinceData {
    self.base.province_data_map.get(&color).expect("province not found with color")
  }

  fn get_province_mut(&mut self, color: Color) -> &mut ProvinceData {
    let province = Arc::make_mut(&mut self.base.province_data_map)
      .get_mut(&color).expect("province not found with color");
    Arc::make_mut(province)
  }

  pub fn get_province_at(&self, pos: Vector2<u32>) -> &ProvinceData {
    self.get_province(self.get_color_at(pos))
  }

  pub fn get_connection(&self, rel: UOrd<Color>) -> &ConnectionData {
    self.base.connection_data_map.get(&rel).expect("connection not found with rel")
  }

  pub fn get_connection_positions(&self, rel: UOrd<Color>) -> (Vector2<f64>, Vector2<f64>) {
    let connection_data = self.get_connection(rel);
    if let (Some(start), Some(stop)) = (connection_data.start, connection_data.stop) {
      ([start[0] as f64, start[1] as f64], [stop[0] as f64, stop[1] as f64])
    } else {
      let (start, stop) = rel.into_tuple();
      let start = self.get_province(start).center_of_mass();
      let stop = self.get_province(stop).center_of_mass();
      (start, stop)
    }
  }

  pub fn get_connection_nearest_within(&self, pos: Vector2<f64>, range: f64) -> Option<&ConnectionData> {
    self.get_rel_nearest(pos).and_then(|(rel, dist)| {
      (dist < range).then(|| self.get_connection(rel))
    })
  }

  pub fn get_connection_nearest(&self, pos: Vector2<f64>) -> Option<&ConnectionData> {
    self.get_rel_nearest(pos).map(|(rel, _)| self.get_connection(rel))
  }

  pub fn get_rel_nearest(&self, pos: Vector2<f64>) -> Option<(UOrd<Color>, f64)> {
    use geo::{Point, Line, Closest};
    use geo::algorithm::closest_point::ClosestPoint;
    use geo::algorithm::euclidean_distance::EuclideanDistance;

    fn distance(map: &Map, rel: UOrd<Color>, pos: Vector2<f64>) -> f64 {
      let (a, b) = map.get_connection_positions(rel);
      let line = Line::new(a, b);
      let point = Point::from(pos);
      let closest = match line.closest_point(&point) {
        Closest::Indeterminate => unreachable!(),
        Closest::SinglePoint(point) => point,
        Closest::Intersection(point) => point
      };

      closest.euclidean_distance(&point)
    }

    fn convert(f: f64) -> u64 {
      const BIT: u64 = 1 << (64 - 1);
      let u = unsafe { std::mem::transmute::<f64, u64>(f) };
      if u & BIT == 0 { u | BIT } else { !u }
    }

    let mut pairs = self.base.connection_data_map.keys()
      .map(|&rel| (rel, distance(self, rel, pos)))
      .collect::<Vec<(UOrd<Color>, f64)>>();
    pairs.sort_by_key(|&(_, d)| convert(d));

    pairs.first()
      .cloned()
  }

  pub fn add_or_remove_connection(&mut self, rel: UOrd<Color>, kind: ConnectionKind) {
    use std::collections::hash_map::Entry;
    match Arc::make_mut(&mut self.base.connection_data_map).entry(rel) {
      Entry::Vacant(entry) => {
        entry.insert(Arc::new(ConnectionData::new(kind)));
      },
      Entry::Occupied(entry) => if entry.get().kind != kind {
        Arc::make_mut(entry.into_mut()).kind = kind;
      } else {
        entry.remove();
      }
    }
  }

  pub fn iter_province_data(&self) -> impl Iterator<Item = (Color, &ProvinceData)> {
    self.base.province_data_map.iter().map(|(i, d)| (*i, &**d))
  }

  pub fn iter_connection_data(&self) -> impl Iterator<Item = (UOrd<Color>, &ConnectionData)> {
    self.base.connection_data_map.iter().map(|(i, c)| (*i, &**c))
  }

  pub fn iter_boundaries(&self) -> impl Iterator<Item = (UOrd<Vector2<u32>>, bool)> + '_ {
    self.boundaries.iter().map(|(b, is_special)| (*b, *is_special))
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

  pub fn contains(self, point: Vector2<u32>) -> bool {
    self.upper[0] >= point[0] && self.lower[0] <= point[0] &&
    self.upper[1] >= point[1] && self.lower[1] <= point[1]
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
  pub pixel_count: u64,
  pub pixel_sum: Vector2<u64>
}

impl ProvinceData {
  pub fn center_of_mass(&self) -> Vector2<f64> {
    let x = self.pixel_sum[0] as f64 / self.pixel_count as f64;
    let y = self.pixel_sum[1] as f64 / self.pixel_count as f64;
    [x, y]
  }

  fn add_pixel(&mut self, pos: Vector2<u32>) {
    self.pixel_count += 1;
    self.pixel_sum[0] += pos[0] as u64;
    self.pixel_sum[1] += pos[1] as u64;
  }

  fn sub_pixel(&mut self, pos: Vector2<u32>) {
    self.pixel_count -= 1;
    self.pixel_sum[0] -= pos[0] as u64;
    self.pixel_sum[1] -= pos[1] as u64;
  }

  pub fn from_definition_config(definition: Definition, config: &Config) -> Self {
    ProvinceData {
      preserved_id: config.preserve_ids.then(|| definition.id),
      kind: definition.kind.into(),
      terrain: definition.terrain,
      continent: definition.continent,
      coastal: Some(definition.coastal),
      pixel_count: 0,
      pixel_sum: [0, 0]
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
      pixel_count: 0,
      pixel_sum: [0, 0]
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

  pub fn default_terrain(self) -> String {
    match self {
      ProvinceKind::Unknown => "unknown".to_owned(),
      ProvinceKind::Land => "plains".to_owned(),
      ProvinceKind::Sea => "ocean".to_owned(),
      ProvinceKind::Lake => "lakes".to_owned()
    }
  }

  pub fn color(self) -> Color {
    match self {
      ProvinceKind::Land => [0x0a, 0xae, 0x3d],
      ProvinceKind::Sea => [0x00, 0x4c, 0x9e],
      ProvinceKind::Lake => [0x24, 0xab, 0xff],
      ProvinceKind::Unknown => [0x22, 0x22, 0x22]
    }
  }

  pub fn color_mapgen(self) -> Option<Color> {
    match self {
      ProvinceKind::Land => Some([150, 68, 192]),
      ProvinceKind::Sea => Some([5, 20, 18]),
      ProvinceKind::Lake => Some([80, 240, 120]),
      ProvinceKind::Unknown => None
    }
  }

  pub fn color_coastal(self, coastal: Option<bool>) -> Color {
    match (coastal, self) {
      (Some(false), ProvinceKind::Land) => [0x00, 0x33, 0x11],
      (Some(true), ProvinceKind::Land) => [0x33, 0x99, 0x55],
      (Some(false), ProvinceKind::Sea) => [0x00, 0x11, 0x33],
      (Some(true), ProvinceKind::Sea) => [0x33, 0x55, 0x99],
      (Some(false), ProvinceKind::Lake) => [0x00, 0x33, 0x33],
      (Some(true), ProvinceKind::Lake) => [0x33, 0x99, 0x99],
      _ => [0x22, 0x22, 0x22]
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
  pub kind: ConnectionKind,
  pub through: Option<Color>,
  pub start: Option<[u32; 2]>,
  pub stop: Option<[u32; 2]>,
  pub rule_name: String,
  pub comment: String
}

impl ConnectionData {
  fn new(kind: ConnectionKind) -> Self {
    ConnectionData {
      kind,
      through: None,
      start: None,
      stop: None,
      rule_name: String::new(),
      comment: String::new()
    }
  }

  pub fn from_adjacency<F>(adjacency: Adjacency, through: F) -> Option<Self>
  where F: Fn(u32) -> Color {
    Some(ConnectionData {
      kind: ConnectionKind::from_adjacency_kind(adjacency.kind)?,
      through: adjacency.through.map(through),
      start: adjacency.start,
      stop: adjacency.stop,
      rule_name: adjacency.rule_name,
      comment: adjacency.comment
    })
  }

  pub fn to_adjacency<F>(&self, rel: UOrd<u32>, through: F) -> Adjacency
  where F: Fn(Color) -> u32 {
    let (from_id, to_id) = rel.into_tuple();
    Adjacency {
      from_id,
      to_id,
      kind: self.kind.into_adjacency_kind(),
      through: self.through.map(through),
      start: self.start,
      stop: self.stop,
      rule_name: self.rule_name.clone(),
      comment: self.comment.clone()
    }
  }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConnectionKind {
  Strait,
  Canal,
  Impassable
}

impl ConnectionKind {
  pub fn to_str(&self) -> &'static str {
    match self {
      ConnectionKind::Strait => "strait/sea",
      ConnectionKind::Canal => "canal/land",
      ConnectionKind::Impassable => "impassable"
    }
  }

  pub fn from_adjacency_kind(kind: AdjacencyKind) -> Option<ConnectionKind> {
    match kind {
      AdjacencyKind::Land => Some(ConnectionKind::Canal),
      AdjacencyKind::River => None,
      AdjacencyKind::LargeRiver => None,
      AdjacencyKind::Sea => Some(ConnectionKind::Strait),
      AdjacencyKind::Impassable => Some(ConnectionKind::Impassable)
    }
  }

  pub fn into_adjacency_kind(self) -> AdjacencyKind {
    match self {
      ConnectionKind::Strait => AdjacencyKind::Sea,
      ConnectionKind::Canal => AdjacencyKind::Land,
      ConnectionKind::Impassable => AdjacencyKind::Impassable
    }
  }

  pub fn draw_color(self) -> DrawColor {
    match self {
      ConnectionKind::Strait => colors::ADJ_LAND,
      ConnectionKind::Canal => colors::ADJ_SEA,
      ConnectionKind::Impassable => colors::ADJ_IMPASSABLE
    }
  }
}

fn p4(color: Color) -> [u8; 4] {
  [color[0], color[1], color[2], 0xff]
}

fn random_color<R: Rng>(rng: &mut R, kind: ProvinceKind) -> Color {
  use crate::util::hsl::hsl_to_rgb;

  let lightness: f32 = match kind {
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
  loop {
    let color = random_color(&mut rng, kind);
    if !collection.contains_color(color) && color != [0x00; 3] {
      return color;
    };
  };
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

impl<T: ColorKeyable> ColorKeyable for &T {
  fn contains_color(&self, color: Color) -> bool {
    T::contains_color(&self, color)
  }
}

pub fn boundary_to_line(b: UOrd<Vector2<u32>>) -> UOrd<Vector2<u32>> {
  match b.into_tuple() {
    ([xa, ya], [xb, yb]) if xa == xb => {
      let y = ya.max(yb);
      UOrd::new([xa, y], [xa + 1, y])
    },
    ([xa, ya], [xb, yb]) if ya == yb => {
      let x = xa.max(xb);
      UOrd::new([x, ya], [x, ya + 1])
    },
    _ => panic!("boundary must be between two pixels, one unit apart")
  }
}

/// Takes the average of the colors of the boundary, and then inverts that
pub fn boundary_color(map: &Map, b: UOrd<Vector2<u32>>) -> Color {
  let (b1, b2) = b.into_tuple_unordered();
  let b1 = map.get_color_at(b1);
  let b2 = map.get_color_at(b2);

  [
    0xff - b1[0] / 2 - b2[0] / 2,
    0xff - b1[1] / 2 - b2[1] / 2,
    0xff - b1[2] / 2 - b2[2] / 2
  ]
}
