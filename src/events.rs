use glutin::window::CursorIcon;
use glutin_window::GlutinWindow;
use graphics::Viewport;
use graphics::context::Context;
use opengl_graphics::GlGraphics;
use piston::event_loop::{EventSettings, Events};
use piston::input::*;
use vecmath::Vector2;

use std::path::PathBuf;

pub trait EventHandler: Sized {
  fn new(gl: &mut GlGraphics) -> Self;
  fn on_init(&mut self) {}
  fn on_render(&mut self, ctx: Context, cursor_pos: Option<Vector2<f64>>, gl: &mut GlGraphics);
  fn on_update(&mut self, dt: f32);
  fn on_key(&mut self, _key: Key, _state: bool, _mods: KeyMods, _pos: Option<Vector2<f64>>) {}
  fn on_mouse(&mut self, _button: MouseButton, _state: bool, _mods: KeyMods, _pos: Vector2<f64>) {}
  fn on_mouse_position(&mut self, _pos: Vector2<f64>, _mods: KeyMods) {}
  fn on_mouse_relative(&mut self, _rel: Vector2<f64>) {}
  fn on_mouse_scroll(&mut self, _s: Vector2<f64>, _mods: KeyMods, _pos: Vector2<f64>) {}
  fn on_file_drop(&mut self, _path: PathBuf) {}
  fn on_resize(&mut self, _viewport: Viewport) {}
  fn on_unfocus(&mut self) {}
  fn on_close(self) {}

  fn get_cursor(&self) -> CursorIcon {
    CursorIcon::Default
  }
}

pub fn launch<H: EventHandler>(window: &mut GlutinWindow, gl: &mut GlGraphics) {
  let mut event_handler = H::new(gl);
  let mut mods = KeyMods::default();
  let mut cursor = CursorIcon::Default;
  let mut cursor_pos: Option<Vector2<f64>> = None;
  let mut init = true;

  let mut events = Events::new(EventSettings::new());
  while let Some(event) = events.next(window) {
    match event {
      Event::Loop(loop_event) => match loop_event {
        Loop::Update(args) => event_handler.on_update(args.dt as f32),
        Loop::Render(args) => if !is_viewport_zero(args.viewport()) {
          gl.draw(args.viewport(), |ctx, gl| {
            event_handler.on_render(ctx, cursor_pos, gl);
          });
        },
        Loop::AfterRender(_) if init => {
          event_handler.on_init();
          init = false;
        },
        _ => ()
      },
      Event::Input(event, _) => match event {
        Input::Button(args) => match args.button {
          Button::Keyboard(Key::LShift) => mods.shift = state(args.state),
          Button::Keyboard(Key::LCtrl) => mods.ctrl = state(args.state),
          Button::Keyboard(Key::LAlt) => mods.alt = state(args.state),
          Button::Keyboard(key) => event_handler.on_key(key, state(args.state), mods, cursor_pos),
          Button::Mouse(button) => if let Some(cursor_pos) = cursor_pos {
            event_handler.on_mouse(button, state(args.state), mods, cursor_pos)
          },
          _ => ()
        },
        Input::Move(Motion::MouseCursor(pos)) => {
          cursor_pos = Some(pos);
          event_handler.on_mouse_position(pos, mods);
        },
        Input::Move(Motion::MouseRelative(rel)) => {
          event_handler.on_mouse_relative(rel);
        },
        Input::Move(Motion::MouseScroll(s)) => {
          event_handler.on_mouse_scroll(s, mods, cursor_pos.unwrap_or([0.0, 0.0]));
        },
        Input::FileDrag(FileDrag::Drop(path)) => {
          event_handler.on_file_drop(path);
        },
        Input::Focus(false) | Input::Cursor(false) => {
          cursor_pos = None;
          event_handler.on_unfocus();
        },
        Input::Close(_) => {
          event_handler.on_close();
          break;
        },
        Input::Resize(resize_args) => {
          if !is_viewport_zero(resize_args.viewport()) {
            event_handler.on_resize(resize_args.viewport());
          };
        },
        _ => ()
      },
      _ => ()
    };

    let new_cursor = event_handler.get_cursor();
    if new_cursor != cursor {
      cursor = new_cursor;
      window.ctx.window().set_cursor_icon(new_cursor);
    };
  };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyMods {
  pub shift: bool,
  pub ctrl: bool,
  pub alt: bool
}

impl Default for KeyMods {
  fn default() -> Self {
    KeyMods {
      shift: false,
      ctrl: false,
      alt: false
    }
  }
}

#[inline(always)]
fn state(state: ButtonState) -> bool {
  match state {
    ButtonState::Press => true,
    ButtonState::Release => false
  }
}

pub fn is_viewport_zero(viewport: Viewport) -> bool {
  viewport.window_size == [0.0, 0.0] || viewport.draw_size == [0, 0]
}
