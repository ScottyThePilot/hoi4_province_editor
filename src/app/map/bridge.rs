//! Anything relating to loading or saving map data
use ahash::{AHashMap, AHashSet};
use defy::Contextualize;
use image::{Rgb, Rgba, RgbImage, RgbaImage, Pixel, DynamicImage, ColorType};
use image::codecs::bmp::{BmpDecoder, BmpEncoder};
use uord::UOrd2 as UOrd;

use super::{Color, Bundle, MapBase, Map, ProvinceData, ConnectionData, random_color_pure};
use crate::app::format::{Adjacency, Definition, ParseCsv};
use crate::config::Config;
use crate::error::Error;
use crate::util::files::Location;

use std::collections::hash_map::Entry;
use std::cmp::Ordering;
use std::io::{self, Cursor, Read, Write};
use std::sync::Arc;

pub(super) fn load_bundle(location: &Location, config: Config) -> Result<Bundle, Error> {
  let (province_image, definition_table, adjacencies_table, rivers) = location.clone().manipulate_files(|files| {
    let province_image = read_rgb_bmp_image(files.open_file("provinces.bmp")?)?;
    let definition_table = read_definition_table(files.open_file("definition.csv")?)?;
    let adjacencies_table = files.open_file_maybe_not_found("adjacencies.csv")?
      .map_or_else(|| Ok(Vec::new()), read_adjacencies_table)?;
    let rivers = files.open_file_maybe_not_found("rivers.bmp")?
      .map(read_rgb_bmp_image).transpose()?;
    Ok((province_image, definition_table, adjacencies_table, rivers))
  })?;

  Ok(construct_map_data(province_image, definition_table, adjacencies_table, rivers, config))
}

