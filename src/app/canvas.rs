use fxhash::FxHashSet;
use graphics::Transformed;
use graphics::context::Context;
use graphics::ellipse::Ellipse;
use itertools::Itertools;
use opengl_graphics::{Filter, GlGraphics, Texture, TextureSettings};
use vecmath::{Matrix2x3, Vector2};

use super::{colors, FontGlyphCache, FONT_SIZE};
use super::map::*;
use super::format::DefinitionKind;
use crate::{WINDOW_WIDTH, WINDOW_HEIGHT};
use crate::config::Config;
use super::alerts::Alerts;
use crate::util::stringify_color;
use crate::error::Error;

use std::sync::Arc;

const ZOOM_SENSITIVITY: f64 = 0.125;
const WINDOW_CENTER: Vector2<f64> = [WINDOW_WIDTH as f64 / 2.0, WINDOW_HEIGHT as f64 / 2.0];

#[allow(missing_debug_implementations)]
pub struct Canvas {
  bundle: Bundle,
  history: History,
  texture: Texture,
  view_mode: ViewMode,
  brush: BrushSettings,
  problems: Vec<Problem>,
  unknown_terrains: Option<FxHashSet<String>>,
  location: Location,
  pub modified: bool,
  pub camera: Camera
}

impl Canvas {
  pub fn load(location: Location, config: Arc<Config>) -> Result<Canvas, Error> {
    let history = History::new(config.max_undo_states);
    let bundle = Bundle::load(&location, config)?;
    let texture_settings = TextureSettings::new().mag(Filter::Nearest);
    let texture = Texture::from_image(&bundle.texture_buffer_color(), &texture_settings);
    // The test map is very small with large ocean provinces, the 'too large box' errors go nuts
    let problems = if cfg!(debug_assertions) { Vec::new() } else { bundle.generate_problems() };
    let unknown_terrains = bundle.search_unknown_terrains();
    let camera = Camera::new(&texture);

    Ok(Canvas {
      bundle,
      history,
      texture,
      view_mode: ViewMode::Color,
      brush: BrushSettings::default(),
      problems,
      unknown_terrains,
      location,
      modified: false,
      camera
    })
  }

  pub fn save(&self, location: &Location) -> Result<(), Error> {
    self.bundle.save(location)
  }

  pub fn location(&self) -> &Location {
    &self.location
  }

  pub fn view_mode(&self) -> ViewMode {
    self.view_mode
  }

  pub fn set_location(&mut self, location: Location) {
    self.location = location;
  }

  pub fn config(&self) -> Arc<Config> {
    Arc::clone(&self.bundle.config)
  }

  pub fn draw(&self, ctx: Context, glyph_cache: &mut FontGlyphCache, cursor_pos: Option<Vector2<f64>>, gl: &mut GlGraphics) {
    let transform = ctx.transform.append_transform(self.camera.display_matrix);
    graphics::image(&self.texture, transform, gl);

    for problem in self.problems.iter() {
      problem.draw(ctx, self.camera.display_matrix, gl);
    };

    if let (ViewMode::Color, Some(cursor_pos)) = (self.view_mode, cursor_pos) {
      let color = if self.brush.color_brush.is_some() { colors::WHITE } else { colors::WHITE_T };
      let ellipse = Ellipse::new_border(color, 0.5).resolution(16);
      let radius = self.brush.radius * self.camera.scale_factor();
      let transform = ctx.transform.trans_pos(cursor_pos);
      ellipse.draw_from_to([radius, radius], [-radius, -radius], &Default::default(), transform, gl);
    };

    let camera_info = self.camera_info(cursor_pos);
    let transform = ctx.transform.trans(8.0, WINDOW_HEIGHT as f64 - 8.0);
    graphics::text(colors::WHITE, FONT_SIZE, &camera_info, glyph_cache, transform, gl)
      .expect("unable to draw text");
  }

  pub fn undo(&mut self) {
    if let Some(commit) = self.history.undo(&mut self.bundle.map) {
      self.problems.clear();
      if self.view_mode == commit.view_mode {
        self.refresh_selective(commit.extents);
      } else {
        self.view_mode = commit.view_mode;
        self.refresh();
      };
    };
  }

