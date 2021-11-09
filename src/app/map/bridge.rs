//! Anything relating to loading or saving map data
use fxhash::FxHashMap;
use image::{Rgb, RgbImage, DynamicImage, ColorType};
use image::codecs::bmp::{BmpDecoder, BmpEncoder};
use zip::read::ZipArchive;
use zip::write::ZipWriter;

use super::*;
use crate::app::format::{Adjacency, Definition, ParseCsv};
use crate::config::Config;
use crate::error::Error;
use crate::util::{fx_hash_map_with_capacity, fx_hash_set_with_capacity};
use crate::util::uord::UOrd;

use std::path::{Path, PathBuf};
use std::collections::hash_map::Entry;
use std::io::{self, Cursor, BufWriter, Read, Write};
use std::cmp::Ordering;
use std::fs::File;
use std::fmt;

fn open_file(path: impl AsRef<Path>) -> Result<File, String> {
  let path = path.as_ref();
  File::open(path).map_err(|err| format!("Unable to open {}: {}", path.display(), err))
}

fn create_file(path: impl AsRef<Path>) -> Result<File, String> {
  let path = path.as_ref();
  File::create(path).map_err(|err| format!("Unable to create {}: {}", path.display(), err))
}

#[derive(Debug, Clone)]
pub enum Location {
  Zip(PathBuf),
  Dir(PathBuf)
}

impl Location {
  pub fn as_path(&self) -> &Path {
    match self {
      Location::Zip(path) => path,
      Location::Dir(path) => path
    }
  }

  fn from_path(mut path: PathBuf) -> Result<Self, Error> {
    if let Some(ext) = path.extension() {
      let name = path.file_name().expect("infallible");
      if ext == "zip" {
        return Ok(Location::Zip(path));
      } else if name == "provinces.bmp" || name == "definition.csv" {
        path.pop();
        return Ok(Location::Dir(path));
      };
    };

    if path.is_dir() {
      Ok(Location::Dir(path))
    } else {
      Err("Invalid location".into())
    }
  }
}

impl fmt::Display for Location {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Location::Zip(path) => write!(f, "archive {}", path.display()),
      Location::Dir(path) => write!(f, "directory {}", path.display())
    }
  }
}

pub trait IntoLocation {
  fn into_location(self) -> Result<Location, Error>;
}

impl IntoLocation for Location {
  fn into_location(self) -> Result<Location, Error> {
    Ok(self)
  }
}

impl IntoLocation for &str {
  fn into_location(self) -> Result<Location, Error> {
    Location::from_path(PathBuf::from(self))
  }
}

impl IntoLocation for String {
  fn into_location(self) -> Result<Location, Error> {
    Location::from_path(PathBuf::from(self))
  }
}

impl IntoLocation for &Path {
  fn into_location(self) -> Result<Location, Error> {
    Location::from_path(self.to_owned())
  }
}

impl IntoLocation for PathBuf {
  fn into_location(self) -> Result<Location, Error> {
    Location::from_path(self)
  }
}

pub(super) fn load_bundle(location: &Location, config: Config) -> Result<Bundle, Error> {
  let (province_image, definition_table, adjacencies_table) = match location {
    Location::Zip(path) => {
      let mut zip = ZipArchive::new(open_file(path)?)?;
      let province_image = read_rgb_bmp_image(zip.by_name("provinces.bmp")?)?;
      let definition_table = read_definition_table(zip.by_name("definition.csv")?)?;
      let adjacencies_table = zip.by_name("adjacencies.csv")
        .map_or_else(|_| Ok(Vec::new()), read_adjacencies_table)?;
      (province_image, definition_table, adjacencies_table)
    },
    Location::Dir(path) => {
      let province_image = read_rgb_bmp_image(open_file(path.join("provinces.bmp"))?)?;
      let definition_table = read_definition_table(open_file(path.join("definition.csv"))?)?;
      let adjacencies_table = maybe_not_found(File::open(path.join("adjacencies.csv")))?
        .map_or_else(|| Ok(Vec::new()), read_adjacencies_table)?;
      (province_image, definition_table, adjacencies_table)
    }
  };

  Ok(construct_map_data(province_image, definition_table, adjacencies_table, config))
}

