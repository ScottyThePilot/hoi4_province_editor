pub mod canvas;
mod command;
mod console;
pub mod format;
pub mod map;

use glutin::window::CursorIcon;
use graphics::types::Color;
use graphics::context::Context;
use graphics::glyph_cache::rusttype::GlyphCache;
use opengl_graphics::{GlGraphics, Filter, Texture, TextureSettings};
use piston::input::{RenderArgs, UpdateArgs, ButtonArgs, Motion};
use rusttype::Font;

use crate::config::Config;
use crate::error::Error;
use self::canvas::{Canvas, ViewMode};
use self::console::{Console, ConsoleHandle, ConsoleAction};
use self::map::{Location, IntoLocation};

use std::path::{Path, PathBuf};
use std::time::Duration;
use std::sync::Arc;
use std::env;
use std::fmt;
use std::mem;

const FONT_DATA: &[u8] = include_bytes!("../assets/Consolas.ttf");
const NEUTRAL: Color = [0.25, 0.25, 0.25, 1.0];

pub type FontGlyphCache = GlyphCache<'static, (), Texture>;

pub struct App {
  pub canvas: Option<Canvas>,
  pub config: Arc<Config>,
  pub console: Console,
  pub cursor: CursorIcon,
  pub glyph_cache: FontGlyphCache,
  pub activate_console: bool,
  pub painting: bool,
  pub mod_shift: bool,
  pub mod_ctrl: bool,
  pub mod_alt: bool
}

impl App {
  pub fn new(_gl: &mut GlGraphics) -> Self {
    let config = Config::load().expect("unable to load config");
    let texture_settings = TextureSettings::new().filter(Filter::Nearest);
    let font = Font::try_from_bytes(FONT_DATA).expect("unable to load font");
    let mut glyph_cache = GlyphCache::from_font(font, (), texture_settings);
    glyph_cache.preload_printable_ascii(10).expect("unable to preload font glyphs");
    let console = Console::new(Duration::from_secs(5));

    App {
      canvas: None,
      config: Arc::new(config),
      console,
      cursor: CursorIcon::Crosshair,
      glyph_cache,
      activate_console: false,
      painting: false,
      mod_shift: false,
      mod_ctrl: false,
      mod_alt: false
    }
  }

  pub fn on_init(&mut self) {
    if cfg!(debug_assertions) {
      // In debug mode, load the custom test map
      self.raw_open_map_at("./test_map.zip");
    } else {
      if let Some(path) = std::env::args().nth(1) {
        self.raw_open_map_at(path);
      } else {
        self.console.push_system(Ok("Drag a file, archive, or folder onto the application to load a map"));
      };
    };
  }

  pub fn on_render_event(&mut self, _args: RenderArgs, ctx: Context, gl: &mut GlGraphics) {
    graphics::clear(NEUTRAL, gl);

    if let Some(canvas) = &self.canvas {
      canvas.draw(ctx, &mut self.glyph_cache, !self.console.is_active(), gl);
    };

    self.console.draw(ctx.transform, &mut self.glyph_cache, gl);
  }

  pub fn on_update_event(&mut self, _args: UpdateArgs) {
    self.console.tick();

    if mem::replace(&mut self.activate_console, false) {
      self.console.activate();
      self.mod_shift = false;
      self.mod_ctrl = false;
      self.mod_alt = false;
    };
  }

