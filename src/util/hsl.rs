#![allow(clippy::many_single_char_names)]
// Code borrowed from https://github.com/DerivedMate/hsl-ish

pub fn hsl_to_rgb(hsl: [f32; 3]) -> [u8; 3] {
  fn hue2rgb(p: f32, q: f32, t: f32) -> f32 {
    let mut t = t;
    if t < 0.0 {
      t += 1.0;
    } else if t > 1.0 {
      t -= 1.0;
    };

    if t < 1.0 / 6.0 {
      p + (q - p) * 6.0 * t
    } else if t < 1.0 / 2.0 {
      q
    } else if t < 2.0 / 3.0 {
      p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
      p
    }
  }

  let (h, s, l) = (hsl[0] / 360.0, hsl[1], hsl[2]);
  let r;
  let g;
  let b;

  if s == 0.0 {
    r = l;
    g = l;
    b = l;
  } else {
    let q = if l < 0.5 {
      l * (1.0 + s)
    } else {
      l + s - l * s
    };

    let p = 2.0 * l - q;
    r = hue2rgb(p, q, h + 1.0 / 3.0);
    g = hue2rgb(p, q, h + 0.0 / 3.0);
    b = hue2rgb(p, q, h - 1.0 / 3.0);
  };

  [
    (r * 255.0).round() as u8,
    (g * 255.0).round() as u8,
    (b * 255.0).round() as u8
  ]
}