fn construct_map_data(
  province_image: RgbImage,
  definition_table: Vec<Definition>,
  adjacencies_table: Vec<Adjacency>,
  config: Config
) -> Bundle {
  let mut color_buffer = province_image;

  let mut preserved_id_count = definition_table[0].id;
  // Create a sparse array for mapping province ids to colors
  let mut color_index = Vec::with_capacity(definition_table.len());
  for d in definition_table.iter() {
    let len = color_index.len().max(d.id as usize + 1);
    preserved_id_count = preserved_id_count.max(d.id);
    color_index.resize(len, None);
    color_index[d.id as usize] = Some(d.rgb);
  };

  assert_eq!(preserved_id_count, definition_table.len() as u32);

  // Initially convert the definition table into a province data map
  let mut definition_map = definition_table.into_iter()
    .map(|d| (d.rgb, ProvinceData::from_definition_config(d, &config)))
    .collect::<FxHashMap<Color, ProvinceData>>();
  // Loop through every pixel in the color buffer, ensuring that the resulting province data map
  // will be valid and will have no provinces mapping to colors not on the color buffer
  let mut province_data_map = FxHashMap::default();
  for (x, y, &Rgb(pixel)) in color_buffer.enumerate_pixels() {
    // If this color isn't in the new province data map, but it is in the definition table,
    // take it from the former and put it in the latter
    match province_data_map.entry(pixel) {
      Entry::Vacant(entry) => {
        let mut province_data = definition_map.remove(&pixel).unwrap_or_default();
        province_data.add_pixel([x, y]);
        entry.insert(Arc::new(province_data));
      },
      Entry::Occupied(entry) => {
        let entry = Arc::make_mut(entry.into_mut());
        entry.add_pixel([x, y]);
      }
    };
  };

  province_data_map.shrink_to_fit();
  let _ = definition_map;

  // Loop through the entries in the adjacencies table, converting ids to colors using `color_index`,
  // since the adjacencies map is indexed by color instead of id
  let mut connection_data_map = fx_hash_map_with_capacity(adjacencies_table.len());
  for a in adjacencies_table.into_iter() {
    if let Some(rel) = get_color_indexes(&color_index, [a.from_id, a.to_id]) {
      let connection_data = ConnectionData::from_adjacency(a, |through| {
        get_color_index(&color_index, through)
          .expect("adjacency present for an id that does not exist")
      });

      if let Some(connection_data) = connection_data {
        connection_data_map.insert(rel, Arc::new(connection_data));
      };
    };
  };

  connection_data_map.shrink_to_fit();
  let _ = color_index;

  // Recolor the entire map if `preserve_ids` is false
  if !config.preserve_ids {
    recolor_everything(
      &mut color_buffer,
      &mut province_data_map,
      &mut connection_data_map
    );
  };

  let id_data = config.preserve_ids.then(|| preserved_id_count );

  let mut map = Map {
    color_buffer,
    province_data_map,
    connection_data_map,
    boundaries: FxHashSet::default(),
    preserved_id_count: id_data
  };

  map.recalculate_all_boundaries();

  Bundle { map, config }
}

pub(super) fn recolor_everything(
  color_buffer: &mut RgbImage,
  province_data_map: &mut FxHashMap<Color, Arc<ProvinceData>>,
  connection_data_map: &mut FxHashMap<UOrd<Color>, Arc<ConnectionData>>
) {
  let mut colors_list = fx_hash_set_with_capacity(province_data_map.len());
  let mut replacement_map = fx_hash_map_with_capacity(province_data_map.len());

  let mut new_province_data_map = fx_hash_map_with_capacity(province_data_map.len());
  for (previous_color, province_data) in province_data_map.drain() {
    let color = random_color_pure(&colors_list, province_data.kind);
    let opt = colors_list.insert(color);
    debug_assert!(opt);
    let opt = replacement_map.insert(previous_color, color);
    debug_assert_eq!(opt, None);
    let opt = new_province_data_map.insert(color, province_data);
    debug_assert_eq!(opt, None);
  };

  *province_data_map = new_province_data_map;

  let mut new_connection_data_map = fx_hash_map_with_capacity(connection_data_map.len());
  for (previous_rel, mut connection_data) in connection_data_map.drain() {
    let rel = previous_rel.map(|color| replacement_map[&color]);
    // Replace `through`'s color with the new one
    let connection_data_mut = Arc::make_mut(&mut connection_data);
    connection_data_mut.through = connection_data_mut.through.map(|t| replacement_map[&t]);
    // This operation should never overwrite an existing entry
    let opt = new_connection_data_map.insert(rel, connection_data);
    debug_assert_eq!(opt, None);
  };

  *connection_data_map = new_connection_data_map;

  for Rgb(pixel) in color_buffer.pixels_mut() {
    *pixel = replacement_map[pixel];
  };
}

#[derive(Debug, Clone)]
enum IdChange {
  DeletedRange(u32, u32),
  CreatedRange(u32, u32),
  Reassigned(u32, u32),
  AssignedNew(u32)
}

