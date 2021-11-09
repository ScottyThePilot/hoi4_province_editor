use fxhash::FxHashSet;
use graphics::Transformed;
use graphics::types::Color as DrawColor;
use graphics::context::Context;
use graphics::ellipse::Ellipse;
use image::RgbImage;
use itertools::Itertools;
use opengl_graphics::{Filter, GlGraphics, Texture, TextureSettings};
use vecmath::{Matrix2x3, Vector2};

use super::{colors, FontGlyphCache};
use super::alerts::Alerts;
use super::map::*;
use super::format::DefinitionKind;
use crate::{WINDOW_WIDTH, WINDOW_HEIGHT};
use crate::config::Config;
use crate::font::{self, FONT_SIZE};
use crate::util::stringify_color;
use crate::util::uord::UOrd;
use crate::error::Error;

use std::path::Path;
use std::fs::File;
use std::io::BufWriter;
use std::fmt;

const ZOOM_SENSITIVITY: f64 = 0.125;
const WINDOW_CENTER: Vector2<f64> = [WINDOW_WIDTH as f64 / 2.0, WINDOW_HEIGHT as f64 / 2.0];

pub struct Canvas {
  bundle: Bundle,
  history: History,
  texture: Texture,
  view_mode: ViewMode,
  problems: Vec<Problem>,
  unknown_terrains: Option<FxHashSet<String>>,
  location: Location,
  show_province_ids: bool,
  show_province_boundaries: bool,
  pub tool: ToolSettings,
  pub modified: bool,
  pub camera: Camera
}

impl Canvas {
  pub fn load(location: Location) -> Result<Canvas, Error> {
    let bundle = Bundle::load(&location, Config::load()?)?;
    let history = History::new(bundle.config.max_undo_states, &bundle.map);
    let texture_settings = TextureSettings::new().mag(Filter::Nearest);
    let texture = Texture::from_image(&bundle.texture_buffer_color(), &texture_settings);
    // The test map is very small with large ocean provinces, the 'too large box' errors go nuts
    let problems = if cfg!(debug_assertions) { Vec::new() } else { bundle.generate_problems() };
    let unknown_terrains = bundle.search_unknown_terrains();
    let show_province_ids = bundle.config.preserve_ids;
    let camera = Camera::new(&texture);

    Ok(Canvas {
      bundle,
      history,
      texture,
      view_mode: ViewMode::default(),
      tool: ToolSettings::default(),
      problems,
      unknown_terrains,
      location,
      show_province_ids,
      show_province_boundaries: false,
      modified: false,
      camera
    })
  }

