//! Structures for managing the history state and abstracting changes applied to the map
use fxhash::FxHashMap;
use image::RgbImage;
use vecmath::Vector2;

use crate::app::canvas::{ViewMode, BrushMask};
use crate::app::map::{Bundle, Color, Extents, Map, ProvinceData, ProvinceKind, ConnectionData, ConnectionKind};
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
    steps.push_front(Step::from(CompleteState {
      color_buffer: map.color_buffer.clone(),
      province_data_map: map.province_data_map.clone(),
      connection_data_map: map.connection_data_map.clone(),
      view_mode: ViewMode::default()
    }));

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
    // If this state is not a CompleteState, search for the latest one
    // and apply all states from it up until this one
    if !self.steps[position].is_complete() {
      let latest_complete = (0..position).rev()
        .find(|&i| self.steps[i].is_complete())
        .expect("no complete state found");
      for i in latest_complete..position {
        self.steps[i].apply_minimum(map);
      };
    };

    // Apply the current state
    self.steps[position].apply(map)
  }

  fn push(&mut self, step: impl Into<Step>) {
    // Erase everything past the current state
    self.steps.truncate(self.position + 1);
    // Push the new step onto the step list
    self.steps.push_back(step.into());
    // Increment the position counter
    self.position += 1;

    if self.steps.len() > self.max_undo_states {
      self.merge_front();
    };
  }

  /// Adds a new `CompleteState` to the history based on the state of the provided map
  fn push_map_state(&mut self, map: &Map, view_mode: ViewMode) {
    self.push(CompleteState {
      color_buffer: map.color_buffer.clone(),
      province_data_map: map.province_data_map.clone(),
      connection_data_map: map.connection_data_map.clone(),
      view_mode
    });
  }

  /// Adds a new `SelectiveState` to the history based on the state of the provided map
  fn push_map_state_selective(&mut self, map: &Map, view_mode: ViewMode, extents: Extents) {
    self.push(SelectiveState {
      color_buffer: map.extract(extents),
      province_data_map: map.province_data_map.clone(),
      connection_data_map: map.connection_data_map.clone(),
      view_mode,
      extents
    });
  }

  fn push_map_state_bufferless(&mut self, map: &Map, view_mode: ViewMode) {
    self.push(BufferlessState {
      province_data_map: map.province_data_map.clone(),
      connection_data_map: map.connection_data_map.clone(),
      view_mode
    });
  }

  /// Adds a new `PartialState` with, or expands the current `PartialState` with the provided extents
  fn update_partial_extents(&mut self, extents: Extents) {
    if let Some(Step::PartialState(partial_extents)) = self.steps.back_mut() {
      *partial_extents = Extents::join(*partial_extents, extents);
    } else {
      self.push(Step::PartialState(extents));
    }
  }

  /// Removes a step from the front of the history, merging
  /// it with the next if the next state is a `SelectiveState`
  fn merge_front(&mut self) {
    let mut complete = match self.steps.pop_front() {
      Some(Step::CompleteState(state)) => state,
      Some(_) | None => panic!()
    };

    let front = match self.steps.pop_front() {
      Some(Step::CompleteState(state)) => Step::CompleteState(state),
      Some(Step::SelectiveState(state)) => {
        let [xo, yo] = state.extents.to_offset();
        complete.province_data_map = state.province_data_map;
        complete.connection_data_map = state.connection_data_map;
        for (x, y, &pixel) in state.color_buffer.enumerate_pixels() {
          complete.color_buffer.put_pixel(x + xo, y + yo, pixel);
        };

        Step::CompleteState(complete)
      },
      Some(Step::BufferlessState(state)) => {
        complete.province_data_map = state.province_data_map;
        complete.connection_data_map = state.connection_data_map;

        Step::CompleteState(complete)
      },
      Some(Step::PartialState(_)) | None => panic!()
    };

    self.steps.push_front(front);
    self.position -= 1;
  }

  pub fn finish_last_step(&mut self, map: &Map) {
    if let Some(last_step) = self.steps.back_mut() {
      if let &mut Step::PartialState(extents) = last_step {
        *last_step = Step::from(SelectiveState {
          color_buffer: map.extract(extents),
          province_data_map: map.province_data_map.clone(),
          connection_data_map: map.connection_data_map.clone(),
          view_mode: ViewMode::default(),
          extents
        });
      };
    };
  }

  pub fn calculate_coastal_provinces(&mut self, bundle: &mut Bundle) -> bool {
    let coastal_provinces = bundle.map.calculate_coastal_provinces();
    let is_not_pointless = coastal_provinces.iter()
      .any(|(&which, &coastal)| bundle.map.get_province(which).coastal != coastal);
    if is_not_pointless {
      for (&color, province_data) in bundle.map.province_data_map.iter_mut() {
        let province_data = Arc::make_mut(province_data);
        province_data.coastal = coastal_provinces[&color];
      };

      self.push_map_state(&bundle.map, ViewMode::Coastal);

      true
    } else {
      false
    }
  }

  pub fn calculate_recolor_map(&mut self, bundle: &mut Bundle) {
    recolor_everything(
      &mut bundle.map.color_buffer,
      &mut bundle.map.province_data_map,
      &mut bundle.map.connection_data_map
    );

    self.push_map_state(&bundle.map, ViewMode::Color);
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

      self.push_map_state_selective(&bundle.map, ViewMode::Kind, extents);
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

      self.push_map_state_selective(&bundle.map, ViewMode::Terrain, extents);
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

      self.push_map_state_selective(&bundle.map, ViewMode::Continent, extents);
      Some(extents)
    } else {
      None
    }
  }

  pub fn paint_entire_province(&mut self, bundle: &mut Bundle, pos: Vector2<u32>, fill_color: Color) -> Option<Extents> {
    let which = bundle.map.get_color_at(pos);
    if which != fill_color {
      let extents = bundle.map.recolor_province(which, fill_color);
      self.push_map_state_selective(&bundle.map, ViewMode::Color, extents);
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
      self.push_map_state_selective(&bundle.map, ViewMode::Color, extents);
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
      self.push_map_state_selective(&bundle.map, ViewMode::Color, extents);
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
    mask: Option<BrushMask>
  ) -> Option<Extents> {
    let (extents, pixels) = pixel_area(&bundle.map, pos, radius, color, mask);
    if !pixels.is_empty() {
      bundle.map.put_many_pixels(color, &pixels);
      self.update_partial_extents(extents);
      Some(extents)
    } else {
      None
    }
  }

  pub fn paint_pixel(&mut self, bundle: &mut Bundle, pos: Vector2<u32>, color: Color) -> Option<Extents> {
    if bundle.map.get_color_at(pos) != color {
      bundle.map.put_pixel(pos, color);
      let extents = Extents::new_point(pos);
      self.update_partial_extents(extents);
      Some(extents)
    } else {
      None
    }
  }

  pub fn add_or_remove_connection(&mut self, bundle: &mut Bundle, rel: UOrd<Color>, kind: ConnectionKind) -> bool {
    use std::collections::hash_map::Entry;
    if rel.is_distinct() {
      match bundle.map.connection_data_map.entry(rel) {
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
      self.push_map_state_bufferless(&bundle.map, ViewMode::Adjacencies);
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
enum Step {
  CompleteState(CompleteState),
  SelectiveState(SelectiveState),
  BufferlessState(BufferlessState),
  PartialState(Extents)
}

impl Step {
  fn apply(&self, map: &mut Map) -> Commit {
    match self {
      Step::CompleteState(complete) => complete.apply(map),
      Step::SelectiveState(selective) => selective.apply(map),
      Step::BufferlessState(bufferless) => bufferless.apply(map),
      Step::PartialState(_) => panic!()
    }
  }

  fn apply_minimum(&self, map: &mut Map) {
    match self {
      Step::CompleteState(complete) => complete.apply_minimum(map),
      Step::SelectiveState(selective) => selective.apply_minimum(map),
      Step::BufferlessState(_) => (), // BufferlessState has no `apply_minimum`
      Step::PartialState(_) => panic!()
    }
  }

  fn is_complete(&self) -> bool {
    matches!(self, Step::CompleteState(_))
  }
}

#[derive(Debug)]
struct CompleteState {
  color_buffer: RgbImage,
  province_data_map: FxHashMap<Color, Arc<ProvinceData>>,
  connection_data_map: FxHashMap<UOrd<Color>, Arc<ConnectionData>>,
  view_mode: ViewMode
}

impl CompleteState {
  fn apply(&self, map: &mut Map) -> Commit {
    map.color_buffer = self.color_buffer.clone();
    map.province_data_map = self.province_data_map.clone();
    map.connection_data_map = self.connection_data_map.clone();
    Commit { view_mode: self.view_mode }
  }

  fn apply_minimum(&self, map: &mut Map) {
    map.color_buffer = self.color_buffer.clone();
  }
}

#[derive(Debug)]
struct SelectiveState {
  color_buffer: RgbImage,
  province_data_map: FxHashMap<Color, Arc<ProvinceData>>,
  connection_data_map: FxHashMap<UOrd<Color>, Arc<ConnectionData>>,
  view_mode: ViewMode,
  extents: Extents
}

impl SelectiveState {
  fn apply(&self, map: &mut Map) -> Commit {
    map.put_selective_raw(&self.color_buffer, self.extents.to_offset());
    map.province_data_map = self.province_data_map.clone();
    map.connection_data_map = self.connection_data_map.clone();
    Commit { view_mode: self.view_mode }
  }

  fn apply_minimum(&self, map: &mut Map) {
    map.put_selective_raw(&self.color_buffer, self.extents.to_offset());
  }
}

#[derive(Debug)]
struct BufferlessState {
  province_data_map: FxHashMap<Color, Arc<ProvinceData>>,
  connection_data_map: FxHashMap<UOrd<Color>, Arc<ConnectionData>>,
  view_mode: ViewMode
}

impl BufferlessState {
  fn apply(&self, map: &mut Map) -> Commit {
    map.province_data_map = self.province_data_map.clone();
    map.connection_data_map = self.connection_data_map.clone();
    Commit { view_mode: self.view_mode }
  }
}

impl From<CompleteState> for Step {
  fn from(value: CompleteState) -> Step {
    Step::CompleteState(value)
  }
}

impl From<SelectiveState> for Step {
  fn from(value: SelectiveState) -> Step {
    Step::SelectiveState(value)
  }
}

impl From<BufferlessState> for Step {
  fn from(value: BufferlessState) -> Step {
    Step::BufferlessState(value)
  }
}

impl From<Extents> for Step {
  fn from(value: Extents) -> Step {
    Step::PartialState(value)
  }
}

#[allow(deprecated)]
fn pixel_lasso(map: &Map, lasso: Vec<Vector2<f64>>, color: Color, mask: Option<BrushMask>) -> (Extents, Vec<Vector2<u32>>) {
  use geo::{Coordinate, LineString, Polygon};
  use geo::algorithm::contains::Contains;

  let mut pixels = Vec::new();
  let mut extents = Extents::from_points(&lasso);
  extents.upper[0] = extents.upper[0].min(map.color_buffer.width() - 1);
  extents.upper[1] = extents.upper[1].min(map.color_buffer.height() - 1);
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
