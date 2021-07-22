use once_cell::sync::OnceCell;
use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;

use crate::app::map::Color;
use crate::util::hsl::hsl_to_rgb;

const SEQUENCE_LENGTH: usize = 4096;

pub fn sequence_color(index: usize) -> Option<Color> {
  static CACHE: OnceCell<Vec<Color>> = OnceCell::new();

  fn generate_sequence() -> Vec<Color> {
    let mut rng = SmallRng::seed_from_u64(0x938b902e4f56bf5b);
    let mut sequence = Vec::with_capacity(SEQUENCE_LENGTH);
    sequence.push([0; 3]);
    while sequence.len() < SEQUENCE_LENGTH {
      let h = rng.gen_range(0.0..360.0);
      let l = rng.gen_range(0.25..0.75);
      let color = hsl_to_rgb([h, 1.0, l]);
      sequence.push(color);
    };

    sequence
  }

  CACHE.get_or_init(generate_sequence)
    .get(index).cloned()
}
