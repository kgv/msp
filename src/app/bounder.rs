use crate::app::Bounds;
use egui::util::cache::{ComputerMut, FrameCache};
use std::{
    collections::BTreeMap,
    ops::{Bound, RangeBounds},
};

/// Bounded
pub(super) type Bounded = FrameCache<Value, Bounder>;

/// Bounder
#[derive(Default)]
pub(super) struct Bounder;

impl ComputerMut<(&Value, Bounds), Value> for Bounder {
    fn compute(&mut self, (peaks, bounds): (&Value, Bounds)) -> Value {
        peaks
            .iter()
            .filter_map(|(mass, intensity)| {
                (bounds.mass.contains(mass)
                    && (bounds.intensity, Bound::Unbounded).contains(intensity))
                .then_some((*mass, *intensity))
            })
            .collect()
    }
}

/// Value
type Value = BTreeMap<usize, u64>;