  pub fn redo(&mut self) {
    if let Some(commit) = self.history.redo(&mut self.bundle.map) {
      self.problems.clear();
      if self.view_mode == commit.view_mode {
        self.refresh_selective(commit.extents);
      } else {
        self.view_mode = commit.view_mode;
        self.refresh();
      };
    };
  }

  pub fn calculate_coastal_provinces(&mut self) {
    self.bundle.calculate_coastal_provinces(&mut self.history);
    self.view_mode = ViewMode::Coastal;
    self.refresh();
  }

  pub fn calculate_recolor_map(&mut self) {
    self.bundle.calculate_recolor_map(&mut self.history);
    self.view_mode = ViewMode::Color;
    self.brush.color_brush = None;
    self.refresh();
  }

  pub fn display_problems(&mut self, alerts: &mut Alerts) {
    self.problems = self.bundle.generate_problems();
    if self.problems.is_empty() {
      alerts.push(Ok("No map problems detected"));
    } else {
      for problem in self.problems.iter() {
        alerts.push(Ok(format!("Problem: {}", problem)));
      };
    };
  }

  pub fn brush_mut(&mut self) -> &mut BrushSettings {
    &mut self.brush
  }

  pub fn cycle_brush(&mut self, cursor_pos: Option<Vector2<f64>>, alerts: &mut Alerts) {
    match self.view_mode {
      ViewMode::Color => {
        let kind = self.brush.kind_brush
          .map(ProvinceKind::from)
          .or_else(|| {
            let pos = cursor_pos.and_then(|cursor_pos| {
              self.camera.relative_position_int(cursor_pos)
            })?;
            Some(self.bundle.map.get_province_at(pos).kind)
          })
          .unwrap_or(ProvinceKind::Land);
        let color = self.bundle.random_color_pure(kind);
        self.brush.color_brush = Some(color);
        alerts.push(Ok(format!("Brush set to color {}", stringify_color(color))))
      },
      ViewMode::Kind => {
        let kind = self.brush.kind_brush;
        let kind = self.bundle.config.cycle_kinds(kind);
        self.brush.kind_brush = Some(kind);
        alerts.push(Ok(format!("Brush set to type {}", kind.to_str().to_uppercase())));
      },
      ViewMode::Terrain => {
        let terrain = self.brush.terrain_brush.as_deref();
        let terrain = self.bundle.config.cycle_terrains(terrain);
        alerts.push(Ok(format!("Brush set to terrain {}", terrain.to_uppercase())));
        self.brush.terrain_brush = Some(terrain);
      },
      ViewMode::Continent => {
        let continent = self.brush.continent_brush;
        let continent = self.bundle.config.cycle_continents(continent);
        self.brush.continent_brush = Some(continent);
        alerts.push(Ok(format!("Brush set to continent {}", continent)));
      },
      ViewMode::Coastal => ()
    };
  }

  pub fn pick_brush(&mut self, cursor_pos: Vector2<f64>, alerts: &mut Alerts) {
    if let Some(pos) = self.camera.relative_position_int(cursor_pos) {
      let color = self.bundle.map.get_color_at(pos);
      let province_data = self.bundle.map.get_province_at(pos);
      match self.view_mode {
        ViewMode::Color => {
          self.brush.color_brush = Some(color);
          alerts.push(Ok(format!("Picked color {}", stringify_color(color))));
        },
        ViewMode::Kind => if let Some(kind) = province_data.kind.to_definition_kind() {
          self.brush.kind_brush = Some(kind);
          alerts.push(Ok(format!("Picked type {}", kind.to_str().to_uppercase())));
        },
        ViewMode::Terrain => if province_data.terrain != "unknown" {
          let terrain = province_data.terrain.as_str();
          self.brush.terrain_brush = Some(terrain.to_owned());
          alerts.push(Ok(format!("Picked terrain {}", terrain.to_uppercase())));
        },
        ViewMode::Continent => {
          let continent = province_data.continent;
          self.brush.continent_brush = Some(continent);
          alerts.push(Ok(format!("Picked continent {}", continent)));
        },
        ViewMode::Coastal => ()
      };
    };
  }