impl ToString for IdChange {
  fn to_string(&self) -> String {
    match self {
      IdChange::DeletedRange(start, end) => format!("Deleted IDs {} through {}", start, end),
      IdChange::CreatedRange(start, end) => format!("Created IDs {} through {}", start, end),
      IdChange::Reassigned(from, to) => format!("Reassigned ID {} to {}", from, to),
      IdChange::AssignedNew(id) => format!("Assigned ID {} to new province", id)
    }
  }
}

type MapData = (Vec<Definition>, Vec<Adjacency>, Option<Vec<IdChange>>);

pub fn save_bundle(location: &Location, bundle: &Bundle) -> Result<(), Error> {
  match location {
    Location::Zip(path) => {
      let (definition_table, adjacencies_table, id_changes) = deconstruct_map_data(bundle)?;
      let mut zip = ZipWriter::new(create_file(path)?);
      zip.set_comment(format!("Generated by {}", crate::APPNAME));

      zip.start_file("provinces.bmp", Default::default())?;
      write_rgb_bmp_image(&mut zip, &bundle.map.color_buffer)?;

      zip.start_file("definition.csv", Default::default())?;
      write_definition_table(&mut zip, definition_table)?;

      if !adjacencies_table.is_empty() {
        zip.start_file("adjacencies.csv", Default::default())?;
        write_adjacencies_table(&mut zip, adjacencies_table)?;
      };

      if let Some(id_changes) = id_changes {
        zip.start_file("id_changes.txt", Default::default())?;
        write_id_changes(&mut zip, id_changes)?;
      };

      zip.finish()?;
    },
    Location::Dir(path) => {
      let (definition_table, adjacencies_table, id_changes) = deconstruct_map_data(bundle)?;

      let file = BufWriter::new(create_file(path.join("provinces.bmp"))?);
      write_rgb_bmp_image(file, &bundle.map.color_buffer)?;

      let file = create_file(path.join("definition.csv"))?;
      write_definition_table(file, definition_table)?;

      if !adjacencies_table.is_empty() {
        let file = create_file(path.join("adjacencies.csv"))?;
        write_adjacencies_table(file, adjacencies_table)?;
      };

      if let Some(id_changes) = id_changes {
        let file = create_file(path.join("id_changes.txt"))?;
        write_id_changes(file, id_changes)?;
      };
    }
  };

  Ok(())
}

fn deconstruct_map_data(bundle: &Bundle) -> Result<MapData, Error> {
  if bundle.config.preserve_ids {
    deconstruct_map_data_preserve_ids(bundle)
  } else {
    deconstruct_map_data_no_preserve_ids(bundle)
  }
}

fn deconstruct_map_data_preserve_ids(bundle: &Bundle) -> Result<MapData, Error> {
  let preserved_id_count = bundle.map.preserved_id_count
    .expect("config key `preserve-ids` was true, but map contained no id data");

  let count = bundle.map.provinces_count();
  let mut outlier_definitions = Vec::new();
  let mut sparse_definitions_table = vec![None; count];
  for (&color, province_data) in bundle.map.province_data_map.iter() {
    if let Some(preserved_id) = province_data.preserved_id {
      let definition = province_data.to_definition(color)?;
      let index = (preserved_id - 1) as usize;
      if index > count {
        outlier_definitions.push(definition);
      } else {
        sparse_definitions_table[index] = Some(definition);
      };
    } else {
      outlier_definitions.push(province_data.to_definition_with_id(color, 0)?);
    };
  };

  outlier_definitions.sort();

  let mut changes = Vec::new();
  // Loop through all of the 'outlier' definitions
  for mut outlier_definition in outlier_definitions.into_iter().rev() {
    // Loop through the sparse definitions table until you find an empty spot
    for (id, slot) in sparse_definitions_table.iter_mut().enumerate().rev() {
      let id = id as u32 + 1;
      if slot.is_none() {
        // Insert the current definition into the sparse definitions table
        if outlier_definition.id != id {
          if outlier_definition.id == 0 {
            changes.push(IdChange::AssignedNew(id));
          } else {
            changes.push(IdChange::Reassigned(outlier_definition.id, id));
          };
        };
        outlier_definition.id = id;
        *slot = Some(outlier_definition);
        break;
      };
    };
  };

  let current_id_count = sparse_definitions_table.len() as u32;
  match u32::cmp(&preserved_id_count, &current_id_count) {
    Ordering::Less => changes.push(IdChange::CreatedRange(preserved_id_count + 1, current_id_count)),
    Ordering::Greater => changes.push(IdChange::DeletedRange(current_id_count + 1, preserved_id_count)),
    Ordering::Equal => ()
  };

  let mut definitions_table = Vec::with_capacity(count);
  let mut color_index = fx_hash_map_with_capacity(definitions_table.len());
  for definition in sparse_definitions_table {
    let definition = definition.expect("infallible");
    color_index.insert(definition.rgb, definition.id);
    definitions_table.push(definition);
  };

  let mut adjacencies_table = Vec::with_capacity(bundle.map.connections_count());
  for (&rel, connection_data) in &bundle.map.connection_data_map {
    let rel = rel.map(|color| color_index[&color]);
    adjacencies_table.push(connection_data.to_adjacency(rel, |t| color_index[&t]));
  };

  adjacencies_table.sort();

  let id_changes = if changes.is_empty() { None } else { Some(changes) };
  Ok((definitions_table, adjacencies_table, id_changes))
}

