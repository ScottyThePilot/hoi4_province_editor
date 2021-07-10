#[warn(missing_debug_implementations)]
extern crate better_panic;
extern crate chrono;
extern crate crossbeam;
extern crate fxhash;
extern crate image;
extern crate native_dialog;
extern crate opengl_graphics;
extern crate glutin;
extern crate glutin_window;
extern crate graphics;
extern crate piston;
extern crate rand;
extern crate rusttype;
extern crate serde;
extern crate serde_multi;
extern crate util_macros;
extern crate vecmath;
extern crate zip;

pub mod app;
pub mod config;
pub mod error;
pub mod util;

use glutin::window::CursorIcon;
use glutin_window::GlutinWindow;
use opengl_graphics::{GlGraphics, OpenGL};
use piston::event_loop::{EventSettings, Events};
use piston::input::{Event, Loop, Input, FileDrag};
use piston::window::{Size, WindowSettings};

use crate::app::App;

use std::path::PathBuf;
use std::env;
use std::io;

const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 720;
const SCREEN: Size = Size {
  width: WINDOW_WIDTH as f64,
  height: WINDOW_HEIGHT as f64
};

pub const APPNAME: &str = concat!("HOI4 Province Map Editor v", env!("CARGO_PKG_VERSION"));

fn main() {
  better_panic::install();

  let root = root_dir().unwrap();
  env::set_current_dir(root).unwrap();

  let opengl = OpenGL::V3_2;
  let mut window: GlutinWindow = WindowSettings::new(APPNAME, SCREEN)
    .graphics_api(opengl).resizable(false).vsync(true)
    .build().expect("unable to initialize window");

  let mut gl = GlGraphics::new(opengl);
  let mut app = App::new(&mut gl);
  let mut cursor = CursorIcon::Default;
  let mut init = true;

  let mut events = Events::new(EventSettings::new());
  while let Some(event) = events.next(&mut window) {
    match event {
      Event::Loop(loop_event) => match loop_event {
        Loop::Update(args) => app.on_update_event(args),
        Loop::Render(args) => gl.draw(args.viewport(), |ctx, gl| {
          app.on_render_event(args, ctx, gl);
        }),
        Loop::AfterRender(_) if init => {
          app.on_init();
          init = false;
        },
        _ => ()
      },
      Event::Input(event, _) => match event {
        Input::Button(args) => app.on_button_event(args),
        Input::Move(motion) => app.on_motion_event(motion),
        Input::Text(string) => app.on_text_event(string),
        Input::FileDrag(FileDrag::Drop(path)) => app.on_file_drop(path),
        Input::Focus(false) | Input::Cursor(false) => app.on_unfocus(),
        Input::Close(_) => {
          app.on_close();
          break;
        },
        _ => ()
      },
      _ => ()
    };

    if app.cursor != cursor {
      cursor = app.cursor;
      window.ctx.window().set_cursor_icon(cursor);
    };
  };
}

fn root_dir() -> io::Result<PathBuf> {
  if let Some(manifest_dir) = env::var_os("CARGO_MANIFEST_DIR") {
    return Ok(PathBuf::from(manifest_dir));
  };

  let mut current_exe = env::current_exe()?.canonicalize()?;

  if current_exe.pop() {
    return Ok(current_exe);
  };

  Err(io::Error::new(io::ErrorKind::Other, "Failed to find an application root"))
}
