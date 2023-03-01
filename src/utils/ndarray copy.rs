use indexmap::IndexSet;
use ndarray::{
    azip, Array, Array1, ArrayView1, Axis, Dimension, IntoNdProducer, Ix1, NdProducer, RemoveAxis,
    Zip,
};
// use rawpointer::PointerExt;
use std::{cmp::Ordering, ptr::copy_nonoverlapping};

/// Permute array
trait PermuteArray {
    type Item;
    type Dim;

    fn permute_axis(self, axis: Axis, permutation: &Permutation) -> Array<Self::Item, Self::Dim>;
}

/// Sort array
pub trait SortArray: PermuteArray {
    type Item;
    type Dim;

    fn sort_axis_by<F>(self, axis: Axis, cmp: F) -> Array<Self::Item, Self::Dim>
    where
        // Self::Item: Clone,
        Self::Dim: RemoveAxis,
        F: FnMut(&usize, &usize) -> Ordering;

    fn permute_axis(self, axis: Axis, permutation: &Permutation) -> Array<Self::Item, Self::Dim>
    where
        // Self::Item: Clone,
        Self::Dim: RemoveAxis;
}

impl<A, D> SortArray for Array<A, D>
where
    D: Dimension,
{
    type Item = A;
    type Dim = D;

    fn permute_axis(self, axis: Axis, permutation: &Permutation) -> Array<A, D>
    where
        D: RemoveAxis,
    {
        let axis_len = self.len_of(axis);
        let axis_stride = self.stride_of(axis);
        assert_eq!(axis_len, permutation.indices.len());

        if self.is_empty() {
            return self;
        }

        let mut result = Array::uninit(self.dim());
        unsafe {
            // logically move ownership of all elements from self into result
            // the result realizes this ownership at .assume_init() further down
            let mut moved_elements = 0;
            azip!((&r#index in &permutation.indices, result in result.axis_iter_mut(axis)) {
                // Use a shortcut to avoid bounds checking in `index_axis` for the source.
                //
                // It works because for any given element pointer in the array we have the
                // relationship:
                //
                // .index_axis(axis, 0) + .stride_of(axis) * j == .index_axis(axis, j)
                //
                // where + is pointer arithmetic on the element pointers.
                //
                // Here source_0 and the offset is equivalent to self.index_axis(axis, perm_i)
                let source = self.raw_view().index_axis_move(axis, 0);
                azip!((from in source, to in result) {
                    let from = from.stride_offset(axis_stride, r#index);
                    copy_nonoverlapping(from, to.as_mut_ptr(), 1);
                    moved_elements += 1;
                });
            });
            debug_assert_eq!(result.len(), moved_elements);
            // forget the old elements but not the allocation
            let mut old_storage = self.into_raw_vec();
            old_storage.set_len(0);
            // transfer ownership of the elements into the result
            result.assume_init()
        }
    }

    fn sort_axis_by<F>(self, axis: Axis, cmp: F) -> Array<Self::Item, Self::Dim>
    where
        // Self::Item: Clone,
        // Self::Dim: RemoveAxis,
        D: RemoveAxis,
        F: FnMut(&usize, &usize) -> Ordering,
    {
        let mut permutation = Permutation::from_iter(0..self.len_of(axis));
        permutation.indices.sort_by(cmp);
        self.permute_axis(axis, &permutation)
    }
}

// Permutation
#[derive(Clone, Debug)]
pub struct Permutation {
    indices: Vec<usize>,
}

impl Permutation {
    pub fn new(indices: IndexSet<usize>) -> Self {
        Self {
            indices: indices.into_iter().collect(),
        }
    }

    pub fn sort_by<F: FnMut(&usize, &usize) -> Ordering>(&mut self, cmp: F) {
        self.indices.sort_by(cmp);
    }
}

impl FromIterator<usize> for Permutation {
    fn from_iter<T: IntoIterator<Item = usize>>(iter: T) -> Self {
        Self {
            indices: FromIterator::from_iter(iter),
        }
    }
}
