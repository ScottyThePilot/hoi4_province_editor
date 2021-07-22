pub mod canvas;
mod alerts;
pub mod format;
pub mod map;

use glutin::window::CursorIcon;
use graphics::context::Context;
use graphics::glyph_cache::rusttype::GlyphCache;
use opengl_graphics::{GlGraphics, Filter, Texture, TextureSettings};
use piston::input::{Key, MouseButton};
use rusttype::Font;
use vecmath::Vector2;

use crate::config::Config;
use crate::error::Error;
use crate::events::{EventHandler, KeyMods};
use self::canvas::{Canvas, ViewMode};
use self::alerts::Alerts;
use self::map::{Location, IntoLocation};

use std::path::{Path, PathBuf};
use std::fmt::Display;
use std::sync::Arc;
use std::env;

pub mod colors {
  use graphics::types::Color;

  pub const WHITE: Color = [1.0, 1.0, 1.0, 1.0];
  pub const NEUTRAL: Color = [0.25, 0.25, 0.25, 1.0];
  pub const OVERLAY_T: Color = [0.0, 0.0, 0.0, 0.5];
}

pub type FontGlyphCache = GlyphCache<'static, (), Texture>;

#[allow(missing_debug_implementations)]
pub struct App {
  pub canvas: Option<Canvas>,
  pub config: Arc<Config>,
  pub alerts: Alerts,
  pub glyph_cache: FontGlyphCache,
  pub painting: bool
}

impl EventHandler for App {
  fn new(_gl: &mut GlGraphics) -> Self {
    const FONT_DATA: &[u8] = include_bytes!("../assets/Consolas.ttf");
    let config = Config::load().expect("unable to load config");
    let texture_settings = TextureSettings::new().filter(Filter::Nearest);
    let font = Font::try_from_bytes(FONT_DATA).expect("unable to load font");
    let mut glyph_cache = GlyphCache::from_font(font, (), texture_settings);
    glyph_cache.preload_printable_ascii(10).expect("unable to preload font glyphs");

    App {
      canvas: None,
      config: Arc::new(config),
      alerts: Alerts::new(5.0),
      glyph_cache,
      painting: false
    }
  }

  fn on_init(&mut self) {
    if cfg!(debug_assertions) {
      // In debug mode, load the custom test map
      self.raw_open_map_at("./test_map.zip");
    } else {
      if let Some(path) = std::env::args().nth(1) {
        self.raw_open_map_at(path);
      } else {
        self.alerts.push_system(Ok("Drag a file, archive, or folder onto the application to load a map"));
      };
    };
  }

  fn on_render(&mut self, ctx: Context, gl: &mut GlGraphics) {
    graphics::clear(colors::NEUTRAL, gl);

    if let Some(canvas) = &self.canvas {
      canvas.draw(ctx, &mut self.glyph_cache, !self.alerts.is_active(), gl);
    };

    self.alerts.draw(ctx, &mut self.glyph_cache, gl);
  }

  fn on_update(&mut self, dt: f32) {
    if !self.alerts.is_active() {
      self.alerts.tick(dt);
    };
  }

