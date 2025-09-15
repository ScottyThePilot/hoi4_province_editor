pub mod hsl;
pub mod random;
pub mod uord;

use chrono::Local;
use fs_err::File;
use fxhash::{FxHashMap, FxHashSet, FxBuildHasher};
use unicase::UniCase;
use unicase::bytemuck::TransparentWrapper;
use vecmath::Vector2;
use zip::read::ZipArchive;
use zip::result::ZipError;
use zip::write::ZipWriter;

use crate::app::map::{Color, Extents};
use crate::error::Error;

use std::io::{self, prelude::*};
use std::path::{Path, PathBuf};
use std::ops::Range;

#[derive(Debug, Clone)]
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

/// Hacky trait equivalent of the nightly `try_block` feature.
#[macro_export]
macro_rules! try_block {
  ($($t:tt)*) => {
    (|| {
      $($t)*
    })()
  };
}



#[derive(Debug, Clone)]
pub struct ZipFilesMap {
  map: FxHashMap<UniCase<PathBuf>, Vec<u8>>
}

impl ZipFilesMap {
  pub fn new() -> Self {
    ZipFilesMap { map: FxHashMap::default() }
  }

  pub fn with_capacity(capacity: usize) -> Self {
    ZipFilesMap { map: fx_hash_map_with_capacity(capacity) }
  }

  pub fn get(&self, name: impl AsRef<Path>) -> Option<&Vec<u8>> {
    self.map.get(UniCase::wrap_ref(name.as_ref()))
  }

  pub fn get_mut(&mut self, name: impl AsRef<Path>) -> Option<&mut Vec<u8>> {
    self.map.get_mut(UniCase::wrap_ref(name.as_ref()))
  }

  pub fn get_or_insert(&mut self, name: impl Into<PathBuf>) -> &mut Vec<u8> {
    self.map.entry(UniCase::wrap(name.into())).or_default()
  }

  pub fn get_insert_new(&mut self, name: impl Into<PathBuf>) -> &mut Vec<u8> {
    self.map.entry(UniCase::wrap(name.into())).and_modify(Vec::clear).or_default()
  }

  pub fn remove(&mut self, name: impl AsRef<Path>) -> Option<Vec<u8>> {
    self.map.remove(UniCase::wrap_ref(name.as_ref()))
  }

  pub fn from_reader(reader: impl Read + Seek) -> Result<Self, ZipError> {
    let mut zip_reader = ZipArchive::new(reader)?;
    let mut zip_files_map = ZipFilesMap::with_capacity(zip_reader.len());
    for i in 0..zip_reader.len() {
      let mut zip_file = zip_reader.by_index(i)?;
      if let Some(zip_file_name) = zip_file.enclosed_name().map(Path::to_owned) {
        let zip_file_buffer = zip_files_map.get_insert_new(zip_file_name);
        io::copy(&mut zip_file, zip_file_buffer).map_err(ZipError::Io)?;
      };
    };

    Ok(zip_files_map)
  }

  pub fn to_writer(&self, writer: impl Write + Seek, comment: impl Into<String>) -> Result<(), ZipError> {
    let mut zip_writer = ZipWriter::new(writer);
    zip_writer.set_comment(comment);

    for (zip_file_name, zip_file_buffer) in self.map.iter() {
      let zip_file_name = AsRef::<Path>::as_ref(zip_file_name).to_string_lossy();
      zip_writer.start_file(zip_file_name, Default::default())?;
      zip_writer.write_all(zip_file_buffer).map_err(ZipError::Io)?;
    };

    zip_writer.finish()?;

    Ok(())
  }

  pub fn edit<F>(mut source: File, comment: impl Into<String>, callback: F) -> Result<(), Error>
  where F: FnOnce(&mut Self) -> Result<(), Error> {
    let mut zip_files_map = Self::from_reader(&mut source)?;
    source.seek(io::SeekFrom::Start(0)).map_err(ZipError::Io)?;
    source.set_len(0).map_err(ZipError::Io)?;
    callback(&mut zip_files_map)?;
    zip_files_map.to_writer(&mut source, comment)?;
    Ok(())
  }
}