  pub fn paint_brush(&mut self, cursor_pos: Vector2<f64>) {
    if let Some(pos) = self.camera.relative_position_int(cursor_pos) {
      if let (Some(color), ViewMode::Color) = (self.brush.color_brush, self.view_mode) {
        let pos = self.camera.relative_position(cursor_pos);
        if let Some(extents) = self.bundle.paint_pixel_area(&mut self.history, pos, self.brush.radius, color) {
          self.problems.clear();
          self.modified = true;
          self.refresh_selective(extents);
        };
      } else if let (Some(kind), ViewMode::Kind) = (self.brush.kind_brush, self.view_mode) {
        if let Some(extents) = self.bundle.paint_province_kind(&mut self.history, pos, kind) {
          self.modified = true;
          self.refresh_selective(extents);
        };
      } else if let (Some(terrain), ViewMode::Terrain) = (&self.brush.terrain_brush, self.view_mode) {
        if let Some(extents) = self.bundle.paint_province_terrain(&mut self.history, pos, terrain.clone()) {
          self.modified = true;
          self.refresh_selective(extents);
        };
      } else if let (Some(continent), ViewMode::Continent) = (self.brush.continent_brush, self.view_mode) {
        if let Some(extents) = self.bundle.paint_province_continent(&mut self.history, pos, continent) {
          self.modified = true;
          self.refresh_selective(extents);
        };
      };
    };
  }

  pub fn paint_stop(&mut self) {
    self.history.finish_last_step();
  }

  pub fn change_brush_radius(&mut self, d: f64) {
    const LIMIT: f64 = std::f64::consts::SQRT_2 / 2.0;
    if let ViewMode::Color = self.view_mode {
      let r = self.brush.radius;
      let d = d * (1.0 + 0.025 * r);
      self.brush.radius = (r + d).max(LIMIT);
    };
  }

  pub fn set_view_mode(&mut self, alerts: &mut Alerts, view_mode: ViewMode) {
    if let (ViewMode::Terrain, Some(unknown_terrains)) = (view_mode, &self.unknown_terrains) {
      let unknown_terrains = unknown_terrains.iter().map(|s| s.to_uppercase()).join(", ");
      alerts.push(Err(format!("Terrain mode unavailable, unknown terrains present: {}", unknown_terrains)));
    } else if view_mode != self.view_mode {
      self.view_mode = view_mode;
      self.refresh();
    };
  }

  fn refresh(&mut self) {
    let buffer = match self.view_mode {
      ViewMode::Color => self.bundle.texture_buffer_color(),
      ViewMode::Kind => self.bundle.texture_buffer_kind(),
      ViewMode::Terrain => self.bundle.texture_buffer_terrain(),
      ViewMode::Continent => self.bundle.texture_buffer_continent(),
      ViewMode::Coastal => self.bundle.texture_buffer_coastal()
    };

    self.texture.update(&buffer);
  }

  fn refresh_selective(&mut self, extents: Extents) {
    use opengl_graphics::{UpdateTexture, Format};
    let (offset, size) = extents.to_offset_size();
    let buffer = match self.view_mode {
      ViewMode::Color => self.bundle.texture_buffer_selective_color(extents),
      ViewMode::Kind => self.bundle.texture_buffer_selective_kind(extents),
      ViewMode::Terrain => self.bundle.texture_buffer_selective_terrain(extents),
      ViewMode::Continent => self.bundle.texture_buffer_selective_continent(extents),
      ViewMode::Coastal => self.bundle.texture_buffer_selective_coastal(extents)
    };

    UpdateTexture::update(&mut self.texture, &mut (), Format::Rgba8, &buffer, offset, size)
      .expect("unable to update texture");
  }

  fn brush_info(&self) -> String {
    match self.view_mode {
      ViewMode::Color => match self.brush.color_brush {
        Some(color) => format!("color {}", stringify_color(color)),
        None => "color (no brush)".to_owned()
      },
      ViewMode::Kind => match self.brush.kind_brush {
        Some(kind) => format!("type {}", kind.to_str().to_uppercase()),
        None => "type (no brush)".to_owned()
      },
      ViewMode::Terrain => match &self.brush.terrain_brush {
        Some(terrain) => format!("terrain {}", terrain.to_uppercase()),
        None => "terrain (no brush)".to_owned()
      },
      ViewMode::Continent => match self.brush.continent_brush {
        Some(continent) => format!("continent {}", continent),
        None => "continent (no brush)".to_owned()
      },
      ViewMode::Coastal => "coastal".to_owned()
    }
  }

