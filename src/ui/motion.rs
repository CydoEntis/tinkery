//! Short movements (tinkery-forge spec F23): the selection pill glides to its
//! new row, a toast slides in from the right and fades at the end of its hold.
//! Whole cells, eased out, over well under half a second.
//!
//! Nothing here reads a clock. `App` owns when a movement started and the time
//! of the frame being drawn, and asks these functions where things are then;
//! with `[ui] motion` off it doesn't ask, and things are where they end.

use std::time::Duration;

/// How long the selection pill takes to reach its new row.
pub const GLIDE: Duration = Duration::from_millis(90);
/// How long a toast takes to slide in.
pub const TOAST_IN: Duration = Duration::from_millis(120);
/// How long before its hold is up a toast starts to fade.
pub const TOAST_FADE: Duration = Duration::from_millis(350);
/// The time between two frames of a movement.
pub const FRAME: Duration = Duration::from_millis(16);

/// Cubic ease-out, `1 - (1 - p)^3`: most of the way at once, the last of it
/// gently. `p` outside 0..=1 is clamped.
pub fn ease_out(p: f64) -> f64 {
    1.0 - (1.0 - p.clamp(0.0, 1.0)).powi(3)
}

/// How far through `total` a movement is after `elapsed`, 0..=1. One that
/// takes no time is over.
pub fn progress(elapsed: Duration, total: Duration) -> f64 {
    if total.is_zero() {
        return 1.0;
    }
    (elapsed.as_secs_f64() / total.as_secs_f64()).min(1.0)
}

/// The row the pill is on, `p` of the way through its glide from `from` to
/// `to`: the eased position rounded, as a pill is on one row or the next and
/// never between two.
pub fn glide_row(from: u16, to: u16, p: f64) -> u16 {
    let (from, to) = (f64::from(from), f64::from(to));
    // Between two u16s, so the cast can't wrap.
    (from + (to - from) * ease_out(p)).round() as u16
}

/// Where a toast is in its movement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// The share of the way in from the right it still has to go: 1 as it
    /// starts, 0 once it is in place.
    pub slide: f64,
    /// How far it has faded towards the ground: 0 as drawn, 1 gone.
    pub fade: f64,
}

impl Pose {
    /// In place and not faded: a toast with motion off.
    pub const STILL: Pose = Pose {
        slide: 0.0,
        fade: 0.0,
    };

    /// A toast `shown` into a hold of `hold`: it slides in over `TOAST_IN`,
    /// eased out, and fades evenly over the last `TOAST_FADE` of the hold. The
    /// fade is inside the hold, so the toast leaves when it always did.
    pub fn at(shown: Duration, hold: Duration) -> Pose {
        let left = hold.saturating_sub(shown);
        Pose {
            slide: 1.0 - ease_out(progress(shown, TOAST_IN)),
            fade: 1.0 - progress(left, TOAST_FADE),
        }
    }

    /// Whether a toast `shown` into a hold of `hold` is moving: sliding in, or
    /// fading and not yet due down.
    pub fn moving(shown: Duration, hold: Duration) -> bool {
        shown < TOAST_IN || (shown < hold && hold.saturating_sub(shown) <= TOAST_FADE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: fn(u64) -> Duration = Duration::from_millis;

    #[test]
    fn ease_out_is_one_minus_the_cube_of_what_is_left() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        assert_eq!(ease_out(0.5), 0.875);
        assert!((ease_out(0.25) - (1.0 - 0.75f64.powi(3))).abs() < 1e-12);
        // Fast, then settling: it never goes back and never overshoots.
        let steps: Vec<f64> = (0..=100).map(|i| ease_out(f64::from(i) / 100.0)).collect();
        assert!(steps.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(steps.iter().all(|step| (0.0..=1.0).contains(step)));
        // Out of range is the nearer end.
        assert_eq!(ease_out(-1.0), 0.0);
        assert_eq!(ease_out(2.0), 1.0);
    }

    #[test]
    fn progress_is_the_share_of_the_time_gone_and_stops_at_one() {
        assert_eq!(progress(MS(0), GLIDE), 0.0);
        assert_eq!(progress(MS(45), GLIDE), 0.5);
        assert_eq!(progress(MS(90), GLIDE), 1.0);
        assert_eq!(progress(MS(500), GLIDE), 1.0);
        assert_eq!(progress(MS(0), Duration::ZERO), 1.0);
    }

    #[test]
    fn the_pill_glides_in_whole_rows_from_where_it_was_to_where_it_goes() {
        // Down eight rows: on its old row, most of the way by half time, there.
        assert_eq!(glide_row(4, 12, 0.0), 4);
        assert_eq!(glide_row(4, 12, 0.5), 11);
        assert_eq!(glide_row(4, 12, 1.0), 12);
        // Up is the same movement mirrored.
        assert_eq!(glide_row(12, 4, 0.0), 12);
        assert_eq!(glide_row(12, 4, 0.5), 5);
        assert_eq!(glide_row(12, 4, 1.0), 4);
        // One row away there is nothing between: it is on one or the other.
        assert_eq!(glide_row(7, 8, 0.1), 7);
        assert_eq!(glide_row(7, 8, 0.5), 8);
        // Going nowhere, it stays.
        assert_eq!(glide_row(7, 7, 0.5), 7);
    }

    #[test]
    fn a_toast_slides_in_over_120_ms() {
        let hold = MS(2500);
        assert_eq!(Pose::at(MS(0), hold).slide, 1.0);
        assert_eq!(Pose::at(MS(60), hold).slide, 0.125);
        assert_eq!(Pose::at(MS(120), hold).slide, 0.0);
        assert_eq!(Pose::at(MS(1000), hold), Pose::STILL);
    }

    #[test]
    fn a_toast_fades_over_the_last_350_ms_of_its_hold() {
        for hold in [MS(2500), MS(4500)] {
            let from = hold - TOAST_FADE;
            assert_eq!(Pose::at(from - MS(1), hold).fade, 0.0);
            assert_eq!(Pose::at(from, hold).fade, 0.0);
            assert_eq!(Pose::at(from + MS(175), hold).fade, 0.5);
            assert_eq!(Pose::at(hold, hold).fade, 1.0);
            // In place all the while.
            assert_eq!(Pose::at(from + MS(175), hold).slide, 0.0);
        }
    }

    #[test]
    fn a_toast_moves_while_it_slides_in_and_while_it_fades() {
        let hold = MS(2500);
        assert!(Pose::moving(MS(0), hold));
        assert!(Pose::moving(MS(119), hold));
        assert!(!Pose::moving(MS(120), hold));
        assert!(!Pose::moving(MS(2149), hold));
        assert!(Pose::moving(MS(2150), hold));
        assert!(Pose::moving(MS(2499), hold));
        // Its hold is up: it is taken down, not moved.
        assert!(!Pose::moving(MS(2500), hold));
    }
}
