pub mod alerts;
pub mod canvas;
pub mod format;
pub mod interface;
pub mod map;

use defy::Contextualize;
use glutin::window::CursorIcon;
use graphics::Viewport;
use graphics::context::Context;
use graphics::glyph_cache::rusttype::GlyphCache;
use opengl_graphics::{GlGraphics, Filter, Texture, TextureSettings};
use piston::input::{Key, MouseButton};
use vecmath::Vector2;

use crate::error::Error;
use crate::font;
use crate::events::{EventHandler, KeyMods};
use crate::util::files::{Location, IntoLocation};
use self::alerts::Alerts;
use self::canvas::{Canvas, ToolMode, ViewMode};
use self::interface::{Interface, ButtonId, get_interface};

use std::path::{Path, PathBuf};
use std::fmt;
use std::env;

pub mod colors {
  use graphics::types::Color as DrawColor;

  pub const BLACK: DrawColor = [0.0, 0.0, 0.0, 1.0];
  pub const WHITE: DrawColor = [1.0, 1.0, 1.0, 1.0];
  pub const WHITE_T: DrawColor = [1.0, 1.0, 1.0, 0.25];
  pub const WHITE_TT: DrawColor = [1.0, 1.0, 1.0, 0.015625];
  pub const PROBLEM: DrawColor = [0.875, 0.0, 0.0, 1.0];
  pub const WARNING: DrawColor = [0.875, 0.5, 0.0, 1.0];
  pub const NEUTRAL: DrawColor = [0.25, 0.25, 0.25, 1.0];
  pub const OVERLAY_T: DrawColor = [0.0, 0.0, 0.0, 0.5];

  pub const ADJ_LAND: DrawColor = [0.2, 0.6, 1.0/3.0, 1.0];
  pub const ADJ_SEA: DrawColor = [0.2, 1.0/3.0, 0.6, 1.0];
  pub const ADJ_IMPASSABLE: DrawColor = [0.0, 0.0, 0.0, 1.0];

  const fn color_inactive(value: u16) -> DrawColor {
    let v = value as f32 / 256.0;
    [v, v, v, 1.0]
  }

  const fn color_active(value: u16) -> DrawColor {
    let v = value as f32 / 256.0;
    [v, v, v * 2.0, 1.0]
  }

  pub const BUTTON: DrawColor = color_inactive(48);
  pub const BUTTON_ACTIVE: DrawColor = color_active(48 + 16);
  pub const BUTTON_HOVER: DrawColor = color_inactive(96);
  pub const BUTTON_HOVER_ACTIVE: DrawColor = color_active(96 + 16);

  pub const BUTTON_TOOLBAR: DrawColor = color_inactive(32);
  pub const BUTTON_TOOLBAR_ACTIVE: DrawColor = color_active(32 + 16);
  pub const BUTTON_TOOLBAR_HOVER: DrawColor = color_inactive(80);
  pub const BUTTON_TOOLBAR_HOVER_ACTIVE: DrawColor = color_active(80 + 16);
}

pub type FontGlyphCache = GlyphCache<'static, (), Texture>;

pub struct App {
  pub canvas: Option<Canvas>,
  pub alerts: Alerts,
  pub glyph_cache: FontGlyphCache,
  pub interface: Option<Interface>,
  pub painting: bool
}

impl EventHandler for App {
  fn new(_gl: &mut GlGraphics) -> Self {
    let texture_settings = TextureSettings::new().filter(Filter::Nearest);
    let mut glyph_cache = GlyphCache::from_font(font::get_font(), (), texture_settings);
    glyph_cache.preload_printable_ascii(font::FONT_SIZE).expect("unable to preload font glyphs");

    App {
      canvas: None,
      alerts: Alerts::new(5.0),
      glyph_cache,
      interface: None,
      painting: false
    }
  }

  fn on_init(&mut self) {
    if let Some(path) = std::env::args().nth(1) {
      self.raw_open_map_at(path);
    } else {
      #[cfg(any(debug_assertions, feature = "debug-mode"))]
      self.raw_open_map_at("./test_map.zip");
      #[cfg(not(any(debug_assertions, feature = "debug-mode")))]
      self.alerts.push(Ok("Drag a file, archive, or folder onto the application to load a map"));
    };
  }

