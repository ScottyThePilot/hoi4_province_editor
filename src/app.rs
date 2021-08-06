pub mod alerts;
pub mod canvas;
pub mod format;
pub mod interface;
pub mod map;

use glutin::window::CursorIcon;
use graphics::context::Context;
use graphics::glyph_cache::rusttype::GlyphCache;
use opengl_graphics::{GlGraphics, Filter, Texture, TextureSettings};
use piston::input::{Key, MouseButton};
use rusttype::Font;
use vecmath::Vector2;

use crate::error::Error;
use crate::events::{EventHandler, KeyMods};
use self::alerts::Alerts;
use self::canvas::{Canvas, ViewMode};
use self::interface::{Interface, ButtonId};
use self::map::{Location, IntoLocation};

use std::path::{Path, PathBuf};
use std::fmt;
use std::env;

pub mod colors {
  use graphics::types::Color;

  pub const WHITE: Color = [1.0, 1.0, 1.0, 1.0];
  pub const WHITE_T: Color = [1.0, 1.0, 1.0, 0.25];
  pub const PROBLEM: Color = [0.875, 0.0, 0.0, 1.0];
  pub const NEUTRAL: Color = [0.25, 0.25, 0.25, 1.0];
  pub const OVERLAY_T: Color = [0.0, 0.0, 0.0, 0.5];

  pub const BUTTON: Color = [0.1875, 0.1875, 0.1875, 1.0];
  pub const BUTTON_HOVER: Color = [0.3750, 0.3750, 0.3750, 1.0];

  pub const BUTTON_TOOLBAR: Color = [0.1250, 0.1250, 0.1250, 1.0];
  pub const BUTTON_TOOLBAR_HOVER: Color = [0.3125, 0.3125, 0.3125, 1.0];
}

pub const FONT_SIZE: u32 = 9;

pub type FontGlyphCache = GlyphCache<'static, (), Texture>;

pub struct App {
  pub canvas: Option<Canvas>,
  pub alerts: Alerts,
  pub glyph_cache: FontGlyphCache,
  pub interface: Interface,
  pub painting: bool
}

impl EventHandler for App {
  fn new(_gl: &mut GlGraphics) -> Self {
    const FONT_DATA: &[u8] = include_bytes!("../assets/Consolas.ttf");
    let texture_settings = TextureSettings::new().filter(Filter::Nearest);
    let font = Font::try_from_bytes(FONT_DATA).expect("unable to load font");
    let mut glyph_cache = GlyphCache::from_font(font.clone(), (), texture_settings);
    glyph_cache.preload_printable_ascii(10).expect("unable to preload font glyphs");
    let interface = self::interface::construct_interface(&font);

    App {
      canvas: None,
      alerts: Alerts::new(5.0),
      glyph_cache,
      interface,
      painting: false
    }
  }

  fn on_init(&mut self) {
    #[cfg(debug_assertions)]
    self.raw_open_map_at("./test_map.zip");

    #[cfg(not(debug_assertions))]
    if let Some(path) = std::env::args().nth(1) {
      self.raw_open_map_at(path);
    } else {
      self.alerts.push(Ok("Drag a file, archive, or folder onto the application to load a map"));
    };
  }

  fn on_render(&mut self, ctx: Context, cursor_pos: Option<Vector2<f64>>, gl: &mut GlGraphics) {
    graphics::clear(colors::NEUTRAL, gl);

    if let Some(canvas) = &self.canvas {
      canvas.draw(ctx, &mut self.glyph_cache, cursor_pos, gl);
    };

    self.alerts.draw(ctx, &mut self.glyph_cache, gl);
    self.interface.draw(ctx, cursor_pos, &mut self.glyph_cache, gl);
  }

  fn on_update(&mut self, dt: f32) {
    if !self.alerts.is_active() {
      self.alerts.tick(dt);
    };
  }

