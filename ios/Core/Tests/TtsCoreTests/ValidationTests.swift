import XCTest
@testable import TtsCore

final class ValidationTests: XCTestCase {

    func testEmptyTextIsRejected() {
        XCTAssertThrowsError(try SpeechValidator.validateText("")) { error in
            XCTAssertEqual(error as? TtsValidationError, .emptyText)
        }
    }

    func testTextIsMeasuredInUtf8BytesNotCharacters() throws {
        try SpeechValidator.validateText(String(repeating: "a", count: SpeechLimits.maxTextBytes))

        XCTAssertThrowsError(
            try SpeechValidator.validateText(
                String(repeating: "a", count: SpeechLimits.maxTextBytes + 1)
            )
        )

        // "é" is one character but two bytes, so half the limit already exceeds it.
        XCTAssertThrowsError(
            try SpeechValidator.validateText(
                String(repeating: "é", count: SpeechLimits.maxTextBytes / 2 + 1)
            )
        )
    }

    func testVoiceIdsAllowThePunctuationRealEnginesUse() throws {
        for id in [
            "com.apple.voice.enhanced.pt-BR",
            "com.apple.ttsbundle.Samantha-compact",
            "pt-br-x-afs#female_1-local",
        ] {
            try SpeechValidator.validateVoiceId(id)
        }
    }

    func testVoiceIdsRejectControlCharacters() {
        XCTAssertThrowsError(try SpeechValidator.validateVoiceId("voice\u{0}id")) { error in
            XCTAssertEqual(error as? TtsValidationError, .invalidVoiceId)
        }
    }

    func testVoiceIdsRejectOverlongInput() throws {
        try SpeechValidator.validateVoiceId(
            String(repeating: "v", count: SpeechLimits.maxVoiceIdLength)
        )

        XCTAssertThrowsError(
            try SpeechValidator.validateVoiceId(
                String(repeating: "v", count: SpeechLimits.maxVoiceIdLength + 1)
            )
        )
    }

    func testLanguageIsMeasuredInCharacters() throws {
        try SpeechValidator.validateLanguage(
            String(repeating: "é", count: SpeechLimits.maxLanguageLength)
        )

        XCTAssertThrowsError(
            try SpeechValidator.validateLanguage(
                String(repeating: "x", count: SpeechLimits.maxLanguageLength + 1)
            )
        )
    }

    /// JavaScript classifies failures by the code prefix, so every variant must carry one.
    func testEveryErrorCarriesItsCode() {
        let errors: [TtsValidationError] = [
            .emptyText,
            .textTooLong(length: 1, max: 0),
            .voiceIdTooLong(length: 1, max: 0),
            .invalidVoiceId,
            .languageTooLong(length: 1, max: 0),
        ]

        for error in errors {
            XCTAssertTrue(error.rejection.hasPrefix(error.code), error.rejection)
            XCTAssertFalse(error.code.isEmpty)
        }
    }
}
