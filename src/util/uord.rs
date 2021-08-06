use std::cmp::Ordering;
use std::borrow::Borrow;
use std::hash::{Hash, Hasher};
use std::fmt;

pub struct UOrd<T> {
  a: T,
  b: T
}

impl<T> UOrd<T> {
  #[inline(always)]
  pub const fn new(a: T, b: T) -> UOrd<T> {
    UOrd { a, b }
  }

  #[inline]
  pub fn contains<Q>(&self, x: &Q) -> bool
  where T: Borrow<Q>, Q: Eq {
    self.a.borrow() == x || self.b.borrow() == x
  }

  pub fn replace(self, from: T, to: T) -> Self
  where T: PartialEq + Copy {
    let a = if self.a == from { to } else { self.a };
    let b = if self.b == from { to } else { self.b };
    UOrd { a, b }
  }

  pub fn as_tuple(&self) -> (&T, &T)
  where T: Ord {
    let UOrd { a, b } = self;
    match T::cmp(a, b) {
      Ordering::Less | Ordering::Equal => (a, b),
      Ordering::Greater => (b, a)
    }
  }

  pub fn into_tuple(self) -> (T, T)
  where T: Ord {
    let UOrd { a, b } = self;
    match T::cmp(&a, &b) {
      Ordering::Less | Ordering::Equal => (a, b),
      Ordering::Greater => (b, a)
    }
  }

  pub fn map<F, U>(self, mut f: F) -> UOrd<U>
  where F: FnMut(T) -> U {
    UOrd::new(f(self.a), f(self.b))
  }
}

impl<T: fmt::Debug + Ord> fmt::Debug for UOrd<T> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    fmt::Debug::fmt(&self.as_tuple(), f)
  }
}

impl<T: Copy> Copy for UOrd<T> {}

impl<T: Clone> Clone for UOrd<T> {
  #[inline(always)]
  fn clone(&self) -> UOrd<T> {
    UOrd {
      a: self.a.clone(),
      b: self.b.clone()
    }
  }
}

impl<T> From<(T, T)> for UOrd<T> {
  #[inline(always)]
  fn from(value: (T, T)) -> UOrd<T> {
    UOrd { a: value.0, b: value.1 }
  }
}

impl<T: Ord> Into<(T, T)> for UOrd<T> {
  #[inline(always)]
  fn into(self) -> (T, T) {
    self.into_tuple()
  }
}

impl<T: PartialEq> PartialEq for UOrd<T> {
  #[inline]
  fn eq(&self, other: &UOrd<T>) -> bool {
    (self.a == other.a && self.b == other.b) ||
    (self.a == other.b && self.b == other.a)
  }
}

impl<T: Eq> Eq for UOrd<T> {}

impl<T: Ord + Hash> Hash for UOrd<T> {
  #[inline]
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.as_tuple().hash(state);
  }
}