  fn camera_info(&self, cursor_pos: Option<Vector2<f64>>) -> String {
    let zoom_info = format!("{:.2}%", self.camera.scale_factor() * 100.0);
    let cursor_info = cursor_pos
      .and_then(|cursor_pos| self.camera.relative_position_int(cursor_pos))
      .map_or_else(String::new, |[x, y]| format!("{}, {} px", x, y));
    let brush_info = self.brush_info();
    format!("{:<24}{:<24}{}", cursor_info, zoom_info, brush_info)
  }
}



#[derive(Debug, Clone)]
pub struct BrushSettings {
  pub color_brush: Option<Color>,
  pub kind_brush: Option<DefinitionKind>,
  pub terrain_brush: Option<String>,
  pub continent_brush: Option<u16>,
  pub radius: f64
}

impl Default for BrushSettings {
  fn default() -> BrushSettings {
    BrushSettings {
      color_brush: None,
      kind_brush: None,
      terrain_brush: None,
      continent_brush: None,
      radius: 8.0
    }
  }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ViewMode {
  Color,
  Kind,
  Terrain,
  Continent,
  Coastal
}

#[derive(Debug)]
pub struct Camera {
  pub texture_size: Vector2<f64>,
  pub display_matrix: Matrix2x3<f64>,
  pub panning: bool
}

impl Camera {
  fn new(texture: &Texture) -> Self {
    use opengl_graphics::ImageSize;
    let (width, height) = texture.get_size();
    let texture_size = [width as f64, height as f64];
    let display_matrix = vecmath::mat2x3_id()
      .trans_pos(vecmath::vec2_scale(texture_size, -0.5))
      .trans_pos(WINDOW_CENTER);
    Camera {
      texture_size,
      display_matrix,
      panning: false
    }
  }

  pub fn on_mouse_relative(&mut self, rel: Vector2<f64>) {
    if self.panning {
      let rel = vecmath::vec2_scale(rel, self.scale_factor().recip());
      self.display_matrix = self.display_matrix.trans_pos(rel);
    };
  }

  pub fn on_mouse_zoom(&mut self, dz: f64, cursor_pos: Vector2<f64>) {
    let zoom = 2.0f64.powf(dz * ZOOM_SENSITIVITY);
    let cursor_rel = self.relative_position(cursor_pos);
    let cursor_rel_neg = vecmath::vec2_neg(cursor_rel);
    self.display_matrix = self.display_matrix
      .trans_pos(cursor_rel)
      .zoom(zoom)
      .trans_pos(cursor_rel_neg);
  }

  pub fn reset(&mut self) {
    self.display_matrix = vecmath::mat2x3_id()
      .trans_pos(vecmath::vec2_scale(self.texture_size, -0.5))
      .trans_pos(WINDOW_CENTER);
  }

  pub fn set_panning(&mut self, panning: bool) {
    self.panning = panning;
  }

  fn relative_position(&self, pos: Vector2<f64>) -> Vector2<f64> {
    vecmath::row_mat2x3_transform_pos2(self.display_matrix_inv(), pos)
  }

  fn relative_position_int(&self, pos: Vector2<f64>) -> Option<Vector2<u32>> {
    let pos = self.relative_position(pos);
    self.within_dimensions(pos)
      .then(|| [pos[0] as u32, pos[1] as u32])
  }

  fn display_matrix_inv(&self) -> Matrix2x3<f64> {
    vecmath::mat2x3_inv(self.display_matrix)
  }

  pub fn scale_factor(&self) -> f64 {
    (self.display_matrix[0][0] + self.display_matrix[1][1]) / 2.0
  }

  fn within_dimensions(&self, pos: Vector2<f64>) -> bool {
    (0.0..self.texture_size[0]).contains(&pos[0]) &&
    (0.0..self.texture_size[1]).contains(&pos[1])
  }
}
