use crate::app::Bounds;
use egui::util::cache::{ComputerMut, FrameCache};
use std::ops::{Bound, RangeBounds};

/// Bounded
pub(super) type Bounded = FrameCache<Vec<u64>, Bounder>;

/// Bounder
#[derive(Default)]
pub(super) struct Bounder;

impl ComputerMut<(&[u64], Bounds), Vec<u64>> for Bounder {
    fn compute(&mut self, (intensities, bounds): (&[u64], Bounds)) -> Vec<u64> {
        intensities
            .iter()
            .enumerate()
            .map(|(mass, &intensity)| {
                if bounds.mass.contains(&mass)
                    && (bounds.intensity, Bound::Unbounded).contains(&intensity)
                {
                    intensity
                } else {
                    0
                }
            })
            .collect()
    }
}
