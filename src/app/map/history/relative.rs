//! Structures for managing the history state and abstracting changes applied to the map
use fxhash::FxHashMap;
use image::Rgb;
use vecmath::Vector2;

use crate::app::canvas::ViewMode;
use crate::app::map::{Bundle, Extents, Map, ProvinceKind, Color};
use crate::app::map::random_color_pure;
use crate::util::{fx_hash_set_with_capacity, fx_hash_map_with_capacity};
use crate::util::XYIter;

use std::collections::VecDeque;
use std::cmp::PartialEq;
use std::mem;



pub fn calculate_coastal_provinces(bundle: &mut Bundle, history: &mut History) -> bool {
  CalculateCoastalProvinces::new(bundle)
    .map(|op| op.insert(&mut bundle.map, history))
    .is_some()
}

pub fn calculate_recolor_map(bundle: &mut Bundle, history: &mut History) -> bool {
  CalculateRecolorMap::new(bundle)
    .map(|op| op.insert(&mut bundle.map, history))
    .is_some()
}

pub fn paint_province_kind(bundle: &mut Bundle, history: &mut History, pos: Vector2<u32>, kind: impl Into<ProvinceKind>) -> Option<Extents> {
  PaintProvinceMeta::new_change_kind(bundle, bundle.map.get_color_at(pos), kind.into())
    .map(|op| op.insert(&mut bundle.map, history))
}

pub fn paint_province_terrain(bundle: &mut Bundle, history: &mut History, pos: Vector2<u32>, terrain: String) -> Option<Extents> {
  PaintProvinceMeta::new_change_terrain(bundle, bundle.map.get_color_at(pos), terrain)
    .map(|op| op.insert(&mut bundle.map, history))
}

pub fn paint_province_continent(bundle: &mut Bundle, history: &mut History, pos: Vector2<u32>, continent: u16) -> Option<Extents> {
  PaintProvinceContinent::new(bundle, bundle.map.get_color_at(pos), continent)
    .map(|op| op.insert(&mut bundle.map, history))
}

pub fn paint_entire_province(bundle: &mut Bundle, history: &mut History, pos: Vector2<u32>, fill_color: Color) -> Option<Extents> {
  PaintRecolorProvince::new(bundle, bundle.map.get_color_at(pos), fill_color)
    .map(|op| op.insert(&mut bundle.map, history))
}

pub fn paint_pixel_area(bundle: &mut Bundle, history: &mut History, pos: Vector2<f64>, radius: f64, color: Color) -> Option<Extents> {
  PaintPixelArea::new(bundle, pos, radius, color)
    .map(|op| op.insert(&mut bundle.map, history))
}

pub fn paint_pixel(bundle: &mut Bundle, history: &mut History, pos: Vector2<u32>, color: Color) -> Option<Extents> {
  PaintPixel::new(bundle, pos, color)
    .map(|op| op.insert(&mut bundle.map, history))
}



trait MapOperation: Sized {
  /// Apply this operation to the map
  fn apply(&self, map: &mut Map);

  /// Apply this operation to the map, generating an operation that would undo this one
  fn apply_generate(&self, map: &mut Map) -> Self;

  /// Apply this operation, inserting it as a step in the history
  fn insert(self, map: &mut Map, history: &mut History) -> Extents
  where Step: From<MapChange<Self>> {
    let before = self.apply_generate(map);
    let extents = self.extents();
    history.push(MapChange::new(before, self));
    extents
  }

  fn view_mode(&self) -> ViewMode;

  fn extents(&self) -> Extents;
}

trait MapOperationPartial: Sized {
  /// Attempt to insert this partial map operation into the history, applying it to the map if successful
  fn insert(self, map: &mut Map, history: &mut History) -> Extents;
}

#[derive(Debug, Clone)]
struct MapChange<Op> {
  before: Op,
  after: Op,
  done: bool
}

impl<Op> MapChange<Op> {
  fn new(before: Op, after: Op) -> Self {
    MapChange { before, after, done: false }
  }
}