fn construct_map_data(
  province_image: RgbImage,
  definition_table: Vec<Definition>,
  adjacencies_table: Vec<Adjacency>,
  rivers: Option<RgbImage>,
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

  // TODO: rework? this will probably crash for certain invalid definition tables
  assert_eq!(preserved_id_count, definition_table.len() as u32);

  // Initially convert the definition table into a province data map
  let mut definition_map = definition_table.into_iter()
    .map(|d| (d.rgb, ProvinceData::from_definition_config(d, &config)))
    .collect::<AHashMap<Color, ProvinceData>>();
  // Loop through every pixel in the color buffer, ensuring that the resulting province data map
  // will be valid and will have no provinces mapping to colors not on the color buffer
  let mut province_data_map = AHashMap::default();
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

  // strip colors from the color index that failed to have province data created for them
  for color_index_entry in color_index.iter_mut() {
    *color_index_entry = color_index_entry.filter(|color| province_data_map.contains_key(color));
  };

  let get_color_index = |id: u32| get_color_index(&color_index, id);

  province_data_map.shrink_to_fit();
  let _ = definition_map;

  // Loop through the entries in the adjacencies table, converting ids to colors using `color_index`,
  // since the adjacencies map is indexed by color instead of id
  let mut preserved_unsupported_adjacencies = Vec::new();
  let mut connection_data_map = AHashMap::with_capacity(adjacencies_table.len());
  for a in adjacencies_table.into_iter() {
    if let Some(rel) = UOrd::new([a.from_id, a.to_id]).try_map_opt(get_color_index) {
      if let Some(connection_data) = ConnectionData::from_adjacency(a.clone(), get_color_index) {
        connection_data_map.insert(rel, Arc::new(connection_data));
      } else {
        preserved_unsupported_adjacencies.push(a);
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

  let id_data = config.preserve_ids.then(|| preserved_id_count);

  let rivers_overlay = rivers.as_ref().map(process_and_clear_rivers_image);

  let mut map = Map {
    base: MapBase {
      color_buffer: Arc::new(color_buffer),
      province_data_map: Arc::new(province_data_map),
      connection_data_map: Arc::new(connection_data_map),
      rivers_overlay: rivers_overlay.map(Arc::new)
    },
    boundaries: AHashMap::default(),
    preserved_unsupported_adjacencies,
    preserved_id_count: id_data
  };

  map.recalculate_all_boundaries();

  Bundle { map, config }
}

pub(super) fn recolor_everything(
  color_buffer: &mut RgbImage,
  province_data_map: &mut AHashMap<Color, Arc<ProvinceData>>,
  connection_data_map: &mut AHashMap<UOrd<Color>, Arc<ConnectionData>>
) {
  let mut colors_list = AHashSet::with_capacity(province_data_map.len());
  let mut replacement_map = AHashMap::with_capacity(province_data_map.len());

  let mut new_province_data_map = AHashMap::with_capacity(province_data_map.len());
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

  let mut new_connection_data_map = AHashMap::with_capacity(connection_data_map.len());
  for (previous_rel, mut connection_data) in connection_data_map.drain() {
    // Replace `through`'s color with the new one
    let connection_data_mut = Arc::make_mut(&mut connection_data);
    connection_data_mut.through = connection_data_mut.through
      .and_then(|t| replacement_map.get(&t).copied());
    if let Some(rel) = previous_rel.try_map_opt(|color| replacement_map.get(&color).copied()) {
      new_connection_data_map.insert(rel, connection_data);
    };
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
  let (definition_table, adjacencies_table, id_changes) = deconstruct_map_data(bundle)?;
  location.clone().manipulate_files(|files| {
    write_rgb_bmp_image(files.create_file("provinces.bmp")?, &bundle.map.base.color_buffer)?;
    write_definition_table(files.create_file("definition.csv")?, definition_table)?;

    if !adjacencies_table.is_empty() {
      write_adjacencies_table(files.create_file("adjacencies.csv")?, adjacencies_table)?;
    };

    if let Some(id_changes) = id_changes {
      write_id_changes(files.create_file("id_changes.txt")?, id_changes)?;
    };

    Ok(())
  })?;

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
  for (&color, province_data) in bundle.map.base.province_data_map.iter() {
    if let Some(preserved_id) = province_data.preserved_id {
      let definition = province_data.to_definition(color)?;
      let index = (preserved_id - 1) as usize;
      if index < sparse_definitions_table.len() {
        sparse_definitions_table[index] = Some(definition);
      } else {
        outlier_definitions.push(definition);
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
  let mut color_index = AHashMap::with_capacity(definitions_table.len());
  for definition in sparse_definitions_table {
    let definition = definition.expect("infallible");
    color_index.insert(definition.rgb, definition.id);
    definitions_table.push(definition);
  };

  let mut adjacencies_table = Vec::with_capacity(bundle.map.connections_count());
  for (&rel, connection_data) in bundle.map.base.connection_data_map.iter() {
    let rel = rel.map(|color| color_index[&color]);
    adjacencies_table.push(connection_data.to_adjacency(rel, |t| color_index[&t]));
  };

  adjacencies_table.extend_from_slice(&bundle.map.preserved_unsupported_adjacencies);
  adjacencies_table.sort();

  let id_changes = if changes.is_empty() { None } else { Some(changes) };
  Ok((definitions_table, adjacencies_table, id_changes))
}

fn deconstruct_map_data_no_preserve_ids(bundle: &Bundle) -> Result<MapData, Error> {
  let mut definitions_table = Vec::with_capacity(bundle.map.provinces_count());
  for (&color, province_data) in bundle.map.base.province_data_map.iter() {
    definitions_table.push(province_data.to_definition_with_id(color, 0)?);
  };

  definitions_table.sort();

  let mut id = 1;
  let mut color_index = AHashMap::with_capacity(definitions_table.len());
  for definition in definitions_table.iter_mut() {
    color_index.insert(definition.rgb, id);
    definition.id = id;
    id += 1;
  };

  let mut adjacencies_table = Vec::with_capacity(bundle.map.connections_count());
  for (&rel, connection_data) in bundle.map.base.connection_data_map.iter() {
    let rel = rel.map(|color| color_index[&color]);
    adjacencies_table.push(connection_data.to_adjacency(rel, |t| color_index[&t]));
  };

  adjacencies_table.sort();

  Ok((definitions_table, adjacencies_table, None))
}

fn process_and_clear_rivers_image(img: &RgbImage) -> RgbaImage {
  //const RIVERS_PIXEL_PALETTE_CLEAR: &[Rgb<u8>] = &[
  //  // land
  //  Rgb([255, 255, 255]),
  //  // water
  //  Rgb([122, 122, 122])
  //];

  const RIVERS_PIXEL_PALETTE_KEEP: &[Rgb<u8>] = &[
    // river source
    Rgb([0, 255, 0]),
    // flow-in source
    Rgb([255, 0, 0]),
    // flow-out source
    Rgb([255, 252, 0]),
    // rivers
    Rgb([0, 225, 255]),
    Rgb([0, 200, 255]),
    Rgb([0, 150, 255]),
    Rgb([0, 100, 255]),
    Rgb([0, 0, 255]),
    Rgb([0, 0, 225]),
    Rgb([0, 0, 200]),
    Rgb([0, 0, 150]),
    Rgb([0, 0, 100])
  ];

  RgbaImage::from_par_fn(img.width(), img.height(), |x, y| {
    let pixel = img.get_pixel(x, y);
    if RIVERS_PIXEL_PALETTE_KEEP.contains(pixel) {
      pixel.to_rgba()
    } else {
      Rgba([0x00; 4])
    }
  })
}

pub fn read_rgb_bmp_image<R: Read>(reader: R) -> Result<RgbImage, Error> {
  let decoder = BmpDecoder::new(read_all(reader).context("failed to read bmp image")?)?;
  let img = DynamicImage::from_decoder(decoder)?;
  Ok(img.into_rgb8())
}

fn read_definition_table<R: Read>(reader: R) -> Result<Vec<Definition>, Error> {
  Definition::read_records(reader).map_err(|err| Error::Csv(err, "definition.csv"))
}

fn read_adjacencies_table<R: Read>(reader: R) -> Result<Vec<Adjacency>, Error> {
  Adjacency::read_records(reader).map_err(|err| Error::Csv(err, "adjacencies.csv"))
}

pub fn write_rgb_bmp_image<W: Write>(mut writer: W, province_image: &RgbImage) -> Result<(), Error> {
  let mut encoder = BmpEncoder::new(&mut writer);
  let (width, height) = province_image.dimensions();
  encoder.encode(province_image.as_raw(), width, height, ColorType::Rgb8).map_err(From::from)
}

fn write_definition_table<W: Write>(writer: W, definition_table: Vec<Definition>) -> Result<(), Error> {
  Definition::write_records(&definition_table, writer).map_err(|err| Error::Csv(err, "definition.csv"))
}

fn write_adjacencies_table<W: Write>(writer: W, adjacencies_table: Vec<Adjacency>) -> Result<(), Error> {
  Adjacency::write_records(&adjacencies_table, writer).map_err(|err| Error::Csv(err, "adjacencies.csv"))
}

fn write_id_changes<W: Write>(mut writer: W, id_changes: Vec<IdChange>) -> Result<(), Error> {
  writeln!(writer, "ID Changes {}", crate::util::now())
    .context("failed to write id changes to file")?;
  for id_change in id_changes {
    writeln!(writer, "- {}", id_change.to_string())
      .context("failed to write id changes to file")?;
  };

  Ok(())
}

fn read_all<R: Read>(mut reader: R) -> io::Result<Cursor<Vec<u8>>> {
  let mut buf = Vec::new();
  reader.read_to_end(&mut buf)?;
  Ok(Cursor::new(buf))
}

fn get_color_index(color_index: &[Option<Color>], id: u32) -> Option<Color> {
  color_index.get(id as usize).and_then(Clone::clone)
}
