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

  let [h, s, l] = hsl;
  let h = h / 360.0;

  let color = if s == 0.0 {
    [l; 3]
  } else {
    let q = if l < 0.5 {
      l * (1.0 + s)
    } else {
      l + s - l * s
    };

    let p = 2.0 * l - q;

    [
      hue2rgb(p, q, h + 1.0 / 3.0),
      hue2rgb(p, q, h + 0.0 / 3.0),
      hue2rgb(p, q, h - 1.0 / 3.0)
    ]
  };

  color.map(|k| {
    (k * 255.0).round() as u8
  })
}