  pub fn on_button_event(&mut self, args: ButtonArgs) {
    use piston::input::{Key, MouseButton, Button};
    use piston::input::ButtonState::Press as Dn;
    use piston::input::ButtonState::Release as Up;
    const CONSOLE_KEY: Option<i32> = Some(41);
    match (self.console.is_active(), &mut self.canvas, args.state, args.button) {
      (true, _, Dn, _) if args.scancode == CONSOLE_KEY => self.action_deactivate_console(),
      (false, _, Dn, _) if args.scancode == CONSOLE_KEY => self.action_activate_console(),
      (true, _, Dn, Button::Keyboard(Key::Left)) => self.console.action(ConsoleAction::Left),
      (true, _, Dn, Button::Keyboard(Key::Right)) => self.console.action(ConsoleAction::Right),
      (true, _, Dn, Button::Keyboard(Key::Backspace)) => self.console.action(ConsoleAction::Backspace),
      (true, _, Dn, Button::Keyboard(Key::Delete)) => self.console.action(ConsoleAction::Delete),
      (true, _, Dn, Button::Keyboard(Key::Return)) => self.action_execute_command(),
      (false, _, Dn, Button::Keyboard(Key::LShift)) => self.mod_shift = true,
      (false, _, Dn, Button::Keyboard(Key::LCtrl)) => self.mod_ctrl = true,
      (false, _, Dn, Button::Keyboard(Key::LAlt)) => self.mod_alt = true,
      (false, _, Up, Button::Keyboard(Key::LShift)) => self.mod_shift = false,
      (false, _, Up, Button::Keyboard(Key::LCtrl)) => self.mod_ctrl = false,
      (false, _, Up, Button::Keyboard(Key::LAlt)) => self.mod_alt = false,
      (false, _, Dn, Button::Keyboard(Key::O)) if self.mod_ctrl => self.action_open_map(self.mod_alt),
      (false, Some(canvas), state, button) => match (state, button) {
        (Dn, Button::Mouse(MouseButton::Left)) => self.action_start_painting(),
        (Up, Button::Mouse(MouseButton::Left)) => self.action_stop_painting(),
        (Dn, Button::Mouse(MouseButton::Right)) => canvas.camera.set_panning(true),
        (Up, Button::Mouse(MouseButton::Right)) => canvas.camera.set_panning(false),
        (Dn, Button::Mouse(MouseButton::Middle)) => canvas.pick_brush(self.console.handle()),
        (Dn, Button::Keyboard(Key::Z)) if self.mod_ctrl => canvas.undo(),
        (Dn, Button::Keyboard(Key::Y)) if self.mod_ctrl => canvas.redo(),
        (Dn, Button::Keyboard(Key::S)) if self.mod_ctrl && self.mod_shift => self.action_save_map_as(self.mod_alt),
        (Dn, Button::Keyboard(Key::S)) if self.mod_ctrl => self.action_save_map(),
        (Dn, Button::Keyboard(Key::R)) if self.mod_ctrl && self.mod_alt => self.action_reveal_map(),
        (Dn, Button::Keyboard(Key::Space)) => canvas.cycle_brush(self.console.handle()),
        (Dn, Button::Keyboard(Key::C)) if self.mod_shift => canvas.calculate_coastal_provinces(),
        (Dn, Button::Keyboard(Key::R)) if self.mod_shift => canvas.calculate_recolor_map(),
        (Dn, Button::Keyboard(Key::P)) if self.mod_shift => canvas.display_problems(self.console.handle()),
        (Dn, Button::Keyboard(Key::H)) => canvas.camera.reset(),
        (Dn, Button::Keyboard(Key::D1)) => canvas.set_view_mode(self.console.handle(), ViewMode::Color),
        (Dn, Button::Keyboard(Key::D2)) => canvas.set_view_mode(self.console.handle(), ViewMode::Kind),
        (Dn, Button::Keyboard(Key::D3)) => canvas.set_view_mode(self.console.handle(), ViewMode::Terrain),
        (Dn, Button::Keyboard(Key::D4)) => canvas.set_view_mode(self.console.handle(), ViewMode::Continent),
        (Dn, Button::Keyboard(Key::D5)) => canvas.set_view_mode(self.console.handle(), ViewMode::Coastal),
        _ => ()
      },
      _ => ()
    };
  }

  pub fn on_motion_event(&mut self, motion: Motion) {
    match (&mut self.canvas, motion) {
      (canvas, Motion::MouseCursor(pos)) => {
        if let Some(canvas) = canvas {
          canvas.camera.on_mouse_position(Some(pos));
          if self.painting {
            canvas.paint_brush();
          };
        };
      },
      (Some(canvas), Motion::MouseRelative(rel)) => {
        canvas.camera.on_mouse_relative(rel);
      },
      (Some(canvas), Motion::MouseScroll([_, d])) => {
        if self.mod_shift {
          canvas.change_brush_radius(d);
        } else {
          canvas.camera.on_mouse_zoom(d);
        };
      },
      _ => ()
    };
  }

  pub fn on_text_event(&mut self, string: String) {
    self.console.action(ConsoleAction::Insert(string));
  }

  pub fn on_file_drop(&mut self, path: PathBuf) {
    self.raw_open_map_at(path);
  }

  pub fn on_unfocus(&mut self) {
    if let Some(canvas) = &mut self.canvas {
      canvas.camera.on_mouse_position(None);
    };
  }

  pub fn on_close(mut self) {
    if self.is_canvas_modified() {
      if msg_dialog_unsaved_changes_exit() {
        self.action_save_map();
      };
    };
  }

  pub fn action_execute_command(&mut self) {
    if let Some(line) = self.console.enter_command() {
      let canvas = self.canvas.as_mut();
      let console = self.console.handle();
      command::line(line, console, canvas);
    };
  }

  fn is_canvas_modified(&self) -> bool {
    if let Some(canvas) = &self.canvas {
      canvas.modified
    } else {
      false
    }
  }

  fn action_deactivate_console(&mut self) {
    self.console.deactivate();
  }

  fn action_activate_console(&mut self) {
    self.activate_console = true;
    if let Some(canvas) = &mut self.canvas {
      canvas.camera.set_panning(false);
      canvas.paint_stop();
    };
    self.painting = false;
    self.mod_shift = false;
    self.mod_ctrl = false;
    self.mod_alt = false;
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
      reveal_in_file_browser(canvas.location().as_path())
        .report(self.console.handle());
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

  fn handle_result<T: ToString>(&mut self, result: Result<T, Error>) {
    self.console.push_system(match result {
      Ok(text) => Ok(text.to_string()),
      Err(err) => Err(err.to_string())
    });
  }
}

trait Report {
  type Return;

  fn report(self, handle: ConsoleHandle) -> Self::Return;
}

impl<T, E: fmt::Display> Report for Result<T, E> {
  type Return = Option<T>;

  fn report(self, mut handle: ConsoleHandle) -> Option<T> {
    match self {
      Ok(value) => Some(value),
      Err(err) => {
        handle.push_system(Err(format!("{}", err)));
        None
      }
    }
  }
}

impl Report for Option<String> {
  type Return = ();

  fn report(self, mut handle: ConsoleHandle) {
    if let Some(string) = self {
      handle.push_system(Ok(string));
    };
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
