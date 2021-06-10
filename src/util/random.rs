use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;
use rand::distributions::{Distribution, Standard};
use rand::distributions::uniform::{SampleRange, SampleUniform};

use crate::app::map::Color;
use crate::util::hsl::hsl_to_rgb;

use std::cell::RefCell;
use std::rc::Rc;

/// An RNG utility struct that can be multiple-owned
#[derive(Debug)]
pub struct RandomHandle {
  ptr: Rc<RefCell<RandomHandleInner>>
}

impl RandomHandle {
  pub fn new() -> Self {
    let rng = SmallRng::from_entropy();
    let sequence_rng = SmallRng::seed_from_u64(0x938b902e4f56bf5b);
    let inner = RandomHandleInner { rng, sequence_rng, sequence: vec![[0; 3]] };
    RandomHandle { ptr: Rc::new(RefCell::new(inner)) }
  }

  pub fn reseed_entropy(&self) {
    self.ptr.borrow_mut().rng = SmallRng::from_entropy();
  }

  pub fn reseed(&self, seed: u64) {
    self.ptr.borrow_mut().rng = SmallRng::seed_from_u64(seed);
  }

  pub fn gen<T>(&self) -> T
  where Standard: Distribution<T> {
    self.ptr.borrow_mut().rng.gen::<T>()
  }

  pub fn gen_range<T, R>(&self, range: R) -> T
  where T: SampleUniform, R: SampleRange<T> {
    self.ptr.borrow_mut().rng.gen_range::<T, R>(range)
  }

  pub fn sequence_color(&self, index: usize) -> Color {
    self.ptr.borrow_mut().sequence_color(index)
  }
}

impl Clone for RandomHandle {
  fn clone(&self) -> Self {
    RandomHandle { ptr: Rc::clone(&self.ptr) }
  }
}

#[derive(Debug)]
struct RandomHandleInner {
  rng: SmallRng,
  sequence_rng: SmallRng,
  sequence: Vec<Color>
}

impl RandomHandleInner {
  fn sequence_color(&mut self, index: usize) -> Color {
    while self.sequence.len() < index + 1 {
      let h = self.sequence_rng.gen_range(0.0..360.0);
      let l = self.sequence_rng.gen_range(0.25..0.75);
      let color = hsl_to_rgb([h, 1.0, l]);
      self.sequence.push(color);
    };

    self.sequence[index]
  }
}
