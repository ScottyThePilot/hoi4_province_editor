use fxhash::FxHashMap;
use graphics::color::RED;
use graphics::context::Context;
use graphics::rectangle::Rectangle;
use graphics::ellipse::Ellipse;
use opengl_graphics::GlGraphics;
use vecmath::{Matrix2x3, Vector2};

use super::{Bundle, Map, Extents};
use crate::util::XYIter;

use std::collections::hash_map::Entry;
use std::fmt;



#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Problem {
  InvalidXCrossing(Vector2<u32>),
  TooLargeBox(Extents),
  TooFewPixels(u64, Vector2<f64>),
  InvalidWidth,
  InvalidHeight,
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
      Problem::TooFewPixels(count, pos) => {
        write!(f, "Province has only {} pixels around {:?}", count, pos)
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
  graphics::line_from_to(RED, 2.0, [x - 8.0, y - 8.0], [x + 8.0, y + 8.0], transform, gl);
  graphics::line_from_to(RED, 2.0, [x - 8.0, y + 8.0], [x + 8.0, y - 8.0], transform, gl);
}

fn draw_dot(pos: Vector2<f64>, transform: Matrix2x3<f64>, display_matrix: Matrix2x3<f64>, gl: &mut GlGraphics) {
  let [x, y] = vecmath::row_mat2x3_transform_pos2(display_matrix, pos);
  Ellipse::new(RED).draw_from_to([x - 4.0, y - 4.0], [x + 4.0, y + 4.0], &Default::default(), transform, gl);
}

fn draw_box(bounds: [Vector2<f64>; 2], transform: Matrix2x3<f64>, display_matrix: Matrix2x3<f64>, gl: &mut GlGraphics) {
  let lower = vecmath::row_mat2x3_transform_pos2(display_matrix, bounds[0]);
  let upper = vecmath::row_mat2x3_transform_pos2(display_matrix, bounds[1]);
  Rectangle::new_border(RED, 1.0).draw_from_to(lower, upper, &Default::default(), transform, gl);
}



pub fn analyze(bundle: &Bundle) -> Vec<Problem> {
  struct ProblemData {
    extents: Extents,
    position_sum: Vector2<f64>
  }

  let [width, height] = bundle.map.dimensions();
  let mut problems = Vec::new();
  let mut problem_data_map = FxHashMap::default();

  for pos in XYIter::new(0..width, 0..height) {
    if pos[0] != width - 1 && pos[1] != height - 1 {
      if is_crossing_at(&bundle.map, pos) {
        let pos = [pos[0] + 1, pos[1] + 1];
        problems.push(Problem::InvalidXCrossing(pos));
      };
    };

    match problem_data_map.entry(bundle.map.get_color_at(pos)) {
      Entry::Vacant(entry) => {
        entry.insert(ProblemData {
          extents: Extents::new_point(pos),
          position_sum: [pos[0] as f64, pos[1] as f64]
        });
      },
      Entry::Occupied(entry) => {
        let entry = entry.into_mut();
        entry.extents = entry.extents.join_point(pos);
        entry.position_sum[0] += pos[0] as f64;
        entry.position_sum[1] += pos[1] as f64;
      }
    };
  };

  for (color, problem_data) in problem_data_map {
    let pixel_count = bundle.map.get_province(color).pixel_count;
    if pixel_count <= 8 {
      let pos = problem_data.position_sum;
      let pos = [pos[0] / pixel_count as f64, pos[1] / pixel_count as f64];
      problems.push(Problem::TooFewPixels(pixel_count, pos));
    };

    let extents = problem_data.extents;
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

fn is_crossing_at(map: &Map, [x, y]: Vector2<u32>) -> bool {
  let a = map.get_color_at([x, y]);
  let b = map.get_color_at([x + 1, y]);
  let c = map.get_color_at([x, y + 1]);
  let d = map.get_color_at([x + 1, y + 1]);
  a != b && c != d && b != d && a != c && a != d && b != c
}

fn vec2_u32_to_f64(pos: Vector2<u32>) -> Vector2<f64> {
  [pos[0] as f64, pos[1] as f64]
}