  fn on_key(&mut self, key: Key, state: bool, mods: KeyMods, cursor_pos: Option<Vector2<f64>>) {
    match (&mut self.canvas, state, key) {
      (_, state, Key::Tab) => self.alerts.set_state(state),
      (_, true, Key::O) if mods.ctrl => self.action_open_map(mods.alt),
      (Some(_), true, Key::S) if mods.ctrl && mods.shift => self.action_save_map_as(mods.alt),
      (Some(_), true, Key::S) if mods.ctrl => self.action_save_map(),
      (Some(_), true, Key::R) if mods.ctrl && mods.alt => self.action_reveal_map(),
      (Some(canvas), true, Key::Z) if mods.ctrl => canvas.undo(),
      (Some(canvas), true, Key::Y) if mods.ctrl => canvas.redo(),
      (Some(canvas), true, Key::Space) => canvas.cycle_brush(cursor_pos, &mut self.alerts),
      (Some(canvas), true, Key::C) if mods.shift => canvas.calculate_coastal_provinces(),
      (Some(canvas), true, Key::R) if mods.shift => canvas.calculate_recolor_map(),
      (Some(canvas), true, Key::P) if mods.shift => canvas.display_problems(&mut self.alerts),
      (Some(canvas), true, Key::H) => canvas.camera.reset(),
      (Some(canvas), true, Key::D1) => canvas.set_view_mode(&mut self.alerts, ViewMode::Color),
      (Some(canvas), true, Key::D2) => canvas.set_view_mode(&mut self.alerts, ViewMode::Kind),
      (Some(canvas), true, Key::D3) => canvas.set_view_mode(&mut self.alerts, ViewMode::Terrain),
      (Some(canvas), true, Key::D4) => canvas.set_view_mode(&mut self.alerts, ViewMode::Continent),
      (Some(canvas), true, Key::D5) => canvas.set_view_mode(&mut self.alerts, ViewMode::Coastal),
      _ => ()
    };
  }

  fn on_mouse(&mut self, button: MouseButton, state: bool, _mods: KeyMods, pos: Vector2<f64>) {
    match (&mut self.canvas, state, button) {
      (Some(_), true, MouseButton::Left) => match self.interface.on_mouse_click(pos) {
        Ok(id) => self.action_interface_button(id),
        Err(true) => self.action_start_painting(pos),
        Err(false) => ()
      },
      (Some(_), false, MouseButton::Left) => self.action_stop_painting(),
      (Some(canvas), true, MouseButton::Right) => canvas.camera.set_panning(true),
      (Some(canvas), false, MouseButton::Right) => canvas.camera.set_panning(false),
      (Some(canvas), true, MouseButton::Middle) => canvas.pick_brush(pos, &mut self.alerts),
      _ => ()
    };
  }

  fn on_mouse_position(&mut self, pos: Vector2<f64>) {
    self.interface.on_mouse_position(pos);
    if let Some(canvas) = &mut self.canvas {
      if self.painting {
        canvas.paint_brush(pos);
      };
    };
  }

  fn on_mouse_relative(&mut self, rel: Vector2<f64>) {
    if let Some(canvas) = &mut self.canvas {
      canvas.camera.on_mouse_relative(rel);
    };
  }

  fn on_mouse_scroll(&mut self, [_, y]: Vector2<f64>, mods: KeyMods, cursor_pos: Vector2<f64>) {
    if let Some(canvas) = &mut self.canvas {
      if mods.shift {
        canvas.change_brush_radius(y);
      } else {
        canvas.camera.on_mouse_zoom(y, cursor_pos);
      };
    };
  }

  fn on_file_drop(&mut self, path: PathBuf) {
    self.raw_open_map_at(path);
  }

  fn on_unfocus(&mut self) {
    self.alerts.set_state(false);
  }

  fn on_close(mut self) {
    if self.is_canvas_modified() {
      if msg_dialog_unsaved_changes_exit() {
        self.action_save_map();
      };
    };
  }

  fn get_cursor(&self) -> CursorIcon {
    CursorIcon::Crosshair
  }
}

impl App {
  fn is_canvas_modified(&self) -> bool {
    if let Some(canvas) = &self.canvas {
      canvas.modified
    } else {
      false
    }
  }

