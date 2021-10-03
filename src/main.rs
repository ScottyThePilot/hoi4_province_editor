#![warn(missing_debug_implementations)]
#[macro_use]
pub mod util;
pub mod app;
pub mod config;
pub mod error;
pub mod events;
pub mod font;

use glutin_window::GlutinWindow;
use opengl_graphics::{GlGraphics, OpenGL};
use piston::window::WindowSettings;

use crate::app::App;
use crate::events::launch;

use std::path::PathBuf;
use std::env;
use std::io;

const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 720;

pub const APPNAME: &str = concat!("HOI4 Province Map Editor v", env!("CARGO_PKG_VERSION"));

fn main() {
  better_panic::install();

  let root = root_dir().expect("unable to find root dir");
  env::set_current_dir(root).expect("unable to set root dir");

  let opengl = OpenGL::V3_2;
  let screen = [WINDOW_WIDTH, WINDOW_HEIGHT];
  let mut window: GlutinWindow = WindowSettings::new(APPNAME, screen)
    .graphics_api(opengl).resizable(false).vsync(true)
    .build().expect("unable to initialize window");
  let mut gl = GlGraphics::new(opengl);
  launch::<App>(&mut window, &mut gl);
}

fn root_dir() -> io::Result<PathBuf> {
  if let Some(manifest_dir) = env::var_os("CARGO_MANIFEST_DIR") {
    return Ok(PathBuf::from(manifest_dir));
  };

  let mut current_exe = dunce::canonicalize(env::current_exe()?)?;

  if current_exe.pop() {
    return Ok(current_exe);
  };

  Err(io::Error::new(io::ErrorKind::Other, "failed to find an application root"))
}