  pub fn save(&mut self, location: &Location) -> Result<(), Error> {
    if self.bundle.config.generate_coastal_on_save {
      self.history.calculate_coastal_provinces(&mut self.bundle);
    };

    self.bundle.save(location)?;
    self.location = location.clone();
    self.modified = false;

    Ok(())
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

  pub fn config(&self) -> &Config {
    &self.bundle.config
  }

  pub fn draw(&self, ctx: Context, glyph_cache: &mut FontGlyphCache, cursor_pos: Option<Vector2<f64>>, gl: &mut GlGraphics) {
    use super::alerts::PADDING;
    use super::interface::get_sidebar_width;

    let transform = ctx.transform.append_transform(self.camera.display_matrix);
    graphics::image(&self.texture, transform, gl);

    if self.camera.scale_factor() > 1.0 && self.show_province_boundaries {
      self.draw_boundaries(ctx, gl);
    };

    if self.view_mode == ViewMode::Adjacencies {
      self.draw_adjacencies(ctx, cursor_pos, gl);
    } else if self.camera.scale_factor() > 1.0 && self.show_province_ids {
      self.draw_ids(ctx, glyph_cache, gl);
    };

    self.draw_problems(ctx, gl);

    self.draw_tool(ctx, cursor_pos, gl);

    let camera_info = self.camera_info(cursor_pos);
    let pos = [PADDING[0] + get_sidebar_width(), WINDOW_HEIGHT as f64 - PADDING[1] * 1.25];
    let transform = ctx.transform.trans_pos(pos);
    graphics::text(colors::WHITE, FONT_SIZE, &camera_info, glyph_cache, transform, gl)
      .expect("unable to draw text");
  }

  fn draw_ids(&self, ctx: Context, glyph_cache: &mut FontGlyphCache, gl: &mut GlGraphics) {
    for (_color, province_data) in self.bundle.map.iter_province_data() {
      let preserved_id = province_data.preserved_id
        .map_or_else(|| "X".to_owned(), |id| id.to_string());
      let color = match self.view_mode {
        ViewMode::Color | ViewMode::Adjacencies => match province_data.kind {
          ProvinceKind::Land | ProvinceKind::Lake => colors::BLACK,
          ProvinceKind::Sea | ProvinceKind::Unknown => colors::WHITE
        },
        ViewMode::Kind | ViewMode::Terrain => colors::BLACK,
        ViewMode::Continent => colors::WHITE,
        ViewMode::Coastal => match province_data.coastal {
          Some(true) => colors::BLACK,
          Some(false) | None => colors::WHITE
        }
      };

      let center_of_mass = vecmath::vec2_add([0.5, 0.5], province_data.center_of_mass());
      let center_of_mass = self.camera.compute_position(center_of_mass);
      if self.camera.within_viewport(center_of_mass) {
        let preserved_id = preserved_id.to_string();
        let offset = [
          font::get_width_metric_str(&preserved_id) / -2.0,
          font::get_v_metrics().ascent - font::get_height_metric() / 2.0
        ];
        let transform = ctx.transform.trans_pos(center_of_mass).trans_pos(offset);
        graphics::text(color, FONT_SIZE, &preserved_id, glyph_cache, transform, gl)
          .expect("unable to draw text");
      };
    };
  }

  fn draw_adjacencies(&self, ctx: Context, cursor_pos: Option<Vector2<f64>>, gl: &mut GlGraphics) {
    //let nearest = cursor_pos
    //  .map(|cursor_pos| self.camera.relative_position(cursor_pos))
    //  .and_then(|pos| self.bundle.map.get_rel_nearest(pos))
    //  .map(|(rel, _dist)| rel);

    if let (Some(sel), Some(kind), Some(cursor_pos)) = (self.tool.adjacency_selection, self.tool.adjacency_brush, cursor_pos) {
      let color = kind.draw_color();
      let pos = self.bundle.map.get_province(sel).center_of_mass();
      let pos = self.camera.compute_position(pos);

      graphics::line_from_to(color, 2.0, pos, cursor_pos, ctx.transform, gl);
    };

    for (rel, connection_data) in self.bundle.map.iter_connection_data() {
      let color = connection_data.kind.draw_color();
      let (center1, center2) = self.bundle.map.get_connection_positions(rel);
      let center1 = self.camera.compute_position(center1);
      let center2 = self.camera.compute_position(center2);

      graphics::line_from_to(color, 2.0, center1, center2, ctx.transform, gl);
    };
  }

  fn draw_boundaries(&self, ctx: Context, gl: &mut GlGraphics) {
    for boundary in self.bundle.map.iter_boundaries() {
      let (b1, b2) = boundary_to_line(boundary).into_tuple();
      let b1 = self.camera.compute_position([b1[0] as f64, b1[1] as f64]);
      let b2 = self.camera.compute_position([b2[0] as f64, b2[1] as f64]);
      if self.camera.within_viewport(b1) || self.camera.within_viewport(b2) {
        let color = match self.view_mode {
          ViewMode::Color | ViewMode::Adjacencies => {
            drawable_color(boundary_color(&self.bundle.map, boundary))
          },
          ViewMode::Kind | ViewMode::Terrain => colors::BLACK,
          ViewMode::Continent => colors::WHITE,
          ViewMode::Coastal => colors::NEUTRAL
        };

        graphics::line_from_to(color, 1.0, b1, b2, ctx.transform, gl);
      };
    };
  }

  fn draw_problems(&self, ctx: Context, gl: &mut GlGraphics) {
    let extras = self.bundle.config.extra_warnings.enabled;
    for problem in self.problems.iter() {
      problem.draw(ctx, extras, &self.camera, gl);
    };
  }

  fn draw_tool(&self, ctx: Context, cursor_pos: Option<Vector2<f64>>, gl: &mut GlGraphics) {
    let color = if self.tool.color_brush.is_some() { colors::WHITE } else { colors::WHITE_T };
    match (self.view_mode, &self.tool.mode, cursor_pos) {
      (ViewMode::Color, ToolMode::PaintArea, Some(cursor_pos)) => {
        let ellipse = Ellipse::new_border(color, 0.5).resolution(16);
        let r = self.tool.radius * self.camera.scale_factor();
        let transform = ctx.transform.trans_pos(cursor_pos);
        ellipse.draw_from_to([r, r], [-r, -r], &Default::default(), transform, gl);
      },
      (ViewMode::Color, ToolMode::Lasso(lasso), cursor_pos) => {
        let can_finish = cursor_pos
          .map(|cursor_pos| lasso.can_finish(&self.camera, cursor_pos))
          .unwrap_or(false);
        let points = lasso.iter()
          .map(|pos| self.camera.compute_position(pos))
          .collect::<Vec<Vector2<f64>>>();
        let first_point = points.first().cloned();
        let last_point = if can_finish { first_point } else { cursor_pos };

        if let (true, Some(first_point)) = (can_finish, first_point) {
          let ellipse = Ellipse::new(color).resolution(6);
          let transform = ctx.transform.trans_pos(first_point);
          ellipse.draw_from_to([5.0, 5.0], [-5.0, -5.0], &Default::default(), transform, gl);
        };

        let lines = points.into_iter()
          .chain(last_point.into_iter())
          .tuple_windows::<(_, _)>();
        for (pos1, pos2) in lines {
          graphics::line_from_to(color, 0.5, pos1, pos2, ctx.transform, gl);
        };
      },
      _ => ()
    };
  }

  pub fn toggle_province_ids(&mut self) {
    self.show_province_ids = !self.show_province_ids;
  }

  pub fn toggle_province_boundaries(&mut self) {
    self.show_province_boundaries = !self.show_province_boundaries;
  }

  pub fn toggle_lasso_snap(&mut self) {
    self.tool.lasso_snap = !self.tool.lasso_snap;
  }

  pub fn reload_config(&mut self, alerts: &mut Alerts) {
    match Config::load() {
      Ok(config) => {
        self.bundle.config = config;
        alerts.push(Ok("Reloaded config"));
      },
      Err(err) => alerts.push(Err(format!("Error: {}", err)))
    };
  }

  pub fn export_land_map<P: AsRef<Path>>(&self, path: P, alerts: &mut Alerts) {
    if let Some(image) = self.bundle.image_buffer_mapgen_land() {
      let path = path.as_ref();
      match export_image_buffer(path, image) {
        Ok(()) => alerts.push(Ok(format!("Exported land map to {}", path.display()))),
        Err(err) => alerts.push(Err(format!("Error: {}", err)))
      };
    } else {
      alerts.push(Err("Error: province with unknown type present"));
    };
  }

  pub fn export_terrain_map<P: AsRef<Path>>(&self, path: P, alerts: &mut Alerts) {
    if let Some(unknown_terrains) = self.unknown_terrains() {
      alerts.push(Err(unknown_terrains));
    } else {
      let path = path.as_ref();
      let image = self.bundle.image_buffer_mapgen_terrain().unwrap();
      match export_image_buffer(path, image) {
        Ok(()) => alerts.push(Ok(format!("Exported terrain map to {}", path.display()))),
        Err(err) => alerts.push(Err(format!("Error: {}", err)))
      };
    };
  }

  pub fn undo(&mut self) {
    if let Some(commit) = self.history.undo(&mut self.bundle.map) {
      self.bundle.map.recalculate_all_boundaries();
      self.problems.clear();
      if self.bundle.config.change_view_mode_on_undo {
        self.view_mode = commit.view_mode;
      };
      self.refresh();
    };
  }

  pub fn redo(&mut self) {
    if let Some(commit) = self.history.redo(&mut self.bundle.map) {
      self.bundle.map.recalculate_all_boundaries();
      self.problems.clear();
      if self.bundle.config.change_view_mode_on_undo {
        self.view_mode = commit.view_mode;
      };
      self.refresh();
    };
  }

  pub fn calculate_coastal_provinces(&mut self) {
    self.history.calculate_coastal_provinces(&mut self.bundle);
    self.view_mode = ViewMode::Coastal;
    self.refresh();
  }

  pub fn calculate_recolor_map(&mut self) {
    self.history.calculate_recolor_map(&mut self.bundle);
    self.view_mode = ViewMode::Color;
    self.tool.color_brush = None;
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

  pub fn set_view_mode(&mut self, alerts: &mut Alerts, view_mode: ViewMode) {
    if let (ViewMode::Terrain, Some(unknown_terrains)) = (view_mode, self.unknown_terrains()) {
      alerts.push(Err(unknown_terrains));
    } else if view_mode != self.view_mode {
      if let ViewMode::Color | ViewMode::Adjacencies = self.view_mode {
        self.cancel_tool();
      };

      self.view_mode = view_mode;
      self.refresh();
    };
  }

  pub fn set_tool_mode(&mut self, mode: ToolMode) {
    self.tool.mode = mode;
  }

  pub fn cycle_tool_brush(&mut self, cursor_pos: Option<Vector2<f64>>, alerts: &mut Alerts) {
    match self.view_mode {
      ViewMode::Color => {
        let kind = self.tool.kind_brush
          .map(ProvinceKind::from)
          .or_else(|| {
            let pos = cursor_pos.and_then(|cursor_pos| {
              self.camera.relative_position_int(cursor_pos)
            })?;
            Some(self.bundle.map.get_province_at(pos).kind)
          })
          .unwrap_or(ProvinceKind::Land);
        let color = self.bundle.random_color_pure(kind);
        self.tool.color_brush = Some(color);
        alerts.push(Ok(format!("Brush set to color {}", stringify_color(color))))
      },
      ViewMode::Kind => {
        let kind = self.tool.kind_brush;
        let kind = self.bundle.config.cycle_kinds(kind);
        self.tool.kind_brush = Some(kind);
        alerts.push(Ok(format!("Brush set to type {}", kind.to_str().to_uppercase())));
      },
      ViewMode::Terrain => {
        let terrain = self.tool.terrain_brush.as_deref();
        let terrain = self.bundle.config.cycle_terrains(terrain);
        alerts.push(Ok(format!("Brush set to terrain {}", terrain.to_uppercase())));
        self.tool.terrain_brush = Some(terrain);
      },
      ViewMode::Continent => {
        let continent = self.tool.continent_brush;
        let continent = self.bundle.config.cycle_continents(continent);
        self.tool.continent_brush = Some(continent);
        alerts.push(Ok(format!("Brush set to continent {}", continent)));
      },
      ViewMode::Coastal => (),
      ViewMode::Adjacencies => {
        let adjacency_kind = self.tool.adjacency_brush;
        let adjacency_kind = self.bundle.config.cycle_connection(adjacency_kind);
        self.tool.adjacency_brush = Some(adjacency_kind);
        alerts.push(Ok(format!("Brush set to adjacencies {}", adjacency_kind.to_str().to_uppercase())));
      }
    };
  }

  pub fn pick_tool_brush(&mut self, cursor_pos: Vector2<f64>, alerts: &mut Alerts) {
    if let Some(pos) = self.camera.relative_position_int(cursor_pos) {
      let color = self.bundle.map.get_color_at(pos);
      let province_data = self.bundle.map.get_province_at(pos);
      match self.view_mode {
        ViewMode::Color => {
          self.tool_paint_end();
          self.tool.color_brush = Some(color);
          alerts.push(Ok(format!("Picked color {}", stringify_color(color))));
        },
        ViewMode::Kind => if let Some(kind) = province_data.kind.to_definition_kind() {
          self.tool.kind_brush = Some(kind);
          alerts.push(Ok(format!("Picked type {}", kind.to_str().to_uppercase())));
        },
        ViewMode::Terrain => if province_data.terrain != "unknown" {
          let terrain = province_data.terrain.as_str();
          self.tool.terrain_brush = Some(terrain.to_owned());
          alerts.push(Ok(format!("Picked terrain {}", terrain.to_uppercase())));
        },
        ViewMode::Continent => {
          let continent = province_data.continent;
          self.tool.continent_brush = Some(continent);
          alerts.push(Ok(format!("Picked continent {}", continent)));
        },
        ViewMode::Coastal => (),
        ViewMode::Adjacencies => ()
      };
    };
  }

  pub fn change_tool_radius(&mut self, d: f64) {
    const LIMIT: f64 = std::f64::consts::SQRT_2 / 2.0;
    if let (ViewMode::Color, ToolMode::PaintArea) = (self.view_mode, &self.tool.mode) {
      let r = self.tool.radius;
      let d = d * (1.0 + 0.025 * r);
      self.tool.radius = (r + d).max(LIMIT);
    };
  }

  /// Activates the tool, ie, performs a left-click action
  pub fn activate_tool(&mut self, cursor_pos: Vector2<f64>) {
    match self.view_mode {
      ViewMode::Color => match self.tool.mode {
        ToolMode::PaintArea => self.tool_paint_brush(cursor_pos),
        ToolMode::PaintBucket => self.tool_paint_bucket(cursor_pos),
        ToolMode::Lasso(_) => self.tool_lasso_add_point(cursor_pos)
      },
      ViewMode::Adjacencies => self.tool_connect_activate(cursor_pos),
      _ => self.tool_paint_brush(cursor_pos)
    };
  }

  /// Deactivates the tool, ie, performs a release-left-click action
  pub fn deactivate_tool(&mut self) {
    if let ToolMode::PaintArea = self.tool.mode {
      self.tool_paint_end();
    };
  }

  pub fn cancel_tool(&mut self) {
    self.tool.adjacency_selection = None;
    if let ToolMode::Lasso(lasso) = &mut self.tool.mode {
      lasso.drain();
    };
  }

  pub fn finish_tool(&mut self) {
    if let ToolMode::Lasso(lasso) = &mut self.tool.mode {
      let lasso = lasso.drain();
      self.tool_lasso_finish(lasso);
    };
  }

  fn tool_lasso_add_point(&mut self, cursor_pos: Vector2<f64>) {
    if let ToolMode::Lasso(lasso) = &mut self.tool.mode {
      if lasso.can_finish(&self.camera, cursor_pos) {
        let lasso = lasso.drain();
        self.tool_lasso_finish(lasso);
      } else {
        let point = self.camera.relative_position(cursor_pos);
        let point = if self.tool.lasso_snap {
          [point[0].round(), point[1].round()]
        } else {
          point
        };

        lasso.push(point);
      };
    };
  }

  fn tool_lasso_finish(&mut self, lasso: Vec<Vector2<f64>>) {
    if let (Some(color), ViewMode::Color) = (self.tool.color_brush, self.view_mode) {
      if lasso.len() > 2 {
        if let Some(extents) = self.history.paint_pixel_lasso(&mut self.bundle, lasso, color, self.tool.brush_mask) {
          self.problems.clear();
          self.modified = true;
          self.refresh_selective(extents);
        };
      };
    };
  }

  fn tool_paint_brush(&mut self, cursor_pos: Vector2<f64>) {
    if let Some(pos) = self.camera.relative_position_int(cursor_pos) {
      if let (Some(color), ViewMode::Color) = (self.tool.color_brush, self.view_mode) {
        let pos = self.camera.relative_position(cursor_pos);
        if let Some(extents) = self.history.paint_pixel_area(&mut self.bundle, pos, self.tool.radius, color, self.tool.brush_mask) {
          self.problems.clear();
          self.modified = true;
          self.refresh_selective(extents);
        };
      } else if let (Some(kind), ViewMode::Kind) = (self.tool.kind_brush, self.view_mode) {
        if let Some(extents) = self.history.paint_province_kind(&mut self.bundle, pos, kind) {
          self.modified = true;
          self.refresh_selective(extents);
        };
      } else if let (Some(terrain), ViewMode::Terrain) = (&self.tool.terrain_brush, self.view_mode) {
        if let Some(extents) = self.history.paint_province_terrain(&mut self.bundle, pos, terrain.clone()) {
          self.modified = true;
          self.refresh_selective(extents);
        };
      } else if let (Some(continent), ViewMode::Continent) = (self.tool.continent_brush, self.view_mode) {
        if let Some(extents) = self.history.paint_province_continent(&mut self.bundle, pos, continent) {
          self.modified = true;
          self.refresh_selective(extents);
        };
      };
    };
  }

  fn tool_paint_end(&mut self) {
    self.history.finish_last_step(&self.bundle.map);
  }

  fn tool_paint_bucket(&mut self, cursor_pos: Vector2<f64>) {
    if let Some(pos) = self.camera.relative_position_int(cursor_pos) {
      if let (Some(fill_color), ViewMode::Color) = (self.tool.color_brush, self.view_mode) {
        if let Some(extents) = self.history.paint_pixel_bucket(&mut self.bundle, pos, fill_color, self.tool.brush_mask) {
          self.problems.clear();
          self.modified = true;
          self.refresh_selective(extents);
        };
      };
    };
  }

  fn tool_connect_activate(&mut self, cursor_pos: Vector2<f64>) {
    if let Some(pos) = self.camera.relative_position_int(cursor_pos) {
      let which = self.bundle.map.get_color_at(pos);
      if let Some(kind) = self.tool.adjacency_brush {
        if let Some(color) = self.tool.adjacency_selection.take() {
          self.history.add_or_remove_connection(&mut self.bundle, UOrd::new(which, color), kind);
        } else {
          self.tool.adjacency_selection = Some(which);
        };
      };
    };
  }

  pub fn validate_pixel_counts(&self, alerts: &mut Alerts) {
    if self.bundle.map.validate_pixel_counts() {
      alerts.push(Ok("Validation successful"));
    } else {
      alerts.push(Err("Validation failed"));
    };
  }

  fn unknown_terrains(&self) -> Option<String> {
    if let Some(unknown_terrains) = &self.unknown_terrains {
      let unknown_terrains = unknown_terrains.iter().map(|s| s.to_uppercase()).join(", ");
      Some(format!("Terrain mode unavailable, unknown terrains present: {}", unknown_terrains))
    } else {
      None
    }
  }

  fn refresh(&mut self) {
    let buffer = match self.view_mode {
      ViewMode::Color => self.bundle.texture_buffer_color(),
      ViewMode::Kind => self.bundle.texture_buffer_kind(),
      ViewMode::Terrain => self.bundle.texture_buffer_terrain(),
      ViewMode::Continent => self.bundle.texture_buffer_continent(),
      ViewMode::Coastal => self.bundle.texture_buffer_coastal(),
      ViewMode::Adjacencies => self.bundle.texture_buffer_color()
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
      ViewMode::Coastal => self.bundle.texture_buffer_selective_coastal(extents),
      ViewMode::Adjacencies => self.bundle.texture_buffer_selective_color(extents)
    };

    UpdateTexture::update(&mut self.texture, &mut (), Format::Rgba8, &buffer, offset, size)
      .expect("unable to update texture");
  }

  fn brush_info(&self) -> String {
    match self.view_mode {
      ViewMode::Color => match self.tool.color_brush {
        Some(color) => format!("Color {}", stringify_color(color)),
        None => "Color (No Brush)".to_owned()
      },
      ViewMode::Kind => match self.tool.kind_brush {
        Some(kind) => format!("Type {}", kind.to_str().to_uppercase()),
        None => "Type (No Brush)".to_owned()
      },
      ViewMode::Terrain => match &self.tool.terrain_brush {
        Some(terrain) => format!("Terrain {}", terrain.to_uppercase()),
        None => "Terrain (No Brush)".to_owned()
      },
      ViewMode::Continent => match self.tool.continent_brush {
        Some(continent) => format!("Continent {}", continent),
        None => "Continent (No Brush)".to_owned()
      },
      ViewMode::Coastal => "Coastal".to_owned(),
      ViewMode::Adjacencies => match self.tool.adjacency_brush {
        Some(connection) => format!("Adjacencies {}", connection.to_str().to_uppercase()),
        None => "Adjacencies (No Brush)".to_owned()
      }
    }
  }

  fn brush_mask_info(&self) -> String {
    if self.view_mode == ViewMode::Color {
      match self.tool.brush_mask {
        Some(brush_mask) => format!("Mask {}", brush_mask.to_str().to_uppercase()),
        None => "No Mask".to_owned()
      }
    } else {
      String::new()
    }
  }

  fn camera_info(&self, cursor_pos: Option<Vector2<f64>>) -> String {
    let zoom_info = format!("{:.2}%", self.camera.scale_factor() * 100.0);
    let cursor_info = cursor_pos
      .and_then(|cursor_pos| self.camera.relative_position_int(cursor_pos))
      .map_or_else(String::new, |[x, y]| format!("{}, {} px", x, y));
    let brush_info = self.brush_info();
    let brush_mask_info = self.brush_mask_info();
    format!("{:<24}{:<24}{:<24}{}", cursor_info, zoom_info, brush_info, brush_mask_info)
  }
}

impl fmt::Debug for Canvas {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.debug_struct("Canvas")
      .field("bundle", &self.bundle)
      .field("history", &self.history)
      .field("texture", &format_args!("..."))
      .field("view_mode", &self.view_mode)
      .field("tool", &self.tool)
      .field("problems", &self.problems)
      .field("unknown_terrains", &self.unknown_terrains)
      .field("location", &self.location)
      .field("modified", &self.modified)
      .field("camera", &self.camera)
      .finish_non_exhaustive()
  }
}



#[derive(Debug, Clone)]
pub struct ToolSettings {
  pub color_brush: Option<Color>,
  pub kind_brush: Option<DefinitionKind>,
  pub terrain_brush: Option<String>,
  pub continent_brush: Option<u16>,
  pub adjacency_brush: Option<ConnectionKind>,
  pub adjacency_selection: Option<Color>,
  pub brush_mask: Option<BrushMask>,
  pub lasso_snap: bool,
  pub radius: f64,
  pub mode: ToolMode
}

impl ToolSettings {
  pub fn cycle_brush_mask(&mut self) {
    self.brush_mask = match self.brush_mask {
      None => Some(BrushMask::LandLakes),
      Some(BrushMask::LandLakes) => Some(BrushMask::Sea),
      Some(BrushMask::Sea) => None
    }
  }
}

impl Default for ToolSettings {
  fn default() -> ToolSettings {
    ToolSettings {
      color_brush: None,
      kind_brush: None,
      terrain_brush: None,
      continent_brush: None,
      adjacency_brush: None,
      adjacency_selection: None,
      brush_mask: None,
      lasso_snap: false,
      radius: 8.0,
      mode: ToolMode::default()
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolMode {
  PaintArea,
  PaintBucket,
  Lasso(Lasso)
}

impl ToolMode {
  pub fn new_lasso() -> Self {
    ToolMode::Lasso(Lasso(Vec::new()))
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lasso(pub Vec<Vector2<f64>>);

impl Lasso {
  fn can_finish(&self, camera: &Camera, cursor_pos: Vector2<f64>) -> bool {
    if let &[point, _, _, ..] = self.0.as_slice() {
      let point = camera.compute_position(point);
      vecmath::vec2_len(vecmath::vec2_sub(point, cursor_pos)) < 5.0
    } else {
      false
    }
  }

  fn drain(&mut self) -> Vec<Vector2<f64>> {
    std::mem::replace(&mut self.0, Vec::new())
  }

  fn push(&mut self, point: Vector2<f64>) {
    self.0.push(point);
  }

  fn iter(&self) -> std::iter::Copied<std::slice::Iter<Vector2<f64>>> {
    self.0.iter().copied()
  }
}

impl Default for ToolMode {
  fn default() -> ToolMode {
    ToolMode::PaintArea
  }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BrushMask {
  LandLakes,
  Sea
}

impl BrushMask {
  #[inline]
  pub fn includes(&self, kind: impl Into<ProvinceKind>) -> bool {
    match (self, kind.into()) {
      (BrushMask::LandLakes, ProvinceKind::Land) => true,
      (BrushMask::LandLakes, ProvinceKind::Lake) => true,
      (BrushMask::Sea, ProvinceKind::Sea) => true,
      (_, ProvinceKind::Unknown) => true,
      _ => false
    }
  }

  fn to_str(self) -> &'static str {
    match self {
      BrushMask::LandLakes => "land + lakes",
      BrushMask::Sea => "sea"
    }
  }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ViewMode {
  Color,
  Kind,
  Terrain,
  Continent,
  Coastal,
  Adjacencies
}

impl Default for ViewMode {
  fn default() -> ViewMode {
    ViewMode::Color
  }
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

  /// Converts a point from camera space to map space
  pub(super) fn relative_position(&self, pos: Vector2<f64>) -> Vector2<f64> {
    vecmath::row_mat2x3_transform_pos2(self.display_matrix_inv(), pos)
  }

  pub(super) fn relative_position_int(&self, pos: Vector2<f64>) -> Option<Vector2<u32>> {
    let pos = self.relative_position(pos);
    self.within_dimensions(pos)
      .then(|| [pos[0] as u32, pos[1] as u32])
  }

  /// Converts from map space to camera space
  pub(super) fn compute_position(&self, pos: Vector2<f64>) -> Vector2<f64> {
    vecmath::row_mat2x3_transform_pos2(self.display_matrix, pos)
  }

  #[inline]
  fn display_matrix_inv(&self) -> Matrix2x3<f64> {
    vecmath::mat2x3_inv(self.display_matrix)
  }

  #[inline]
  pub fn scale_factor(&self) -> f64 {
    (self.display_matrix[0][0] + self.display_matrix[1][1]) / 2.0
  }

  #[inline]
  pub(super) fn within_dimensions(&self, pos: Vector2<f64>) -> bool {
    0.0 <= pos[0] && pos[0] < self.texture_size[0] &&
    0.0 <= pos[1] && pos[1] < self.texture_size[1]
  }

  #[inline]
  pub(super) fn within_viewport(&self, pos: Vector2<f64>) -> bool {
    0.0 <= pos[0] && pos[0] < WINDOW_WIDTH as f64 &&
    0.0 <= pos[1] && pos[1] < WINDOW_HEIGHT as f64
  }
}

fn export_image_buffer<P: AsRef<Path>>(path: P, image: RgbImage) -> Result<(), Error> {
  super::map::write_rgb_bmp_image(BufWriter::new(File::create(path)?), &image)
}

#[inline]
fn drawable_color(color: Color) -> DrawColor {
  [color[0] as f32 / 255.0, color[1] as f32 / 255.0, color[2] as f32 / 255.0, 1.0]
}
