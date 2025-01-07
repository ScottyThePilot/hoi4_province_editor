#![warn(missing_debug_implementations)]
#![cfg_attr(not(any(debug_assertions, feature = "debug-mode")), windows_subsystem = "windows")]
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
  install_handler();

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

use std::io::prelude::*;

fn write_application_info(mut out: impl Write) -> Result<(), std::io::Error> {
  writeln!(out, "Version: v{}", env!("CARGO_PKG_VERSION"))?;
  writeln!(out, "Debug Assertions Enabled: {:?}", cfg!(debug_assertions))?;
  writeln!(out, "Debug Mode Feature Enabled: {:?}", cfg!(feature = "debug-mode"))?;
  writeln!(out)?;

  Ok(())
}

fn install_handler() {
  use chrono::Local;
  use color_backtrace::{BacktracePrinter, Verbosity};
  use termcolor::NoColor;

  use std::fs::File;
  use std::panic::{set_hook, PanicInfo};
  use std::sync::Mutex;

  let printer = BacktracePrinter::new()
    .verbosity(Verbosity::Full)
    .lib_verbosity(Verbosity::Full)
    .clear_frame_filters();
  let out = Mutex::new(color_backtrace::default_output_stream());
  set_hook(Box::new(move |pi: &PanicInfo| {
    // if either of these are enabled, the console is enabled (on windows)
    if cfg!(any(debug_assertions, feature = "debug-mode")) {
      let mut out_lock = out.lock().unwrap();
      if let Err(err) = printer.print_panic_info(pi, &mut *out_lock) {
        eprintln!("Error while printing panic: {err:?}");
      };
    };

    // only write panic info to file if not on dev profile
    if cfg!(not(debug_assertions)) {
      let now = Local::now().format("%Y%m%d_%H%M%S");
      match File::create(format!("crash_{}.log", now)) {
        Ok(out_file) => {
          if let Err(err) = write_application_info(&out_file) {
            eprintln!("Error while printing application info: {err:?}");
          };

          if let Err(err) = printer.print_panic_info(pi, &mut NoColor::new(&out_file)) {
            eprintln!("Error while printing panic: {err:?}");
          };
        },
        Err(e) => eprintln!("Error creating crash log: {:?}", e)
      };
    };
  }));
}
