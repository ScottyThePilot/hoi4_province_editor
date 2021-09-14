use graphics::context::Context;
use graphics::rectangle::Rectangle;
use graphics::ellipse::Ellipse;
use opengl_graphics::GlGraphics;
use vecmath::{Matrix2x3, Vector2};

use crate::app::colors;
use super::{Bundle, Color, Map, Extents};
use crate::util::{fx_hash_map_with_capacity, XYIter};

use std::collections::hash_map::Entry;
use std::fmt;



#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Problem {
  InvalidXCrossing(Vector2<u32>),
  TooLargeBox(Extents),
  TooFewPixels(u64, Vector2<f64>),
  InvalidWidth,
  InvalidHeight
}

impl Problem {
  pub fn draw(&self, ctx: Context, display_matrix: Matrix2x3<f64>, gl: &mut GlGraphics) {
    match *self {
      Problem::InvalidXCrossing(pos) => {
        let pos = vec2_u32_to_f64(pos);
        draw_cross(pos, ctx.transform, display_matrix, gl);
      },
      Problem::TooLargeBox(extents) => {
        let lower = vec2_u32_to_f64(extents.lower);
        let upper = vec2_u32_to_f64(extents.upper);
        let upper = vecmath::vec2_add(upper, [1.0; 2]);
        draw_box([lower, upper], ctx.transform, display_matrix, gl);
      },
      Problem::TooFewPixels(_, pos) => {
        let pos = vecmath::vec2_add(pos, [0.5; 2]);
        draw_dot(pos, ctx.transform, display_matrix, gl);
      },
      _ => ()
    }
  }
}

impl fmt::Display for Problem {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Problem::InvalidXCrossing(pos) => {
        write!(f, "Invalid X crossing at {:?}", pos)
      },
      Problem::TooLargeBox(extents) => {
        write!(f, "Province has too large box from {:?} to {:?}", extents.upper, extents.lower)
      },
      Problem::TooFewPixels(count, [x, y]) => {
        write!(f, "Province has only {} pixels around [{:.0}, {:.0}]", count, x, y)
      },
      Problem::InvalidWidth => {
        write!(f, "Map texture width is not a multiple of 64")
      },
      Problem::InvalidHeight => {
        write!(f, "Map texture height is not a multiple of 64")
      }
    }
  }
}

fn draw_cross(pos: Vector2<f64>, transform: Matrix2x3<f64>, display_matrix: Matrix2x3<f64>, gl: &mut GlGraphics) {
  let [x, y] = vecmath::row_mat2x3_transform_pos2(display_matrix, pos);
  graphics::line_from_to(colors::PROBLEM, 2.0, [x - 8.0, y - 8.0], [x + 8.0, y + 8.0], transform, gl);
  graphics::line_from_to(colors::PROBLEM, 2.0, [x - 8.0, y + 8.0], [x + 8.0, y - 8.0], transform, gl);
}

fn draw_dot(pos: Vector2<f64>, transform: Matrix2x3<f64>, display_matrix: Matrix2x3<f64>, gl: &mut GlGraphics) {
  let [x, y] = vecmath::row_mat2x3_transform_pos2(display_matrix, pos);
  Ellipse::new(colors::PROBLEM)
    .draw_from_to([x - 4.0, y - 4.0], [x + 4.0, y + 4.0], &Default::default(), transform, gl);
}

fn draw_box(bounds: [Vector2<f64>; 2], transform: Matrix2x3<f64>, display_matrix: Matrix2x3<f64>, gl: &mut GlGraphics) {
  let lower = vecmath::row_mat2x3_transform_pos2(display_matrix, bounds[0]);
  let upper = vecmath::row_mat2x3_transform_pos2(display_matrix, bounds[1]);
  Rectangle::new_border(colors::PROBLEM, 1.0)
    .draw_from_to(lower, upper, &Default::default(), transform, gl);
}



pub fn analyze(bundle: &Bundle) -> Vec<Problem> {
  let [width, height] = bundle.map.dimensions();
  let mut problems = Vec::new();
  let mut province_extents = fx_hash_map_with_capacity::<Color, Extents>(bundle.map.provinces_count());

  for pos in XYIter::new(0..width, 0..height) {
    if pos[1] != height - 1 && is_crossing_at(&bundle.map, pos) {
      let pos = [(pos[0] + 1) % width, pos[1] + 1];
      problems.push(Problem::InvalidXCrossing(pos));
    };

    match province_extents.entry(bundle.map.get_color_at(pos)) {
      Entry::Vacant(entry) => {
        entry.insert(Extents::new_point(pos));
      },
      Entry::Occupied(entry) => {
        let entry = entry.into_mut();
        *entry = entry.join_point(pos);
      }
    };
  };

  for (color, extents) in province_extents {
    let province_data = bundle.map.get_province(color);
    if province_data.pixel_count <= 8 {
      let center_of_mass = province_data.center_of_mass();
      problems.push(Problem::TooFewPixels(province_data.pixel_count, center_of_mass));
    };

    let (_, [province_width, province_height]) = extents.to_offset_size();
    if province_width > width / 8 || province_height > height / 8 {
      problems.push(Problem::TooLargeBox(extents));
    };
  };

  if width % 64 != 0 {
    problems.push(Problem::InvalidWidth);
  };

  if height % 64 != 0 {
    problems.push(Problem::InvalidHeight);
  };

  problems
}

fn is_crossing_at(map: &Map, [x0, y0]: Vector2<u32>) -> bool {
  #![allow(clippy::many_single_char_names)]
  let [x1, y1] = [if x0 + 1 == map.color_buffer.width() { 0 } else { x0 + 1 }, y0 + 1];
  let a = map.get_color_at([x0, y0]);
  let b = map.get_color_at([x1, y0]);
  let c = map.get_color_at([x0, y1]);
  let d = map.get_color_at([x1, y1]);
  a != b && c != d && b != d && a != c && a != d && b != c
}

fn vec2_u32_to_f64(pos: Vector2<u32>) -> Vector2<f64> {
  [pos[0] as f64, pos[1] as f64]
}
