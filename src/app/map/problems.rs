use fxhash::FxHashMap;
use graphics::context::Context;
use graphics::rectangle::Rectangle;
use graphics::ellipse::Ellipse;
use graphics::types::Color as DrawColor;
use opengl_graphics::GlGraphics;
use vecmath::Vector2;

use crate::app::colors;
use crate::app::canvas::Camera;
use super::{Bundle, Color, Map, Extents, boundary_to_line};
use crate::util::{fx_hash_map_with_capacity, stringify_color, XYIter};
use crate::util::uord::UOrd;

use std::collections::hash_map::Entry;
use std::fmt;



#[derive(Debug, Clone, PartialEq)]
pub enum Problem {
  InvalidXCrossing(Vector2<u32>),
  TooLargeBox(Extents),
  TooFewPixels(u64, Vector2<f64>),
  InvalidWidth,
  InvalidHeight,
  LonePixel(Vector2<u32>),
  FewSharedBorders(UOrd<Color>, Vec<UOrd<Vector2<u32>>>)
}

impl Problem {
  pub fn draw(&self, ctx: Context, extras: bool, camera: &Camera, gl: &mut GlGraphics) {
    match *self {
      Problem::InvalidXCrossing(pos) => {
        let pos = vec2_u32_to_f64(pos);
        draw_cross(pos, ctx, camera, colors::PROBLEM, gl);
      },
      Problem::TooLargeBox(extents) => {
        let lower = vec2_u32_to_f64(extents.lower);
        let upper = vec2_u32_to_f64(extents.upper);
        let upper = vecmath::vec2_add(upper, [1.0; 2]);
        draw_box([lower, upper], ctx, camera, colors::PROBLEM, gl);
      },
      Problem::TooFewPixels(_, pos) => {
        let pos = vecmath::vec2_add(pos, [0.5; 2]);
        draw_dot(pos, ctx, camera, colors::PROBLEM, gl);
      },
      Problem::LonePixel(pos) if extras => {
        let pos = [pos[0] as f64 + 0.5, pos[1] as f64 + 0.5];
        draw_dot(pos, ctx, camera, colors::WARNING, gl);
      },
      Problem::FewSharedBorders(_, ref borders) if extras => {
        if camera.scale_factor() > 1.0 {
          // When the zoom is < 100%, draw each border individually
          for &boundary in borders.iter() {
            let (b1, b2) = boundary_to_line(boundary)
              .map(vec2_u32_to_f64)
              .into_tuple_unordered();
            draw_line(b1, b2, ctx, camera, colors::WARNING, gl);
          };
        } else {
          // When the zoom is > 100%, just draw a dot here
          let count = borders.len() * 2;
          let pos = borders.iter()
            .flat_map(|&b| b.into_iter())
            .reduce(vecmath::vec2_add)
            .expect("infallible");
          let pos = [pos[0] as f64 / count as f64, pos[1] as f64 / count as f64];
          draw_dot(pos, ctx, camera, colors::WARNING, gl);
        };
      },
      _ => ()
    }
  }
}

impl fmt::Display for Problem {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match *self {
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
      },
      Problem::LonePixel(pos) => {
        write!(f, "Lone pixel at {:?}", pos)
      },
      Problem::FewSharedBorders(boundary, ref borders) => {
        let (a, b) = boundary.map(|which| stringify_color(which)).into_tuple();
        write!(f, "Only {} shared borders between provinces {} and {}", borders.len(), a, b)
      },
    }
  }
}

fn draw_cross(pos: Vector2<f64>, ctx: Context, camera: &Camera, color: DrawColor, gl: &mut GlGraphics) {
  let [x, y] = camera.compute_position(pos);
  graphics::line_from_to(color, 2.0, [x - 8.0, y - 8.0], [x + 8.0, y + 8.0], ctx.transform, gl);
  graphics::line_from_to(color, 2.0, [x - 8.0, y + 8.0], [x + 8.0, y - 8.0], ctx.transform, gl);
}

fn draw_dot(pos: Vector2<f64>, ctx: Context, camera: &Camera, color: DrawColor, gl: &mut GlGraphics) {
  let [x, y] = camera.compute_position(pos);
  Ellipse::new(color)
    .draw_from_to([x - 4.0, y - 4.0], [x + 4.0, y + 4.0], &Default::default(), ctx.transform, gl);
}

fn draw_box(bounds: [Vector2<f64>; 2], ctx: Context, camera: &Camera, color: DrawColor, gl: &mut GlGraphics) {
  let lower = camera.compute_position(bounds[0]);
  let upper = camera.compute_position(bounds[1]);
  Rectangle::new_border(color, 1.0)
    .draw_from_to(lower, upper, &Default::default(), ctx.transform, gl);
}

fn draw_line(p1: Vector2<f64>, p2: Vector2<f64>, ctx: Context, camera: &Camera, color: DrawColor, gl: &mut GlGraphics) {
  let p1 = camera.compute_position(p1);
  let p2 = camera.compute_position(p2);
  if camera.within_viewport(p1) || camera.within_viewport(p2) {
    graphics::line_from_to(color, 2.0, p1, p2, ctx.transform, gl);
  };
}

pub fn analyze(bundle: &Bundle) -> Vec<Problem> {
  let extras = bundle.config.extra_warnings.enabled;
  let [width, height] = bundle.map.dimensions();
  let mut problems = Vec::new();
  let mut province_extents = fx_hash_map_with_capacity::<Color, Extents>(bundle.map.provinces_count());
  let mut borders: FxHashMap<UOrd<Color>, Vec<UOrd<Vector2<u32>>>> = FxHashMap::default();

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

    if extras && bundle.config.extra_warnings.lone_pixels {
      let color = bundle.map.get_color_at(pos);
      let alone = bundle.map.iter_pixels_adjacent(pos)
        .all(|p| bundle.map.get_color_at(p) != color);
      if alone {
        problems.push(Problem::LonePixel(pos));
      };
    };

    if extras && bundle.config.extra_warnings.few_shared_borders {
      if pos[0] + 1 < width {
        let other = [pos[0] + 1, pos[1]];
        let a = bundle.map.get_color_at(pos);
        let b = bundle.map.get_color_at(other);
        let borders = borders.entry(UOrd::new(a, b))
          .or_insert_with(Vec::new);
        borders.push(UOrd::new(pos, other));
      };

      if pos[1] + 1 < height {
        let other = [pos[0], pos[1] + 1];
        let a = bundle.map.get_color_at(pos);
        let b = bundle.map.get_color_at(other);
        let borders = borders.entry(UOrd::new(a, b))
          .or_insert_with(Vec::new);
        borders.push(UOrd::new(pos, other));
      };
    };
  };

  if extras && bundle.config.extra_warnings.few_shared_borders {
    for (boundary, borders) in borders {
      if borders.len() <= bundle.config.extra_warnings.few_shared_borders_threshold {
        problems.push(Problem::FewSharedBorders(boundary, borders));
      };
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
