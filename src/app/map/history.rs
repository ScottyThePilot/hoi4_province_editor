//! Structures for managing the history state and abstracting changes applied to the map
use vecmath::Vector2;

use crate::app::canvas::{ViewMode, BrushMask};
use crate::app::map::{Bundle, Color, Extents, Map, MapBase, ProvinceKind, ConnectionData, ConnectionKind};
use crate::app::map::bridge::recolor_everything;
use crate::util::XYIter;
use crate::util::uord::UOrd;

use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Debug)]
pub struct History {
  steps: VecDeque<Step>,
  position: usize,
  max_undo_states: usize
}

impl History {
  pub fn new(max_undo_states: usize, map: &Map) -> Self {
    assert!(max_undo_states >= 2, "The maximum number of undo states cannot be less than 2");
    let mut steps = VecDeque::with_capacity(1);
    steps.push_front(Step {
      map_base: map.base.clone(),
      origin: StepOrigin::LoadMap,
      view_mode: ViewMode::default()
    });

    History {
      steps,
      position: 0,
      max_undo_states
    }
  }

  pub fn undo(&mut self, map: &mut Map) -> Option<Commit> {
    // Apply the previous state
    if self.position != 0 {
      Some(self.apply(map, self.position - 1))
    } else {
      None
    }
  }

  pub fn redo(&mut self, map: &mut Map) -> Option<Commit> {
    // Apply the next state
    if self.position + 1 < self.steps.len() {
      Some(self.apply(map, self.position + 1))
    } else {
      None
    }
  }

  fn apply(&mut self, map: &mut Map, position: usize) -> Commit {
    self.position = position;

    // Apply the current state
    self.steps[position].apply(map)
  }

  fn push(&mut self, step: Step) {
    // Erase everything past the current state
    self.steps.truncate(self.position + 1);

    match self.steps.back_mut() {
      Some(last_step) if last_step.can_merge_with(&step) => {
        last_step.merge_with(step);
      },
      Some(..) | None => {
        // Push the new step onto the step list
        self.steps.push_back(step);
        // Increment the position counter
        self.position += 1;
      }
    };

    if self.steps.len() > self.max_undo_states {
      self.steps.pop_front();
    };
  }

  /// Adds a new `CompleteState` to the history based on the state of the provided map
  fn push_map_state(&mut self, map: &Map, origin: StepOrigin, view_mode: ViewMode) {
    self.push(Step {
      map_base: map.base.clone(),
      origin,
      view_mode
    });
  }

  pub fn calculate_coastal_provinces(&mut self, bundle: &mut Bundle) -> bool {
    let coastal_provinces = bundle.map.calculate_coastal_provinces();
    let is_not_pointless = coastal_provinces.iter()
      .any(|(&which, &coastal)| bundle.map.get_province(which).coastal != coastal);
    if is_not_pointless {
      for (&color, province_data) in Arc::make_mut(&mut bundle.map.base.province_data_map).iter_mut() {
        let province_data = Arc::make_mut(province_data);
        province_data.coastal = coastal_provinces[&color];
      };

      self.push_map_state(&bundle.map, StepOrigin::CalculateCoastalProvinces, ViewMode::Coastal);

      true
    } else {
      false
    }
  }

  pub fn calculate_recolor_map(&mut self, bundle: &mut Bundle) {
    recolor_everything(
      Arc::make_mut(&mut bundle.map.base.color_buffer),
      Arc::make_mut(&mut bundle.map.base.province_data_map),
      Arc::make_mut(&mut bundle.map.base.connection_data_map)
    );

    self.push_map_state(&bundle.map, StepOrigin::CalculateRecolorMap, ViewMode::Color);
  }

  pub fn paint_province_kind(&mut self, bundle: &mut Bundle, pos: Vector2<u32>, kind: impl Into<ProvinceKind>) -> Option<Extents> {
    let kind = kind.into();
    let which = bundle.map.get_color_at(pos);
    let province_data = bundle.map.get_province(which);
    if province_data.kind != kind && kind != ProvinceKind::Unknown {
      let terrain = kind.default_terrain();
      let continent = kind.correct_continent_id(province_data.continent);
      // Because the type changed, a repaint is always necessary
      let repaint = bundle.random_color_pure(kind);

      let province_data = bundle.map.get_province_mut(which);
      province_data.set_meta(kind, terrain.clone(), continent);
      let extents = bundle.map.recolor_province(which, repaint);

      self.push_map_state(&bundle.map, StepOrigin::PaintProvinceKind, ViewMode::Kind);
      Some(extents)
    } else {
      None
    }
  }

