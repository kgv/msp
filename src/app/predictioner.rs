use std::iter::zip;

use egui::util::cache::{ComputerMut, FrameCache};
use indexmap::IndexMap;
use ndarray::{indices, ArrayD, Dim, Dimension, IntoDimension, IxDynImpl, ShapeBuilder};
use tracing::error;

/// Predicted
pub(super) type Predicted = FrameCache<IndexMap<Dim<IxDynImpl>, f64>, Predictioner>;

#[derive(Clone, Copy, Debug, Default, Hash)]
pub(super) struct In<'a> {
    pub(super) mass: usize,
    pub(super) intensities: &'a [u64],
    pub(super) pattern: &'a [Vec<usize>],
    pub(super) zero_is_allowed: bool,
}

type Out = IndexMap<Dim<IxDynImpl>, f64>;

/// Predictioner
#[derive(Default)]
pub(super) struct Predictioner;

impl ComputerMut<In<'_>, IndexMap<Dim<IxDynImpl>, f64>> for Predictioner {
    fn compute(
        &mut self,
        In {
            mass,
            intensities,
            pattern,
            zero_is_allowed: zero_is_valid,
        }: In,
    ) -> IndexMap<Dim<IxDynImpl>, f64> {
        let shape = pattern.iter().map(Vec::len).collect::<Vec<_>>();
        let mut predictions = indices(shape)
            .into_iter()
            .filter_map(|index| {
                let mut mass = mass;
                let mut intensity = 0.0;
                for delta in zip(pattern, index.slice()).map(|(step, &index)| step[index]) {
                    mass = mass.checked_sub(delta)?;
                    intensity += Some(intensities[mass])
                        .filter(|&intensity| intensity != 0 || zero_is_valid)?
                        as f64;
                }
                Some((index, intensity))
            })
            .collect::<IndexMap<_, _>>();
        predictions.sort_by(|_, left, _, right| right.total_cmp(left));
        predictions
    }
}

// let predictions = ArrayD::from_shape_fn(shape, |dimension| {
//     let mut mass = mass;
//     let mut intensity = 0.0;
//     for delta in zip(pattern, dimension.slice()).map(|(step, &index)| step[index]) {
//         mass = match mass.checked_sub(delta) {
//             None => return 0.0,
//             Some(mass) => mass,
//         };
//         intensity += match intensities[mass] {
//             0 if !zero_is_valid => return 0.0,
//             intensity => intensity as f64,
//         };
//     }
//     intensity
// });
// predictions
//     .indexed_iter()
//     .sorted_by(|left, right| right.1.total_cmp(left.1))
//     .map(|(dim, &value)| (dim, value))
//     .take(3)
//     .collect()
