use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;
use rand::distributions::{Distribution, Standard};
use rand::distributions::uniform::{SampleRange, SampleUniform};

use crate::app::map::Color;
use crate::util::hsl::hsl_to_rgb;

use std::sync::{Arc, Mutex};

const SEQUENCE_LENGTH: usize = 4096;

/// An RNG utility struct that can be multiple-owned
#[derive(Debug)]
pub struct RandomHandle {
  random: Mutex<RandomHandleInner>,
  sequence: Arc<Vec<Color>>
}

impl RandomHandle {
  pub fn new() -> Self {
    RandomHandle {
      random: Mutex::new(RandomHandleInner::new()),
      sequence: Arc::new(generate_sequence())
    }
  }

  pub fn reseed_entropy(&self) {
    let mut lock = self.random.lock().unwrap();
    lock.rng = SmallRng::from_entropy();
  }

  pub fn reseed(&self, seed: u64) {
    let mut lock = self.random.lock().unwrap();
    lock.rng = SmallRng::seed_from_u64(seed);
  }

  pub fn gen<T>(&self) -> T
  where Standard: Distribution<T> {
    let mut lock = self.random.lock().unwrap();
    lock.rng.gen::<T>()
  }

  pub fn gen_range<T, R>(&self, range: R) -> T
  where T: SampleUniform, R: SampleRange<T> {
    let mut lock = self.random.lock().unwrap();
    lock.rng.gen_range::<T, R>(range)
  }

  pub fn sequence_color(&self, index: usize) -> Option<Color> {
    self.sequence.get(index).cloned()
  }

  pub fn derive(&self) -> Self {
    RandomHandle {
      random: Mutex::new(RandomHandleInner::new()),
      sequence: Arc::clone(&self.sequence)
    }
  }
}

#[derive(Debug)]
struct RandomHandleInner {
  rng: SmallRng
}

impl RandomHandleInner {
  fn new() -> Self {
    Self {
      rng: SmallRng::from_entropy()
    }
  }
}

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
