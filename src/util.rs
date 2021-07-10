pub mod csv;
pub mod hsl;
pub mod random;
pub mod uord;

use chrono::Local;
use fxhash::{FxHashMap, FxHashSet, FxBuildHasher};
use vecmath::Vector2;

use crate::app::map::{Color, Extents};

use std::ops::Range;

pub struct XYIter {
  x_range: Range<u32>,
  y_range: Range<u32>,
  x: u32,
  y: u32,
  done: bool
}

impl XYIter {
  pub fn new(x: Range<u32>, y: Range<u32>) -> Self {
    XYIter {
      x: x.start,
      y: y.start,
      x_range: x,
      y_range: y,
      done: false
    }
  }

  pub fn from_extents(extents: Extents) -> Self {
    let x = extents.lower[0]..(extents.upper[0] + 1);
    let y = extents.lower[1]..(extents.upper[1] + 1);
    XYIter::new(x, y)
  }
}

impl Iterator for XYIter {
  type Item = Vector2<u32>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.done {
      None
    } else {
      let item = [self.x, self.y];
      self.x += 1;
      if self.x >= self.x_range.end {
        self.y += 1;
        self.x = self.x_range.start;
        if self.y >= self.y_range.end {
          self.done = true;
          self.y = self.y_range.start;
        };
      };

      Some(item)
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    let len = self.x_range.len() * self.y_range.len();
    (len, Some(len))
  }
}

impl ExactSizeIterator for XYIter {}

pub fn stringify_color(color: Color) -> String {
  format!("({}, {}, {})", color[0], color[1], color[2])
}

pub fn fx_hash_map_with_capacity<K, V>(capacity: usize) -> FxHashMap<K, V> {
  FxHashMap::with_capacity_and_hasher(capacity, FxBuildHasher::default())
}

pub fn fx_hash_set_with_capacity<T>(capacity: usize) -> FxHashSet<T> {
  FxHashSet::with_capacity_and_hasher(capacity, FxBuildHasher::default())
}

pub fn now() -> impl std::fmt::Display + 'static {
  Local::now().format("%Y-%m-%d %H:%M:%S")
}
