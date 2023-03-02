use egui::util::cache::{ComputerMut, FrameCache};
use indexmap::IndexMap;
use ndarray::{indices, Dim, Dimension, IxDynImpl};
use std::{collections::BTreeMap, iter::zip};

/// Key
#[derive(Clone, Copy, Debug, Hash)]
pub(super) struct Key<'a> {
    pub(super) mass: usize,
    pub(super) peaks: &'a BTreeMap<usize, u64>,
    pub(super) pattern: &'a [Vec<usize>],
    pub(super) zero_is_included: bool,
}

/// Predicted
pub(super) type Predicted = FrameCache<IndexMap<Dim<IxDynImpl>, f64>, Predictioner>;

/// Predictioner
#[derive(Default)]
pub(super) struct Predictioner;

impl ComputerMut<Key<'_>, IndexMap<Dim<IxDynImpl>, f64>> for Predictioner {
    fn compute(&mut self, args: Key) -> IndexMap<Dim<IxDynImpl>, f64> {
        let shape = args.pattern.iter().map(Vec::len).collect::<Vec<_>>();
        let mut predictions = indices(shape)
            .into_iter()
            .filter_map(|index| {
                let mut mass = args.mass;
                let mut intensity = 0.0;
                for delta in zip(args.pattern, index.slice()).map(|(step, &index)| step[index]) {
                    mass = mass.checked_sub(delta)?;
                    intensity +=
                        args.peaks
                            .get(&mass)
                            .copied()
                            .or(args.zero_is_included.then_some(0))? as f64;
                }
                Some((index, intensity))
            })
            .collect::<IndexMap<_, _>>();
        predictions.sort_by(|_, left, _, right| right.total_cmp(left));
        predictions
    }
}