impl<Op: MapOperation> MapChange<Op> {
  fn undo(&self, map: &mut Map) -> Commit {
    self.before.apply(map);
    Commit {
      view_mode: self.before.view_mode(),
      extents: self.before.extents()
    }
  }

  fn redo(&self, map: &mut Map) -> Commit {
    self.after.apply(map);
    Commit {
      view_mode: self.after.view_mode(),
      extents: self.after.extents()
    }
  }
}



#[derive(Debug)]
pub struct History {
  steps: VecDeque<Step>,
  position: usize,
  capacity: usize
}

impl History {
  pub fn new(capacity: usize) -> Self {
    History {
      steps: VecDeque::new(),
      position: 0,
      capacity
    }
  }

  pub fn undo(&mut self, map: &mut Map) -> Option<Commit> {
    if self.position != 0 {
      let commit = self.steps[self.position - 1].undo(map);
      self.position -= 1;
      Some(commit)
    } else {
      None
    }
  }

  pub fn redo(&mut self, map: &mut Map) -> Option<Commit> {
    if self.position < self.steps.len() {
      let commit = self.steps[self.position].redo(map);
      self.position += 1;
      Some(commit)
    } else {
      None
    }
  }

  fn push(&mut self, step: impl Into<Step>) {
    self.steps.truncate(self.position);
    self.steps.push_back(step.into());
    self.position += 1;
    if self.steps.len() > self.capacity {
      self.steps.pop_front();
      self.position -= 1;
    };
  }

  pub fn finish_last_step(&mut self) {
    if let Some(step) = self.steps.back_mut() {
      step.finish();
    };
  }
}

/// Represents a change that the history has made to the map
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
  pub view_mode: ViewMode,
  pub extents: Extents
}



#[derive(Debug, Clone)]
enum Step {
  CalculateCoastalProvinces(MapChange<CalculateCoastalProvinces>),
  CalculateRecolorMap(MapChange<CalculateRecolorMap>),
  PaintProvinceMeta(MapChange<PaintProvinceMeta>),
  PaintProvinceContinent(MapChange<PaintProvinceContinent>),
  PaintEntireProvince(MapChange<PaintRecolorProvince>),
  PaintPixels(MapChange<PaintPixels>)
}

impl Step {
  pub(super) fn undo(&self, map: &mut Map) -> Commit {
    match self {
      Step::CalculateCoastalProvinces(change) => change.undo(map),
      Step::CalculateRecolorMap(change) => change.undo(map),
      Step::PaintProvinceMeta(change) => change.undo(map),
      Step::PaintProvinceContinent(change) => change.undo(map),
      Step::PaintEntireProvince(change) => change.undo(map),
      Step::PaintPixels(change) => change.undo(map)
    }
  }

  pub(super) fn redo(&self, map: &mut Map) -> Commit {
    match self {
      Step::CalculateCoastalProvinces(change) => change.redo(map),
      Step::CalculateRecolorMap(change) => change.redo(map),
      Step::PaintProvinceMeta(change) => change.redo(map),
      Step::PaintProvinceContinent(change) => change.redo(map),
      Step::PaintEntireProvince(change) => change.redo(map),
      Step::PaintPixels(change) => change.redo(map)
    }
  }

  pub(super) fn finish(&mut self) {
    if let Step::PaintPixels(change) = self {
      change.done = true;
    };
  }
}

impl From<MapChange<CalculateCoastalProvinces>> for Step {
  fn from(change: MapChange<CalculateCoastalProvinces>) -> Step {
    Step::CalculateCoastalProvinces(change)
  }
}

impl From<MapChange<CalculateRecolorMap>> for Step {
  fn from(change: MapChange<CalculateRecolorMap>) -> Step {
    Step::CalculateRecolorMap(change)
  }
}

impl From<MapChange<PaintProvinceMeta>> for Step {
  fn from(change: MapChange<PaintProvinceMeta>) -> Step {
    Step::PaintProvinceMeta(change)
  }
}

