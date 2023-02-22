use crate::app::Bounds;
use egui::util::cache::{ComputerMut, FrameCache};
use itertools::{Itertools, Permutations, Unique};
use std::{
    ops::{Bound, RangeBounds},
    slice::Iter,
};

/// Permutated
pub(super) type Permutated = FrameCache<Unique<Permutations<Iter<'static, usize>>>, Permutationer>;

/// Permutationer
#[derive(Default)]
pub(super) struct Permutationer;

// Unique<Permutations<Iter>>
impl ComputerMut<&[usize], Unique<Permutations<Iter<'static, usize>>>> for Permutationer {
    fn compute(&mut self, input: &[usize]) -> Unique<Permutations<Iter<'static, usize>>> {
        input.iter().copied().permutations(input.len()).unique()
    }
}
