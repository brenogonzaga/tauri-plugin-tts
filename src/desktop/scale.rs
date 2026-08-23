/// A backend's `(min, normal, max)` for one parameter.
pub type Range = (f32, f32, f32);

/// Maps `value`, where 1.0 means "the platform's normal", onto `range`.
///
/// Anchoring at normal rather than interpolating end to end is what keeps 1.0 sounding the
/// same everywhere; the two halves are scaled independently.
pub fn anchored_at_normal(value: f32, (user_min, user_max): (f32, f32), range: Range) -> f32 {
    let (min, normal, max) = range;

    if value <= 1.0 {
        let t = ((value - user_min) / (1.0 - user_min)).clamp(0.0, 1.0);
        min + t * (normal - min)
    } else {
        let t = ((value - 1.0) / (user_max - 1.0)).clamp(0.0, 1.0);
        normal + t * (max - normal)
    }
}

/// Volume has no "normal" anchor: 0.0..1.0 maps straight across the backend's full range.
/// On speech-dispatcher that range is -100..100, where normal and max are both 100.
pub fn across_range(value: f32, min: f32, max: f32) -> f32 {
    min + value.clamp(0.0, 1.0) * (max - min)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::RATE_RANGE;

    // Real backend ranges, from tts 0.26.
    const AV_RATE: Range = (0.1, 0.5, 2.0);
    const WINRT_RATE: Range = (0.5, 1.0, 6.0);
    const SPEECH_DISPATCHER_PITCH: Range = (-100.0, 0.0, 100.0);

    fn rate(value: f32, range: Range) -> f32 {
        anchored_at_normal(value, RATE_RANGE, range)
    }

    #[test]
    fn one_is_the_platform_normal_everywhere() {
        assert_eq!(rate(1.0, AV_RATE), 0.5);
        assert_eq!(rate(1.0, WINRT_RATE), 1.0);
        assert_eq!(
            anchored_at_normal(1.0, (0.5, 2.0), SPEECH_DISPATCHER_PITCH),
            0.0
        );
    }

    #[test]
    fn the_extremes_saturate_at_the_backend_bounds() {
        assert_eq!(rate(RATE_RANGE.1, AV_RATE), 2.0);
        assert_eq!(rate(RATE_RANGE.0, AV_RATE), 0.1);
        assert_eq!(rate(99.0, WINRT_RATE), 6.0);
        assert_eq!(rate(0.0, WINRT_RATE), 0.5);
        assert_eq!(
            anchored_at_normal(2.0, (0.5, 2.0), SPEECH_DISPATCHER_PITCH),
            100.0
        );
    }

    #[test]
    fn the_mapping_is_monotonic() {
        let steps = [0.1, 0.25, 0.5, 1.0, 2.0, 3.0, 4.0];
        for pair in steps.windows(2) {
            assert!(
                rate(pair[0], AV_RATE) < rate(pair[1], AV_RATE),
                "{} -> {}",
                pair[0],
                pair[1]
            );
        }
    }

    /// The documented floor has to be reachable and distinct: anchoring the lower half at
    /// 0.25 instead collapsed everything in 0.1..=0.25 onto the backend minimum.
    #[test]
    fn the_documented_floor_is_distinguishable() {
        assert_eq!(rate(RATE_RANGE.0, AV_RATE), 0.1);
        assert!(rate(0.1, AV_RATE) < rate(0.25, AV_RATE));
    }

    #[test]
    fn volume_spans_the_whole_backend_range() {
        assert_eq!(across_range(1.0, 0.0, 1.0), 1.0);
        assert_eq!(across_range(0.0, 0.0, 1.0), 0.0);
        assert_eq!(across_range(1.0, -100.0, 100.0), 100.0);
        assert_eq!(across_range(0.0, -100.0, 100.0), -100.0);
        assert_eq!(across_range(0.5, -100.0, 100.0), 0.0);
    }

    #[test]
    fn volume_clamps_out_of_range_input() {
        assert_eq!(across_range(2.0, 0.0, 1.0), 1.0);
        assert_eq!(across_range(-1.0, 0.0, 1.0), 0.0);
    }
}
