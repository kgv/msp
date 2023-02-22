use std::ops::Bound;

/// Extension methods for [`Bound`]
pub trait BoundExt<T> {
    // fn max(self, other: Self) -> Self
    // where
    //     T: Ord;

    // fn min(self, other: Self) -> Self
    // where
    //     T: Ord;

    fn value(&self) -> Option<&T>;

    fn variant_name(&self) -> &'static str;
}

impl<T> BoundExt<T> for Bound<T> {
    // fn max(mut self, other: Self) -> Self
    // where
    //     T: Ord,
    // {
    //     if let Self::Included(left) | Self::Excluded(left) = &self {
    //         if let Self::Included(right) | Self::Excluded(right) = other {
    //             if left <= &right {
    //                 self = Self::Excluded(right);
    //             }
    //         }
    //     }
    //     self
    // }

    // fn min(mut self, other: Self) -> Self
    // where
    //     T: Ord,
    // {
    //     if let Self::Included(left) | Self::Excluded(left) = &self {
    //         if let Self::Included(right) | Self::Excluded(right) = other {
    //             if left >= &right {
    //                 self = Self::Excluded(right);
    //             }
    //         }
    //     }
    //     self
    // }

    fn value(&self) -> Option<&T> {
        match self {
            Self::Included(value) => Some(value),
            Self::Excluded(value) => Some(value),
            Self::Unbounded => None,
        }
    }

    fn variant_name(&self) -> &'static str {
        match self {
            Self::Included(_) => "Included",
            Self::Excluded(_) => "Excluded",
            Self::Unbounded => "Unbounded",
        }
    }
}