impl From<MapChange<PaintProvinceContinent>> for Step {
  fn from(change: MapChange<PaintProvinceContinent>) -> Step {
    Step::PaintProvinceContinent(change)
  }
}

impl From<MapChange<PaintRecolorProvince>> for Step {
  fn from(change: MapChange<PaintRecolorProvince>) -> Step {
    Step::PaintEntireProvince(change)
  }
}

impl From<MapChange<PaintPixels>> for Step {
  fn from(change: MapChange<PaintPixels>) -> Step {
    Step::PaintPixels(change)
  }
}

impl From<MapChange<PaintPixel>> for Step {
  fn from(change: MapChange<PaintPixel>) -> Step {
    let extents = Extents::new_point(change.before.pos);
    let before = PaintPixels { affected_pixels: vec![change.before.into()], extents };
    let after = PaintPixels { affected_pixels: vec![change.after.into()], extents };
    Step::PaintPixels(MapChange::new(before, after))
  }
}



#[derive(Debug, Clone)]
pub struct CalculateCoastalProvinces {
  pub(super) coastal_provinces: FxHashMap<Color, Option<bool>>,
  pub(super) extents: Extents
}

impl CalculateCoastalProvinces {
  pub(super) fn new(bundle: &Bundle) -> Option<Self> {
    let coastal_provinces = bundle.map.calculate_coastal_provinces();
    let is_not_pointless = coastal_provinces.iter()
      .any(|(&which, &coastal)| bundle.map.get_province(which).coastal != coastal);
    if is_not_pointless {
      let [width, height] = bundle.map.dimensions();
      let extents = Extents::new([width - 1, height - 1], [0, 0]);
      Some(CalculateCoastalProvinces { coastal_provinces, extents })
    } else {
      None
    }
  }
}

impl MapOperation for CalculateCoastalProvinces {
  fn apply(&self, map: &mut Map) {
    for (&color, province_data) in map.province_data_map.iter_mut() {
      province_data.coastal = self.coastal_provinces[&color];
    };
  }

  fn apply_generate(&self, map: &mut Map) -> Self {
    let mut coastal_provinces = fx_hash_map_with_capacity(self.coastal_provinces.len());
    for (&color, province_data) in map.province_data_map.iter_mut() {
      let coastal = self.coastal_provinces[&color];
      let coastal = mem::replace(&mut province_data.coastal, coastal);
      coastal_provinces.insert(color, coastal);
    };

    let extents = self.extents;
    CalculateCoastalProvinces { coastal_provinces, extents }
  }

  fn view_mode(&self) -> ViewMode {
    ViewMode::Coastal
  }

  fn extents(&self) -> Extents {
    self.extents
  }
}

#[derive(Debug, Clone)]
pub struct CalculateRecolorMap {
  pub(super) replacement_map: FxHashMap<Color, Color>,
  pub(super) extents: Extents
}

impl CalculateRecolorMap {
  pub(super) fn new(bundle: &Bundle) -> Option<Self> {
    let mut colors_list = fx_hash_set_with_capacity(bundle.map.provinces_count());
    let mut replacement_map = fx_hash_map_with_capacity(bundle.map.provinces_count());
    for (&previous_color, province_data) in bundle.map.province_data_map.iter() {
      let color = random_color_pure(&colors_list, province_data.kind);
      let opt = colors_list.insert(color);
      debug_assert!(opt);
      let opt = replacement_map.insert(previous_color, color);
      debug_assert_eq!(opt, None);
    };

    let [width, height] = bundle.map.dimensions();
    let extents = Extents::new([width - 1, height - 1], [0, 0]);
    Some(CalculateRecolorMap { replacement_map, extents })
  }
}