  fn on_render(&mut self, ctx: Context, cursor_pos: Option<Vector2<f64>>, gl: &mut GlGraphics) {
    let Some(viewport) = ctx.viewport else { return };
    let ictx = self.get_interface_draw_context();
    let interface = &*get_interface(&mut self.interface, viewport);
    graphics::clear(colors::NEUTRAL, gl);

    if let Some(canvas) = &mut self.canvas {
      canvas.draw(ctx, interface, &mut self.glyph_cache, cursor_pos, gl);
    };

    self.alerts.draw(ctx, interface, &mut self.glyph_cache, gl);
    interface.draw(ctx, ictx, cursor_pos, &mut self.glyph_cache, gl);
  }

  fn on_update(&mut self, dt: f32) {
    if !self.alerts.is_active() {
      self.alerts.tick(dt);
    };
  }

  fn on_key(&mut self, key: Key, state: bool, mods: KeyMods, cursor_pos: Option<Vector2<f64>>) {
    let Some(interface) = self.interface.as_ref() else { return };
    match (&mut self.canvas, state, key) {
      (_, state, Key::Tab) => self.alerts.set_state(state),
      (_, true, Key::O) if mods.ctrl => self.action_open_map(mods.alt),
      (Some(_), true, Key::S) if mods.ctrl && mods.shift => self.action_save_map_as(mods.alt),
      (Some(_), true, Key::S) if mods.ctrl => self.action_save_map(),
      (Some(_), true, Key::R) if mods.ctrl && mods.alt => self.action_reveal_map(),
      (Some(canvas), true, Key::Z) if mods.ctrl => canvas.undo(),
      (Some(canvas), true, Key::Y) if mods.ctrl => canvas.redo(),
      (Some(canvas), true, Key::Space) => canvas.cycle_tool_brush(interface, cursor_pos, mods.shift, &mut self.alerts),
      (Some(canvas), true, Key::Escape) => canvas.cancel_tool(),
      (Some(canvas), true, Key::Return) => canvas.finish_tool(),
      (Some(canvas), true, Key::C) if mods.shift => canvas.calculate_coastal_provinces(),
      (Some(canvas), true, Key::R) if mods.shift => canvas.calculate_recolor_map(),
      (Some(canvas), true, Key::P) if mods.shift => canvas.display_problems(&mut self.alerts),
      (Some(canvas), true, Key::M) if mods.shift => canvas.tool.cycle_brush_mask(),
      (Some(canvas), true, Key::H) => canvas.camera.reset(),
      (Some(canvas), true, Key::A) => canvas.set_tool_mode(ToolMode::PaintArea),
      (Some(canvas), true, Key::B) => canvas.set_tool_mode(ToolMode::PaintBucket),
      (Some(canvas), true, Key::L) => canvas.set_tool_mode(ToolMode::new_lasso()),
      (Some(_), true, Key::D1) => self.action_change_view_mode(ViewMode::Color),
      (Some(_), true, Key::D2) => self.action_change_view_mode(ViewMode::Kind),
      (Some(_), true, Key::D3) => self.action_change_view_mode(ViewMode::Terrain),
      (Some(_), true, Key::D4) => self.action_change_view_mode(ViewMode::Continent),
      (Some(_), true, Key::D5) => self.action_change_view_mode(ViewMode::Coastal),
      (Some(_), true, Key::D6) => self.action_change_view_mode(ViewMode::Adjacencies),
      _ => ()
    };
  }

  fn on_mouse(&mut self, button: MouseButton, state: bool, mods: KeyMods, pos: Vector2<f64>) {
    let Some(interface) = self.interface.as_mut() else { return };
    match (&mut self.canvas, state, button) {
      (_, true, MouseButton::Left) => match interface.on_mouse_click(pos) {
        Ok(id) => self.action_interface_button(id),
        Err(true) => self.action_activate_tool(pos, mods),
        Err(false) => ()
      },
      (Some(_), false, MouseButton::Left) => self.action_deactivate_tool(),
      (Some(canvas), true, MouseButton::Right) => canvas.camera.set_panning(true),
      (Some(canvas), false, MouseButton::Right) => canvas.camera.set_panning(false),
      (Some(canvas), true, MouseButton::Middle) => canvas.pick_tool_brush(interface, pos, &mut self.alerts),
      _ => ()
    };
  }