  pub fn action_interface_button(&mut self, id: ButtonId) {
    use self::interface::ButtonId::*;
    match (&mut self.canvas, id) {
      (_, ToolbarFileOpenFileArchive) => self.action_open_map(true),
      (_, ToolbarFileOpenFolder) => self.action_open_map(false),
      (Some(_), ToolbarFileSave) => self.action_save_map(),
      (Some(_), ToolbarFileSaveAsArchive) => self.action_save_map_as(true),
      (Some(_), ToolbarFileSaveAsFolder) => self.action_save_map_as(false),
      (Some(_), ToolbarFileReveal) => self.action_reveal_map(),
      (Some(_), ToolbarFileExportLandMap) => self.action_export_land_map(),
      (Some(_), ToolbarFileExportTerrainMap) => self.action_export_terrain_map(),
      (Some(canvas), ToolbarEditUndo) => canvas.undo(),
      (Some(canvas), ToolbarEditRedo) => canvas.redo(),
      (Some(canvas), ToolbarEditCoastal) => canvas.calculate_coastal_provinces(),
      (Some(canvas), ToolbarEditRecolor) => canvas.calculate_recolor_map(),
      (Some(canvas), ToolbarEditProblems) => canvas.display_problems(&mut self.alerts),
      (Some(canvas), ToolbarViewMode1) => canvas.set_view_mode(&mut self.alerts, ViewMode::Color),
      (Some(canvas), ToolbarViewMode2) => canvas.set_view_mode(&mut self.alerts, ViewMode::Kind),
      (Some(canvas), ToolbarViewMode3) => canvas.set_view_mode(&mut self.alerts, ViewMode::Terrain),
      (Some(canvas), ToolbarViewMode4) => canvas.set_view_mode(&mut self.alerts, ViewMode::Continent),
      (Some(canvas), ToolbarViewMode5) => canvas.set_view_mode(&mut self.alerts, ViewMode::Coastal),
      (Some(canvas), ToolbarViewResetZoom) => canvas.camera.reset(),
      _ => ()
    };
  }

  fn action_start_painting(&mut self, pos: Vector2<f64>) {
    self.painting = true;
    if let Some(canvas) = &mut self.canvas {
      canvas.paint_brush(pos);
    };
  }

  fn action_stop_painting(&mut self) {
    self.painting = false;
    if let Some(canvas) = &mut self.canvas {
      canvas.paint_stop();
    };
  }

  fn action_open_map(&mut self, archive: bool) {
    if let Some(canvas) = &mut self.canvas {
      if canvas.modified {
        if msg_dialog_unsaved_changes() {
          self.action_save_map();
        };
      };
    };

    if let Some(location) = file_dialog_open(archive) {
      self.raw_open_map_at(location);
    };
  }

  fn action_save_map(&mut self) {
    if let Some(canvas) = &self.canvas {
      let location = canvas.location().clone();
      self.raw_save_map_at(location);
    };
  }

  fn action_save_map_as(&mut self, archive: bool) {
    if self.canvas.is_some() {
      if let Some(location) = file_dialog_save(archive) {
        self.raw_save_map_at(location);
      };
    };
  }

  fn action_reveal_map(&mut self) {
    if let Some(canvas) = &self.canvas {
      let path = canvas.location().as_path();
      let result = reveal_in_file_browser(path);
      self.handle_result_none(result);
    };
  }

  fn action_export_land_map(&mut self) {
    if let Some(canvas) = &self.canvas {
      if let Some(path) = file_dialog_save_bmp("land") {
        canvas.export_land_map(path, &mut self.alerts);
      };
    };
  }

  fn action_export_terrain_map(&mut self) {
    if let Some(canvas) = &self.canvas {
      if let Some(path) = file_dialog_save_bmp("terrain") {
        canvas.export_terrain_map(path, &mut self.alerts);
      };
    };
  }

  fn raw_open_map_at(&mut self, location: impl IntoLocation) {
    let result = crate::try_block!{
      let location = location.into_location()?;
      let success_message = format!("Loaded map from {}", location);
      let canvas = Canvas::load(location)?;
      self.canvas = Some(canvas);
      Ok(success_message)
    };

    self.handle_result(result);
  }