impl MapOperation for CalculateRecolorMap {
  fn apply(&self, map: &mut Map) {
    let mut new_province_data_map = fx_hash_map_with_capacity(map.provinces_count());
    for (previous_color, province_data) in map.province_data_map.drain() {
      let color = self.replacement_map[&previous_color];
      let opt = new_province_data_map.insert(color, province_data);
      debug_assert_eq!(opt, None);
    };

    map.province_data_map = new_province_data_map;

    let mut new_connection_data_map = fx_hash_map_with_capacity(map.connection_data_map.len());
    for (previous_rel, connection_data) in map.connection_data_map.drain() {
      let rel = previous_rel.map(|color| self.replacement_map[&color]);
      // This operation should never overwrite an existing entry
      let opt = new_connection_data_map.insert(rel, connection_data);
      debug_assert_eq!(opt, None);
    };

    map.connection_data_map = new_connection_data_map;

    for Rgb(pixel) in map.color_buffer.pixels_mut() {
      *pixel = self.replacement_map[pixel];
    };
  }

  fn apply_generate(&self, map: &mut Map) -> Self {
    self.apply(map);
    let mut replacement_map = fx_hash_map_with_capacity(self.replacement_map.len());
    for (&which, &color) in self.replacement_map.iter() {
      replacement_map.insert(color, which);
    };

    let extents = self.extents;
    CalculateRecolorMap { replacement_map, extents }
  }

  fn view_mode(&self) -> ViewMode {
    ViewMode::Color
  }

  fn extents(&self) -> Extents {
    self.extents
  }
}

#[derive(Debug, Clone)]
pub struct PaintProvinceMeta {
  pub(super) which: Color,
  pub(super) repaint: Option<Color>,
  pub(super) kind: ProvinceKind,
  pub(super) terrain: String,
  pub(super) continent: u16,
  pub(super) view_mode: ViewMode,
  pub(super) extents: Extents
}

impl PaintProvinceMeta {
  pub(super) fn new_change_kind(bundle: &Bundle, which: Color, kind: ProvinceKind) -> Option<Self> {
    let province_data = bundle.map.get_province(which);
    if province_data.kind != kind && kind != ProvinceKind::Unknown {
      let terrain = bundle.config.default_terrain(kind);
      //let terrain = bundle.config.default_terrain(kind);
      let continent = kind.correct_continent_id(province_data.continent);
      // Because the type changed, a repaint is always necessary
      let repaint = Some(bundle.random_color_pure(kind));
      Some(PaintProvinceMeta {
        which, repaint,
        kind, terrain, continent,
        view_mode: ViewMode::Kind,
        extents: get_color_extents(&bundle.map, which)
      })
    } else {
      None
    }
  }

  pub(super) fn new_change_terrain(bundle: &Bundle, which: Color, terrain: String) -> Option<Self> {
    let province_data = bundle.map.get_province(which);
    if province_data.terrain != terrain {
      let kind = bundle.config.terrain_kind(&terrain)
        .unwrap_or(ProvinceKind::Unknown);
      let continent = kind.correct_continent_id(province_data.continent);
      // If the type changed, generate a new color for it
      let repaint = (province_data.kind != kind)
        .then(|| bundle.random_color_pure(kind));
      Some(PaintProvinceMeta {
        which, repaint,
        kind, terrain, continent,
        view_mode: ViewMode::Terrain,
        extents: get_color_extents(&bundle.map, which)
      })
    } else {
      None
    }
  }
}

impl MapOperation for PaintProvinceMeta {
  fn apply(&self, map: &mut Map) {
    let province_data = map.get_province_mut(self.which);
    province_data.kind = self.kind;
    province_data.terrain = self.terrain.clone();
    province_data.continent = self.continent;
    if let Some(fill_color) = self.repaint {
      map.recolor_province(self.which, fill_color);
    };
  }

  fn apply_generate(&self, map: &mut Map) -> Self {
    let province_data = map.get_province_mut(self.which);
    let kind = mem::replace(&mut province_data.kind, self.kind);
    let terrain = mem::replace(&mut province_data.terrain, self.terrain.clone());
    let continent = mem::replace(&mut province_data.continent, self.continent);
    let (which, repaint) = if let Some(fill_color) = self.repaint {
      map.recolor_province(self.which, fill_color);
      (fill_color, Some(self.which))
    } else {
      (self.which, None)
    };

    PaintProvinceMeta {
      which,
      repaint,
      kind,
      terrain,
      continent,
      view_mode: self.view_mode,
      extents: self.extents
    }
  }

