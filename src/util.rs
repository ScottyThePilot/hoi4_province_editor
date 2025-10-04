pub mod files;
pub mod hsl;
pub mod random;
pub mod uord;

use chrono::Local;
use vecmath::Vector2;

use crate::app::map::{Color, Extents};

use std::ops::Range;

#[derive(Debug, Clone)]
pub struct XYIter {
  x_range: Range<u32>,
  y_range: Range<u32>,
  i: u64
}

impl XYIter {
  pub fn new(x: Range<u32>, y: Range<u32>) -> Self {
    XYIter { x_range: x, y_range: y, i: 0 }
  }

  pub fn from_extents(extents: Extents) -> Self {
    let x = extents.lower[0]..(extents.upper[0] + 1);
    let y = extents.lower[1]..(extents.upper[1] + 1);
    XYIter::new(x, y)
  }

  pub const fn width(&self) -> u32 {
    self.x_range.end - self.x_range.start
  }

  pub const fn height(&self) -> u32 {
    self.y_range.end - self.y_range.start
  }

  pub const fn area(&self) -> u64 {
    self.width() as u64 * self.height() as u64
  }
}

impl Iterator for XYIter {
  type Item = Vector2<u32>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.i < self.area() {
      let x = self.i.rem_euclid(self.width() as u64) as u32 + self.x_range.start;
      let y = self.i.div_euclid(self.width() as u64) as u32 + self.y_range.start;
      self.i += 1;
      Some([x, y])
    } else {
      None
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    let len = self.area() as usize;
    (len, Some(len))
  }

  fn count(self) -> usize {
    self.len()
  }
}

impl ExactSizeIterator for XYIter {
  fn len(&self) -> usize {
    self.area() as usize
  }
}

pub fn stringify_color(color: Color) -> String {
  format!("({}, {}, {})", color[0], color[1], color[2])
}

pub fn now() -> impl std::fmt::Display + 'static {
  Local::now().format("%Y-%m-%d %H:%M:%S")
}

/// Hacky trait equivalent of the nightly `try_block` feature.
#[macro_export]
macro_rules! try_block {
  ($($t:tt)*) => {
    (|| {
      $($t)*
    })()
  };
}