fn deconstruct_map_data_no_preserve_ids(bundle: &Bundle) -> Result<MapData, Error> {
  let mut definitions_table = Vec::with_capacity(bundle.map.provinces_count());
  for (&color, province_data) in bundle.map.province_data_map.iter() {
    definitions_table.push(province_data.to_definition_with_id(color, 0)?);
  };

  definitions_table.sort();

  let mut id = 1;
  let mut color_index = fx_hash_map_with_capacity(definitions_table.len());
  for definition in definitions_table.iter_mut() {
    color_index.insert(definition.rgb, id);
    definition.id = id;
    id += 1;
  };

  let mut adjacencies_table = Vec::with_capacity(bundle.map.connections_count());
  for (&rel, connection_data) in &bundle.map.connection_data_map {
    let rel = rel.map(|color| color_index[&color]);
    adjacencies_table.push(connection_data.to_adjacency(rel, |t| color_index[&t]));
  };

  adjacencies_table.sort();

  Ok((definitions_table, adjacencies_table, None))
}

pub fn read_rgb_bmp_image<R: Read>(reader: R) -> Result<RgbImage, Error> {
  let decoder = BmpDecoder::new(read_all(reader)?)?;
  let img = DynamicImage::from_decoder(decoder)?;
  Ok(img.into_rgb8())
}

fn read_definition_table<R: Read>(reader: R) -> Result<Vec<Definition>, Error> {
  Definition::parse_all(read_all(reader)?).map_err(From::from)
}

fn read_adjacencies_table<R: Read>(reader: R) -> Result<Vec<Adjacency>, Error> {
  Adjacency::parse_all(read_all(reader)?).map_err(From::from)
}

pub fn write_rgb_bmp_image<W: Write>(mut writer: W, province_image: &RgbImage) -> Result<(), Error> {
  let mut encoder = BmpEncoder::new(&mut writer);
  let (width, height) = province_image.dimensions();
  encoder.encode(province_image.as_raw(), width, height, ColorType::Rgb8).map_err(From::from)
}

fn write_definition_table<W: Write>(mut writer: W, definition_table: Vec<Definition>) -> Result<(), Error> {
  let data = Definition::stringify_all(&definition_table);
  writer.write_all(data.as_bytes()).map_err(From::from)
}

fn write_adjacencies_table<W: Write>(mut writer: W, adjacencies_table: Vec<Adjacency>) -> Result<(), Error> {
  let data = Adjacency::stringify_all(&adjacencies_table);
  writer.write_all(data.as_bytes()).map_err(From::from)
}

fn write_id_changes<W: Write>(mut writer: W, id_changes: Vec<IdChange>) -> Result<(), Error> {
  writeln!(writer, "ID Changes {}", crate::util::now())?;
  for id_change in id_changes {
    writeln!(writer, "- {}", id_change.to_string())?;
  };

  Ok(())
}

fn read_all<R: Read>(mut reader: R) -> io::Result<Cursor<Vec<u8>>> {
  let mut buf = Vec::new();
  reader.read_to_end(&mut buf)?;
  Ok(Cursor::new(buf))
}

fn get_color_indexes(color_index: &[Option<Color>], [a, b]: [u32; 2]) -> Option<UOrd<Color>> {
  let a = get_color_index(color_index, a)?;
  let b = get_color_index(color_index, b)?;
  Some(UOrd::new(a, b))
}

fn get_color_index(color_index: &[Option<Color>], id: u32) -> Option<Color> {
  if let Some(&Some(color)) = color_index.get(id as usize) { Some(color) } else { None }
}

fn maybe_not_found<T>(err: io::Result<T>) -> io::Result<Option<T>> {
  use std::io::ErrorKind;
  match err {
    Ok(value) => Ok(Some(value)),
    Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
    Err(err) => Err(err)
  }
}