  fn view_mode(&self) -> ViewMode {
    self.view_mode
  }

  fn extents(&self) -> Extents {
    self.extents
  }
}

#[derive(Debug, Clone)]
pub struct PaintProvinceContinent {
  pub(super) which: Color,
  pub(super) continent: u16,
  pub(super) extents: Extents
}

impl PaintProvinceContinent {
  pub(super) fn new(bundle: &Bundle, which: Color, continent: u16) -> Option<Self> {
    let province_data = bundle.map.get_province(which);
    let valid_continent = province_data.kind.valid_continent_id(continent);
    if province_data.continent != continent && valid_continent {
      let extents = get_color_extents(&bundle.map, which);
      Some(PaintProvinceContinent { which, continent, extents })
    } else {
      None
    }
  }
}

impl MapOperation for PaintProvinceContinent {
  fn apply(&self, map: &mut Map) {
    map.get_province_mut(self.which).continent = self.continent;
  }

  fn apply_generate(&self, map: &mut Map) -> Self {
    let province_data = map.get_province_mut(self.which);
    let continent = mem::replace(&mut province_data.continent, self.continent);
    PaintProvinceContinent {
      which: self.which,
      continent,
      extents: self.extents
    }
  }

  fn view_mode(&self) -> ViewMode {
    ViewMode::Continent
  }

  fn extents(&self) -> Extents {
    self.extents
  }
}

#[derive(Debug, Clone)]
pub struct PaintRecolorProvince {
  pub(super) which: Color,
  pub(super) fill_color: Color,
  pub(super) extents: Extents
}

impl PaintRecolorProvince {
  pub(super) fn new(bundle: &Bundle, which: Color, fill_color: Color) -> Option<Self> {
    if which != fill_color {
      let extents = get_color_extents(&bundle.map, which);
      Some(PaintRecolorProvince { which, fill_color, extents })
    } else {
      None
    }
  }
}

impl MapOperation for PaintRecolorProvince {
  fn apply(&self, map: &mut Map) {
    map.recolor_province(self.which, self.fill_color);
  }

  fn apply_generate(&self, map: &mut Map) -> Self {
    map.recolor_province(self.which, self.fill_color);
    // Simply swapping `which` and `fill_color` gives us the previous state
    PaintRecolorProvince {
      which: self.fill_color,
      fill_color: self.which,
      extents: self.extents
    }
  }

  fn view_mode(&self) -> ViewMode {
    ViewMode::Color
  }

  fn extents(&self) -> Extents {
    self.extents
  }
}

#[derive(Debug, Clone)]
pub struct PaintPixels {
  pub(super) affected_pixels: Vec<(Vector2<u32>, Color)>,
  pub(super) extents: Extents
}

impl MapOperation for PaintPixels {
  fn apply(&self, map: &mut Map) {
    map.put_many_pixels(&self.affected_pixels);
  }

  fn apply_generate(&self, map: &mut Map) -> Self {
    let affected_pixels = self.affected_pixels.iter()
      .map(|&(pos, _)| (pos, map.get_color_at(pos)))
      .collect();
    map.put_many_pixels(&self.affected_pixels);
    PaintPixels {
      affected_pixels,
      extents: self.extents
    }
  }

  fn view_mode(&self) -> ViewMode {
    ViewMode::Color
  }

