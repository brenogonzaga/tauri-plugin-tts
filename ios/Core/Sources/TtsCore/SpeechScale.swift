import Foundation

/// Maps the plugin's platform-independent rate, pitch and volume onto a backend's own scale.
///
/// AVFoundation's rate runs 0.0...1.0 with 0.5 as normal, so a value passed through
/// unchanged sounds nothing like the same value on Android or Windows.
public enum SpeechScale {

    /// A backend's own bounds for one parameter.
    public struct Range {
        public let min: Float
        public let normal: Float
        public let max: Float

        public init(min: Float, normal: Float, max: Float) {
            self.min = min
            self.normal = normal
            self.max = max
        }
    }

    /// Maps `value`, where 1.0 means "the platform's normal", onto `range`.
    ///
    /// The halves either side of normal are scaled independently. Multiplying by the
    /// platform default instead reached AVFoundation's maximum at 2.0, so 2.0, 3.0 and 4.0
    /// all spoke at the same speed and the top of the accepted range was unreachable.
    public static func anchoredAtNormal(
        _ value: Float,
        userRange: ClosedRange<Float>,
        range: Range
    ) -> Float {
        if value <= 1.0 {
            let span = 1.0 - userRange.lowerBound
            let t = clamped((value - userRange.lowerBound) / span)
            return range.min + t * (range.normal - range.min)
        }

        let span = userRange.upperBound - 1.0
        let t = clamped((value - 1.0) / span)
        return range.normal + t * (range.max - range.normal)
    }

    /// Volume has no "normal" anchor: it maps straight across the backend's full range.
    public static func acrossRange(_ value: Float, min: Float, max: Float) -> Float {
        min + clamped(value) * (max - min)
    }

    private static func clamped(_ value: Float) -> Float {
        Swift.min(Swift.max(value, 0.0), 1.0)
    }
}