  fn on_key(&mut self, key: Key, state: bool, mods: KeyMods) {
    match (&mut self.canvas, state, key) {
      (_, state, Key::Tab) => self.alerts.set_state(state),
      (_, true, Key::O) if mods.ctrl => self.action_open_map(mods.alt),
      (Some(_), true, Key::S) if mods.ctrl && mods.shift => self.action_save_map_as(mods.alt),
      (Some(_), true, Key::S) if mods.ctrl => self.action_save_map(),
      (Some(_), true, Key::R) if mods.ctrl && mods.alt => self.action_reveal_map(),
      (Some(canvas), true, Key::Z) if mods.ctrl => canvas.undo(),
      (Some(canvas), true, Key::Y) if mods.ctrl => canvas.redo(),
      (Some(canvas), true, Key::Space) => canvas.cycle_brush(&mut self.alerts),
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

  fn on_mouse(&mut self, button: MouseButton, state: bool, _mods: KeyMods) {
    match (&mut self.canvas, state, button) {
      (Some(_), true, MouseButton::Left) => self.action_start_painting(),
      (Some(_), false, MouseButton::Left) => self.action_stop_painting(),
      (Some(canvas), true, MouseButton::Right) => canvas.camera.set_panning(true),
      (Some(canvas), false, MouseButton::Right) => canvas.camera.set_panning(false),
      (Some(canvas), true, MouseButton::Middle) => canvas.pick_brush(&mut self.alerts),
      _ => ()
    };
  }

  fn on_mouse_position(&mut self, pos: Vector2<f64>) {
    if let Some(canvas) = &mut self.canvas {
      canvas.camera.on_mouse_position(Some(pos));
      if self.painting {
        canvas.paint_brush();
      };
    };
  }

  fn on_mouse_relative(&mut self, rel: Vector2<f64>) {
    if let Some(canvas) = &mut self.canvas {
      canvas.camera.on_mouse_relative(rel);
    };
  }

  fn on_mouse_scroll(&mut self, [_, y]: Vector2<f64>, mods: KeyMods) {
    if let Some(canvas) = &mut self.canvas {
      if mods.shift {
        canvas.change_brush_radius(y);
      } else {
        canvas.camera.on_mouse_zoom(y);
      };
    };
  }

  fn on_file_drop(&mut self, path: PathBuf) {
    self.raw_open_map_at(path);
  }

  fn on_unfocus(&mut self) {
    self.alerts.set_state(false);
    if let Some(canvas) = &mut self.canvas {
      canvas.camera.on_mouse_position(None);
    };
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

  fn action_start_painting(&mut self) {
    self.painting = true;
    if let Some(canvas) = &mut self.canvas {
      canvas.paint_brush();
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
    if let Some(_) = &self.canvas {
      if let Some(location) = file_dialog_save(archive) {
        self.raw_save_map_at(location.clone());
      };
    };
  }

  fn action_reveal_map(&mut self) {
    if let Some(canvas) = &self.canvas {
      let result = reveal_in_file_browser(canvas.location().as_path());
      self.handle_result_none(result);
    };
  }

  fn raw_open_map_at(&mut self, location: impl IntoLocation) {
    fn inner(app: &mut App, location: impl IntoLocation) -> Result<String, Error> {
      let location = location.into_location()?;
      let success_message = format!("Loaded map from {}", location);
      let canvas = Canvas::load(location, Arc::clone(&app.config))?;
      app.canvas = Some(canvas);
      Ok(success_message)
    }

    let result = inner(self, location);
    self.handle_result(result);
  }

  fn raw_save_map_at(&mut self, location: impl IntoLocation) {
    fn inner(app: &mut App, location: impl IntoLocation) -> Result<String, Error> {
      let canvas = app.canvas.as_mut().ok_or(Error::from("no canvas loaded"))?;
      let location = location.into_location()?;
      let success_message = format!("Saved map to {}", location);
      canvas.save(&location)?;
      canvas.set_location(location);
      canvas.modified = false;
      Ok(success_message)
    }

    let result = inner(self, location);
    self.handle_result(result);
  }

  fn handle_result_none(&mut self, result: Result<(), Error>) {
    if let Err(err) = result {
      self.alerts.push_system(Err(format!("Error: {}", err)));
    };
  }

  fn handle_result<T: Display>(&mut self, result: Result<T, Error>) {
    self.alerts.push_system(match result {
      Ok(text) => Ok(format!("{}", text)),
      Err(err) => Err(format!("Error: {}", err))
    });
  }
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

  let command = match () {
    #[cfg(target_os = "windows")] _ => "explorer",
    #[cfg(target_os = "macos")] _ => "open",
    #[cfg(target_os = "linux")] _ => "xdg-open",
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    _ => return Err("unable to reveal in file browser".into())
  };

  let success = Command::new(command).arg(path.as_ref())
    .output()?.status.success();

  match success {
    true => Ok(()),
    false => Err("unable to reveal in file browser".into())
  }
}
