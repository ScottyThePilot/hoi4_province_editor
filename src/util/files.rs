use ahash::AHashMap;
use fs_err::File;
use unicase::UniCase;
use unicase::bytemuck::TransparentWrapper;
use zip::read::ZipArchive;
use zip::result::ZipError;
use zip::write::ZipWriter;

use crate::error::Error;

use std::io::{self, prelude::*};
use std::path::{Path, PathBuf};



#[derive(Debug, Clone)]
pub struct ZipFilesMap {
  map: AHashMap<UniCase<PathBuf>, Vec<u8>>
}

impl ZipFilesMap {
  pub fn new() -> Self {
    ZipFilesMap { map: AHashMap::default() }
  }

  pub fn with_capacity(capacity: usize) -> Self {
    ZipFilesMap { map: AHashMap::with_capacity(capacity) }
  }

  pub fn try_get(&self, name: impl AsRef<Path>) -> Result<&Vec<u8>, Error> {
    let name = name.as_ref();
    self.get(name).ok_or_else(|| {
      Error::from(format!("could not find file {} in zip", name.display()))
    })
  }

  pub fn try_get_mut(&mut self, name: impl AsRef<Path>) -> Result<&mut Vec<u8>, Error> {
    let name = name.as_ref();
    self.get_mut(name).ok_or_else(|| {
      Error::from(format!("could not find file {} in zip", name.display()))
    })
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