  fn on_mouse_position(&mut self, pos: Vector2<f64>, mods: KeyMods) {
    let Some(interface) = self.interface.as_mut() else { return };
    interface.on_mouse_position(pos);
    if let Some(canvas) = &mut self.canvas {
      if self.painting && canvas.tool.mode == ToolMode::PaintArea && canvas.view_mode() != ViewMode::Adjacencies {
        // Mouse movement should not activate the tool for the paint bucket and lasso tools
        canvas.activate_tool(interface, pos, mods.shift);
      };
    };
  }

  fn on_mouse_relative(&mut self, rel: Vector2<f64>) {
    if let Some(canvas) = &mut self.canvas {
      canvas.camera.on_mouse_relative(rel);
    };
  }

  fn on_mouse_scroll(&mut self, [_, y]: Vector2<f64>, mods: KeyMods, cursor_pos: Vector2<f64>) {
    let Some(interface) = self.interface.as_ref() else { return };
    let Some(canvas) = &mut self.canvas else { return };

    if mods.shift {
      canvas.change_tool_radius(y);
    } else {
      canvas.camera.on_mouse_zoom(interface, y, cursor_pos);
    };
  }

  fn on_file_drop(&mut self, path: PathBuf) {
    self.raw_open_map_at(path);
  }

  fn on_resize(&mut self, viewport: Viewport) {
    self.interface = Some(Interface::new(viewport));
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
  fn get_interface_draw_context(&self) -> InterfaceDrawContext {
    match &self.canvas {
      Some(canvas) => InterfaceDrawContext {
        view_mode: Some(canvas.view_mode()),
        selected_tool: Some(match &canvas.tool.mode {
          ToolMode::PaintArea => 0,
          ToolMode::PaintBucket => 1,
          ToolMode::Lasso(_) => 2
        }),
        enabled_options: canvas.enabled_options()
      },
      None => InterfaceDrawContext {
        view_mode: None,
        selected_tool: None,
        enabled_options: [false; 3]
      }
    }
  }

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
      (Some(canvas), ToolbarEditToggleLassoSnap) => canvas.toggle_lasso_snap(),
      (Some(canvas), ToolbarEditNextMaskMode) => canvas.tool.cycle_brush_mask(),
      (Some(_), ToolbarViewMode1) => self.action_change_view_mode(ViewMode::Color),
      (Some(_), ToolbarViewMode2) => self.action_change_view_mode(ViewMode::Kind),
      (Some(_), ToolbarViewMode3) => self.action_change_view_mode(ViewMode::Terrain),
      (Some(_), ToolbarViewMode4) => self.action_change_view_mode(ViewMode::Continent),
      (Some(_), ToolbarViewMode5) => self.action_change_view_mode(ViewMode::Coastal),
      (Some(_), ToolbarViewMode6) => self.action_change_view_mode(ViewMode::Adjacencies),
      (Some(canvas), ToolbarViewToggleProvinceIds | SidebarOptionProvinceIds) => canvas.toggle_province_ids(),
      (Some(canvas), ToolbarViewToggleProvinceBoundaries | SidebarOptionProvinceBoundaries) => canvas.toggle_province_boundaries(),
      (Some(canvas), ToolbarViewToggleRiverOverlay | SidebarOptionRiverOverlay) => if canvas.toggle_river_overlay() {
        self.alerts.push(Err("You must have a map with rivers.bmp to use this"));
      },
      (Some(canvas), ToolbarViewResetZoom) => canvas.camera.reset(),
      (_, ToolbarViewFontLicense) => self.handle_result_none(font::view_font_license()),
      (Some(canvas), SidebarToolPaintArea) => canvas.set_tool_mode(ToolMode::PaintArea),
      (Some(canvas), SidebarToolPaintBucket) => canvas.set_tool_mode(ToolMode::PaintBucket),
      (Some(canvas), SidebarToolLasso) => canvas.set_tool_mode(ToolMode::new_lasso()),
      #[cfg(any(debug_assertions, feature = "debug-mode"))]
      (Some(canvas), ToolbarDebugValidatePixelCounts) => canvas.validate_pixel_counts(&mut self.alerts),
      #[cfg(any(debug_assertions, feature = "debug-mode"))]
      (_, ToolbarDebugTriggerCrash) => panic!("debug crash"),
      (None, _) => self.alerts.push(Err("You must have a map loaded to use this")),
    };
  }

  fn action_activate_tool(&mut self, pos: Vector2<f64>, mods: KeyMods) {
    let Some(interface) = self.interface.as_ref() else { return };
    self.painting = true;
    if let Some(canvas) = &mut self.canvas {
      if canvas.view_mode() == ViewMode::Adjacencies && canvas.tool.adjacency_brush.is_none() {
        self.alerts.push(Err("No Adjacency brush selected"));
      } else {
        canvas.activate_tool(interface, pos, mods.shift);
      };
    };
  }

  fn action_deactivate_tool(&mut self) {
    self.painting = false;
    if let Some(canvas) = &mut self.canvas {
      canvas.deactivate_tool();
    };
  }

  fn action_change_view_mode(&mut self, view_mode: ViewMode) {
    self.painting = false;
    if let Some(canvas) = &mut self.canvas {
      canvas.set_view_mode(&mut self.alerts, view_mode);
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
      let mut success_message = format!("Saved map to {}", location);
      let save_operation = canvas.save(&location)?;
      if save_operation.had_id_changes {
        success_message.push_str("\nThe most recent save included modified province IDs, see 'id_changes.txt' for more info");
        success_message.push_str("\nIf you do not need province IDs to be preserved, you may disable it in the config")
      };

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

#[derive(Debug, Clone, Copy)]
pub struct InterfaceDrawContext {
  pub view_mode: Option<ViewMode>,
  pub selected_tool: Option<usize>,
  pub enabled_options: [bool; 3]
}

use rfd::{FileDialog, MessageDialog, MessageDialogResult, MessageButtons, MessageLevel};

fn file_dialog_save_bmp(filename: &str) -> Option<PathBuf> {
  let root = env::current_dir()
    .unwrap_or_else(|_| PathBuf::from("./"));
  FileDialog::new()
    .set_directory(&root)
    .set_file_name(format!("{}.bmp", filename))
    .add_filter("24-bit Bitmap", &["bmp"])
    .save_file()
}

fn file_dialog_save(archive: bool) -> Option<Location> {
  let root = env::current_dir()
    .unwrap_or_else(|_| PathBuf::from("./"));
  if archive {
    FileDialog::new()
      .set_directory(&root)
      .set_file_name("map.zip")
      .add_filter("ZIP Archive", &["zip"])
      .save_file()
      .map(Location::ZipArchive)
  } else {
    FileDialog::new()
      .set_directory(&root)
      .pick_folder()
      .map(Location::Directory)
  }
}

fn file_dialog_open(archive: bool) -> Option<Location> {
  let root = env::current_dir()
    .unwrap_or_else(|_| PathBuf::from("./"));
  if archive {
    FileDialog::new()
      .set_directory(&root)
      .set_file_name("map.zip")
      .add_filter("ZIP Archive", &["zip"])
      .pick_file()
      .map(Location::ZipArchive)
  } else {
    FileDialog::new()
      .set_directory(&root)
      .pick_folder()
      .map(Location::Directory)
  }
}

fn msg_dialog_unsaved_changes_exit() -> bool {
  let result = MessageDialog::new()
    .set_title(crate::APPNAME)
    .set_description("You have unsaved changes, would you like to save them before exiting?")
    .set_level(MessageLevel::Warning)
    .set_buttons(MessageButtons::YesNo)
    .show();

  match result {
    MessageDialogResult::Yes => true,
    MessageDialogResult::No => false,
    _ => unreachable!()
  }
}

fn msg_dialog_unsaved_changes() -> bool {
  let result = MessageDialog::new()
    .set_title(crate::APPNAME)
    .set_description("You have unsaved changes, would you like to save them?")
    .set_level(MessageLevel::Warning)
    .set_buttons(MessageButtons::YesNo)
    .show();

  match result {
    MessageDialogResult::Yes => true,
    MessageDialogResult::No => false,
    _ => unreachable!()
  }
}

pub fn reveal_in_file_browser(path: impl AsRef<Path>) -> Result<(), Error> {
  use std::process::Command;

  let path = crate::util::files::canonicalize(path)?;
  if cfg!(target_os = "windows") {
    Command::new("explorer").arg(&path).status()
      .context("failed to execute command 'explorer'")?;
    Ok(())
  } else if cfg!(target_os = "macos") {
    Command::new("open").arg(&path).status()
      .context("failed to execute command 'open'")?;
    Ok(())
  } else if cfg!(target_os = "linux") {
    Command::new("xdg-open").arg(&path).status()
      .context("failed to execute command 'xdg-open'")?;
    Ok(())
  } else {
    Err("unable to reveal in file browser".into())
  }
}