  pub fn paint_province_terrain(&mut self, bundle: &mut Bundle, pos: Vector2<u32>, terrain: String) -> Option<Extents> {
    let which = bundle.map.get_color_at(pos);
    let province_data = bundle.map.get_province(which);
    if province_data.terrain != terrain {
      let kind = bundle.config.terrain_kind(&terrain)
        .unwrap_or(ProvinceKind::Unknown);
      let continent = kind.correct_continent_id(province_data.continent);
      // If the type changed, generate a new color for it
      let repaint = (province_data.kind != kind)
        .then(|| bundle.random_color_pure(kind));

      let province_data = bundle.map.get_province_mut(which);
      province_data.set_meta(kind, terrain.clone(), continent);
      let extents = if let Some(repaint) = repaint {
        bundle.map.recolor_province(which, repaint)
      } else {
        bundle.map.get_color_extents(which)
      };

      self.push_map_state(&bundle.map, StepOrigin::PaintProvinceTerrain, ViewMode::Terrain);
      Some(extents)
    } else {
      None
    }
  }

  pub fn paint_province_continent(&mut self, bundle: &mut Bundle, pos: Vector2<u32>, continent: u16) -> Option<Extents> {
    let which = bundle.map.get_color_at(pos);
    let province_data = bundle.map.get_province(which);
    let valid_continent = province_data.kind.valid_continent_id(continent);
    if province_data.continent != continent && valid_continent {
      let extents = bundle.map.get_color_extents(which);

      let province_data = bundle.map.get_province_mut(which);
      province_data.continent = continent;

      self.push_map_state(&bundle.map, StepOrigin::PaintProvinceContinent, ViewMode::Continent);
      Some(extents)
    } else {
      None
    }
  }

  pub fn paint_entire_province(&mut self, bundle: &mut Bundle, pos: Vector2<u32>, fill_color: Color) -> Option<Extents> {
    let which = bundle.map.get_color_at(pos);
    if which != fill_color {
      let extents = bundle.map.recolor_province(which, fill_color);
      self.push_map_state(&bundle.map, StepOrigin::PaintEntireProvince, ViewMode::Color);
      Some(extents)
    } else {
      None
    }
  }

  pub fn paint_pixel_lasso(
    &mut self,
    bundle: &mut Bundle,
    lasso: Vec<Vector2<f64>>,
    color: Color,
    mask: Option<BrushMask>
  ) -> Option<Extents> {
    let (extents, pixels) = pixel_lasso(&bundle.map, lasso, color, mask);
    if !pixels.is_empty() {
      bundle.map.put_many_pixels(color, &pixels);
      self.push_map_state(&bundle.map, StepOrigin::PaintPixelLasso, ViewMode::Color);
      Some(extents)
    } else {
      None
    }
  }

  pub fn paint_pixel_bucket(
    &mut self,
    bundle: &mut Bundle,
    pos: Vector2<u32>,
    color: Color,
    mask: Option<BrushMask>
  ) -> Option<Extents> {
    let which = bundle.map.get_color_at(pos);
    let previous_kind = bundle.map.get_province(which).kind;
    let masked = mask.map_or(true, |mask| mask.includes(previous_kind));
    if masked && which != color {
      let extents = bundle.map.flood_fill_province(pos, color);
      self.push_map_state(&bundle.map, StepOrigin::PaintPixelBucket, ViewMode::Color);
      Some(extents)
    } else {
      None
    }
  }

  pub fn paint_pixel_area(
    &mut self,
    bundle: &mut Bundle,
    pos: Vector2<f64>,
    radius: f64,
    color: Color,
    mask: Option<BrushMask>,
    id: u32
  ) -> Option<Extents> {
    let (extents, pixels) = pixel_area(&bundle.map, pos, radius, color, mask);
    if !pixels.is_empty() {
      bundle.map.put_many_pixels(color, &pixels);
      self.push_map_state(&bundle.map, StepOrigin::PaintPixelArea(id), ViewMode::Color);
      Some(extents)
    } else {
      None
    }
  }

