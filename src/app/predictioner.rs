use egui::util::cache::{ComputerMut, FrameCache};

/// Predicted
pub(super) type Predicted = FrameCache<f64, Predictioner>;

/// Predictioner
#[derive(Default)]
pub(super) struct Predictioner;

impl ComputerMut<(&[usize], &[u64]), f64> for Predictioner {
    fn compute(&mut self, (permutation, intensities): (&[usize], &[u64])) -> f64 {
        let mut mass = intensities.len() - 1;
        let mut intensity = intensities[mass] as _;
        for &delta in permutation {
            mass = mass.saturating_sub(delta);
            intensity *= intensities[mass] as f64;
            if intensity < f64::EPSILON {
                break;
            }
        }
        intensity
    }
}
