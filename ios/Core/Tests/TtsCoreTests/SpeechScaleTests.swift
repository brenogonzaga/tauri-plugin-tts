import XCTest
@testable import TtsCore

/// Real backend bounds, so the assertions describe what a user actually hears.
private let avRate = SpeechScale.Range(min: 0.0, normal: 0.5, max: 1.0)
private let winRtRate = SpeechScale.Range(min: 0.5, normal: 1.0, max: 6.0)
private let speechDispatcherPitch = SpeechScale.Range(min: -100, normal: 0, max: 100)

private func rate(_ value: Float, _ range: SpeechScale.Range = avRate) -> Float {
    SpeechScale.anchoredAtNormal(value, userRange: SpeechLimits.rateRange, range: range)
}

final class SpeechScaleTests: XCTestCase {

    func testOneIsThePlatformNormalEverywhere() {
        XCTAssertEqual(rate(1.0), 0.5)
        XCTAssertEqual(rate(1.0, winRtRate), 1.0)
        XCTAssertEqual(
            SpeechScale.anchoredAtNormal(
                1.0,
                userRange: SpeechLimits.pitchRange,
                range: speechDispatcherPitch
            ),
            0.0
        )
    }

    func testTheExtremesSaturateAtTheBackendBounds() {
        XCTAssertEqual(rate(SpeechLimits.rateRange.upperBound), 1.0)
        XCTAssertEqual(rate(SpeechLimits.rateRange.lowerBound), 0.0)
        XCTAssertEqual(rate(99.0, winRtRate), 6.0)
        XCTAssertEqual(rate(0.0, winRtRate), 0.5)
    }

    /// Multiplying by the platform default reached the maximum at 2.0, so everything above
    /// it spoke at the same speed.
    func testTheUpperHalfOfTheRangeIsUsable() {
        let steps: [Float] = [1.0, 1.5, 2.0, 3.0, 4.0]

        for (previous, next) in zip(steps, steps.dropFirst()) {
            XCTAssertLessThan(rate(previous), rate(next), "\(previous) -> \(next)")
        }
    }

    func testTheMappingIsMonotonic() {
        let steps: [Float] = [0.1, 0.25, 0.5, 1.0, 2.0, 4.0]

        for (previous, next) in zip(steps, steps.dropFirst()) {
            XCTAssertLessThan(rate(previous), rate(next), "\(previous) -> \(next)")
        }
    }

    func testVolumeSpansTheWholeBackendRange() {
        XCTAssertEqual(SpeechScale.acrossRange(1.0, min: 0, max: 1), 1.0)
        XCTAssertEqual(SpeechScale.acrossRange(0.0, min: 0, max: 1), 0.0)
        XCTAssertEqual(SpeechScale.acrossRange(0.5, min: -100, max: 100), 0.0)
    }

    func testVolumeClampsOutOfRangeInput() {
        XCTAssertEqual(SpeechScale.acrossRange(2.0, min: 0, max: 1), 1.0)
        XCTAssertEqual(SpeechScale.acrossRange(-1.0, min: 0, max: 1), 0.0)
    }
}