  fn extents(&self) -> Extents {
    self.extents
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaintPixelArea {
  pub(super) pos: Vector2<f64>,
  pub(super) radius: f64,
  pub(super) color: Color,
  pub(super) extents: Extents
}

impl PaintPixelArea {
  pub(super) fn new(bundle: &Bundle, pos: Vector2<f64>, radius: f64, color: Color) -> Option<Self> {
    if !is_pixel_area_pointless(&bundle.map, pos, radius, color) {
      let extents = Extents::new_pos_radius(pos, radius, bundle.map.dimensions());
      Some(PaintPixelArea { pos, radius, color, extents })
    } else {
      None
    }
  }
}

impl MapOperationPartial for PaintPixelArea {
  fn insert(self, map: &mut Map, history: &mut History) -> Extents {
    let [before, after] = pixel_area(map, self.pos, self.radius, self.color);
    map.put_many_pixels(&after);
    match history.steps.back_mut() {
      Some(Step::PaintPixels(op)) if !op.done => {
        let extents = op.before.extents.join(self.extents);
        op.before.affected_pixels.extend(before);
        op.before.extents = extents;
        op.after.affected_pixels.extend(after);
        op.after.extents = extents;
      },
      _ => history.push(MapChange {
        before: PaintPixels {
          affected_pixels: before,
          extents: self.extents
        },
        after: PaintPixels {
          affected_pixels: after,
          extents: self.extents
        },
        done: false
      })
    };

    self.extents
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintPixel {
  pos: Vector2<u32>,
  color: Color
}

impl PaintPixel {
  pub(super) fn new(bundle: &Bundle, pos: Vector2<u32>, color: Color) -> Option<Self> {
    if bundle.map.get_color_at(pos) != color {
      Some(PaintPixel { pos, color })
    } else {
      None
    }
  }
}

impl MapOperationPartial for PaintPixel {
  fn insert(self, map: &mut Map, history: &mut History) -> Extents {
    let color = map.get_color_at(self.pos);
    map.put_pixel(self.pos, self.color);
    let extents = Extents::new_point(self.pos);
    let (before, after) = (PaintPixel { pos: self.pos, color }, self);
    match history.steps.back_mut() {
      Some(Step::PaintPixels(op)) if !op.done => {
        let combined_extents = op.before.extents.join(extents);
        op.before.affected_pixels.push(before.into());
        op.before.extents = combined_extents;
        op.after.affected_pixels.push(after.into());
        op.after.extents = combined_extents;
      },
      _ => history.push(MapChange::new(before, after))
    };

    extents
  }
}

impl Into<(Vector2<u32>, Color)> for PaintPixel {
  fn into(self) -> (Vector2<u32>, Color) {
    (self.pos, self.color)
  }
}

impl PartialEq<(Vector2<u32>, Color)> for PaintPixel {
  fn eq(&self, (pos, color): &(Vector2<u32>, Color)) -> bool {
    self.pos == *pos && self.color == *color
  }
}

fn pixel_area(map: &Map, pos: Vector2<f64>, radius: f64, color: Color) -> [Vec<(Vector2<u32>, Color)>; 2] {
  let mut before = Vec::new();
  let mut after = Vec::new();
  for [x, y] in XYIter::from_extents(Extents::new_pos_radius(pos, radius, map.dimensions())) {
    let distance = f64::hypot(x as f64 + 0.5 - pos[0], y as f64 + 0.5 - pos[1]);
    let previous_color = map.get_color_at([x, y]);
    if distance < radius && color != previous_color {
      before.push(([x, y], previous_color));
      after.push(([x, y], color));
    };
  };

  [before, after]
}

fn is_pixel_area_pointless(map: &Map, pos: Vector2<f64>, radius: f64, color: Color) -> bool {
  for [x, y] in XYIter::from_extents(Extents::new_pos_radius(pos, radius, map.dimensions())) {
    let distance = f64::hypot(x as f64 + 0.5 - pos[0], y as f64 + 0.5 - pos[1]);
    let previous_color = map.get_color_at([x, y]);
    if distance < radius && color != previous_color {
      return false;
    };
  };

  true
}

fn get_color_extents(map: &Map, which: Color) -> Extents {
  let mut out: Option<Extents> = None;

  for (x, y, &Rgb(pixel)) in map.color_buffer.enumerate_pixels() {
    if pixel == which {
      out = Some(out.map_or_else(
        || Extents::new_point([x, y]),
        |extents| extents.join_point([x, y])
      ));
    };
  };

  out.expect("color not found in map")
}
