use std::ops::Bound;

/// Extension methods for [`Bound`]
pub(crate) trait BoundExt<T> {
    fn value(&self) -> Option<&T>;

    fn variant_name(&self) -> &'static str;
}

impl<T> BoundExt<T> for Bound<T> {
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