  fn raw_save_map_at(&mut self, location: impl IntoLocation) {
    let result = crate::try_block!{
      let canvas = self.canvas.as_mut()
        .ok_or_else(|| Error::from("no canvas loaded"))?;
      let location = location.into_location()?;
      let success_message = format!("Saved map to {}", location);
      canvas.save(&location)?;
      canvas.set_location(location);
      canvas.modified = false;
      Ok(success_message)
    };

    self.handle_result(result);
  }

  fn handle_result_none(&mut self, result: Result<(), Error>) {
    if let Err(err) = result {
      self.alerts.push(Err(format!("Error: {}", err)));
    };
  }

  fn handle_result<T: fmt::Display>(&mut self, result: Result<T, Error>) {
    self.alerts.push(match result {
      Ok(text) => Ok(text.to_string()),
      Err(err) => Err(format!("Error: {}", err))
    });
  }
}

impl fmt::Debug for App {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.debug_struct("App")
      .field("canvas", &self.canvas)
      .field("alerts", &self.alerts)
      .field("glyph_cache", &format_args!("..."))
      .field("interface", &self.interface)
      .field("painting", &self.painting)
      .finish()
  }
}

fn file_dialog_save_bmp(filename: &str) -> Option<PathBuf> {
  use native_dialog::FileDialog;
  let root = env::current_dir()
    .unwrap_or_else(|_| PathBuf::from("./"));
  FileDialog::new()
    .set_location(&root)
    .set_filename(&format!("{}.bmp", filename))
    .add_filter("24-bit Bitmap", &["bmp"])
    .show_save_single_file()
    .expect("error displaying file dialog")
}

fn file_dialog_save(archive: bool) -> Option<Location> {
  use native_dialog::FileDialog;
  let root = env::current_dir()
    .unwrap_or_else(|_| PathBuf::from("./"));
  if archive {
    FileDialog::new()
      .set_location(&root)
      .set_filename("map.zip")
      .add_filter("ZIP Archive", &["zip"])
      .show_save_single_file()
      .expect("error displaying file dialog")
      .map(Location::Zip)
  } else {
    FileDialog::new()
      .set_location(&root)
      .show_open_single_dir()
      .expect("error displaying file dialog")
      .map(Location::Dir)
  }
}

fn file_dialog_open(archive: bool) -> Option<Location> {
  use native_dialog::FileDialog;
  let root = env::current_dir()
    .unwrap_or_else(|_| PathBuf::from("./"));
  if archive {
    FileDialog::new()
      .set_location(&root)
      .set_filename("map.zip")
      .add_filter("ZIP Archive", &["zip"])
      .show_open_single_file()
      .expect("error displaying file dialog")
      .map(Location::Zip)
  } else {
    FileDialog::new()
      .set_location(&root)
      .show_open_single_dir()
      .expect("error displaying file dialog")
      .map(Location::Dir)
  }
}

fn msg_dialog_unsaved_changes_exit() -> bool {
  use native_dialog::{MessageDialog, MessageType};
  MessageDialog::new()
    .set_title(crate::APPNAME)
    .set_text("You have unsaved changes, would you like to save them before exiting?")
    .set_type(MessageType::Warning)
    .show_confirm()
    .expect("error displaying file dialog")
}

fn msg_dialog_unsaved_changes() -> bool {
  use native_dialog::{MessageDialog, MessageType};
  MessageDialog::new()
    .set_title(crate::APPNAME)
    .set_text("You have unsaved changes, would you like to save them?")
    .set_type(MessageType::Warning)
    .show_confirm()
    .expect("error displaying file dialog")
}

pub fn reveal_in_file_browser(path: impl AsRef<Path>) -> Result<(), Error> {
  use std::process::Command;

  let path = dunce::canonicalize(path)?;
  if cfg!(target_os = "windows") {
    Command::new("explorer").arg(&path).status()?;
    Ok(())
  } else if cfg!(target_os = "macos") {
    Command::new("open").arg(&path).status()?;
    Ok(())
  } else if cfg!(target_os = "linux") {
    Command::new("xdg-open").arg(&path).status()?;
    Ok(())
  } else {
    Err("unable to reveal in file browser".into())
  }
}
