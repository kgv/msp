use egui::{
    plot::{MarkerShape, Points},
    util::cache::{ComputerMut, FrameCache},
};

pub type Predicted = FrameCache<Option<Points>, Predictioner>;

/// Predictioner
#[derive(Default)]
pub struct Predictioner;

impl ComputerMut<(&[usize], &[u64]), Option<Points>> for Predictioner {
    fn compute(&mut self, (permutation, mut intensities): (&[usize], &[u64])) -> Option<Points> {
        let sum = intensities.iter().sum();
        let mut series = Vec::new();
        for &dm in permutation {
            let mass = intensities.len() - 1;
            let intensity = intensities[mass];
            series.push([mass as _, intensity as _]);
        }
        // intensities = intensities[..mass-]
        // let mut accumulator = 1.0;
        for &dm in permutation {
            mass = mass.checked_sub(dm)?;
            let intensity = filtered[mass];
            if intensity < 25.0 {
                return None;
            }
            accumulator *= intensity / sum;
            series.push([mass as _, intensity as _]);
        }
        Some(
            Points::new(series)
                .filled(true)
                .radius(9.0)
                .shape(MarkerShape::Circle)
                .name(accumulator),
        )
    }
}