  pub fn paint_pixel(
    &mut self,
    bundle: &mut Bundle,
    pos: Vector2<u32>,
    color: Color,
    id: u32
  ) -> Option<Extents> {
    if bundle.map.get_color_at(pos) != color {
      bundle.map.put_pixel(pos, color);
      let extents = Extents::new_point(pos);
      self.push_map_state(&bundle.map, StepOrigin::PaintPixel(id), ViewMode::Color);
      Some(extents)
    } else {
      None
    }
  }

  pub fn add_or_remove_connection(&mut self, bundle: &mut Bundle, rel: UOrd<Color>, kind: ConnectionKind) -> bool {
    use std::collections::hash_map::Entry;
    if rel.is_distinct() {
      match Arc::make_mut(&mut bundle.map.base.connection_data_map).entry(rel) {
        Entry::Vacant(entry) => {
          entry.insert(Arc::new(ConnectionData::new(kind)));
        },
        Entry::Occupied(entry) => if entry.get().kind == kind {
          entry.remove();
        } else {
          Arc::make_mut(entry.into_mut()).kind = kind;
        }
      };

      bundle.map.recalculate_specialness();
      self.push_map_state(&bundle.map, StepOrigin::AddOrRemoveConnection, ViewMode::Adjacencies);
      true
    } else {
      false
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
  pub view_mode: ViewMode
}

#[derive(Debug)]
struct Step {
  map_base: MapBase,
  origin: StepOrigin,
  view_mode: ViewMode
}

impl Step {
  fn apply(&self, map: &mut Map) -> Commit {
    map.base = self.map_base.clone();
    Commit { view_mode: self.view_mode }
  }

  fn can_merge_with(&self, other: &Self) -> bool {
    self.origin.can_merge_with(other.origin)
  }

  fn merge_with(&mut self, other: Self) {
    self.map_base = other.map_base;
  }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum StepOrigin {
  LoadMap,
  CalculateCoastalProvinces,
  CalculateRecolorMap,
  PaintProvinceKind,
  PaintProvinceTerrain,
  PaintProvinceContinent,
  PaintEntireProvince,
  PaintPixelLasso,
  PaintPixelBucket,
  PaintPixelArea(u32),
  PaintPixel(u32),
  AddOrRemoveConnection
}

impl StepOrigin {
  fn can_merge_with(self, other: Self) -> bool {
    match (self, other) {
      (Self::PaintPixelArea(id1), Self::PaintPixelArea(id2)) => id1 == id2,
      (Self::PaintPixel(id1), Self::PaintPixel(id2)) => id1 == id2,
      _ => false
    }
  }
}

#[allow(deprecated)]
fn pixel_lasso(map: &Map, lasso: Vec<Vector2<f64>>, color: Color, mask: Option<BrushMask>) -> (Extents, Vec<Vector2<u32>>) {
  use geo::{Coordinate, LineString, Polygon};
  use geo::algorithm::contains::Contains;

  let mut pixels = Vec::new();
  let mut extents = Extents::from_points(&lasso);
  extents.upper[0] = extents.upper[0].min(map.width() - 1);
  extents.upper[1] = extents.upper[1].min(map.height() - 1);
  let lasso = Polygon::new(LineString::from(lasso), Vec::new());
  for [x, y] in XYIter::from_extents(extents) {
    let coord = Coordinate::from([x as f64 + 0.5, y as f64 + 0.5]);
    let previous_color = map.get_color_at([x, y]);
    let previous_kind = map.get_province(previous_color).kind;
    let masked = mask.map_or(true, |mask| mask.includes(previous_kind));
    if masked && color != previous_color && lasso.contains(&coord) {
      pixels.push([x, y]);
    };
  };

  (extents, pixels)
}

fn pixel_area(map: &Map, pos: Vector2<f64>, radius: f64, color: Color, mask: Option<BrushMask>) -> (Extents, Vec<Vector2<u32>>) {
  let mut pixels = Vec::new();
  let extents = Extents::from_pos_radius(pos, radius, map.dimensions());
  for [x, y] in XYIter::from_extents(extents) {
    let distance = f64::hypot(x as f64 + 0.5 - pos[0], y as f64 + 0.5 - pos[1]);
    let previous_color = map.get_color_at([x, y]);
    let previous_kind = map.get_province(previous_color).kind;
    let masked = mask.map_or(true, |mask| mask.includes(previous_kind));
    if masked && distance < radius && color != previous_color {
      pixels.push([x, y]);
    };
  };

  (extents, pixels)
}
