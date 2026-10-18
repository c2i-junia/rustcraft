//! Turns the wheel and the trackpad into whole steps, for whatever a scroll moves step by step.

use bevy::input::mouse::MouseScrollUnit;

/// Pixels of trackpad scrolling that make one step.
pub const SCROLL_PIXELS_PER_STEP: f32 = 20.0;

/// The whole steps a scroll of `y` makes, positive going up. A line of the wheel is one step;
/// trackpad pixels add up in `remainder`, which keeps the part of a step not made yet. It is
/// dropped when the scroll turns the other way, so that a trackpad reversing course answers at
/// once.
pub fn scroll_steps(remainder: &mut f32, unit: MouseScrollUnit, y: f32) -> i32 {
    let steps = match unit {
        MouseScrollUnit::Line => y,
        MouseScrollUnit::Pixel => y / SCROLL_PIXELS_PER_STEP,
    };
    if steps * *remainder < 0. {
        *remainder = 0.;
    }
    *remainder += steps;
    let whole = remainder.trunc();
    *remainder -= whole;
    whole as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_line_of_the_wheel_is_one_step() {
        let mut remainder = 0.;
        for (lines, steps) in [(1., 1), (1., 1), (-1., -1), (3., 3)] {
            assert_eq!(
                scroll_steps(&mut remainder, MouseScrollUnit::Line, lines),
                steps
            );
        }
    }

    #[test]
    fn trackpad_pixels_add_up_to_whole_steps() {
        let mut remainder = 0.;
        let below_one_step: Vec<i32> = (0..3)
            .map(|_| scroll_steps(&mut remainder, MouseScrollUnit::Pixel, 5.))
            .collect();
        assert_eq!(below_one_step, [0, 0, 0]);
        let mut remainder = 0.;
        let two_steps: i32 = (0..8)
            .map(|_| scroll_steps(&mut remainder, MouseScrollUnit::Pixel, 5.))
            .sum();
        assert_eq!(two_steps, 2);
        assert_eq!(remainder, 0.);
    }

    #[test]
    fn reversing_the_wheel_drops_the_remainder() {
        let mut remainder = 0.;
        assert_eq!(scroll_steps(&mut remainder, MouseScrollUnit::Pixel, 15.), 0);
        assert_eq!(
            scroll_steps(&mut remainder, MouseScrollUnit::Pixel, -10.),
            0
        );
        assert_eq!(remainder * SCROLL_PIXELS_PER_STEP, -10.);
        assert_eq!(
            scroll_steps(&mut remainder, MouseScrollUnit::Pixel, -10.),
            -1
        );
    }
}
