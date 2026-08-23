import XCTest
@testable import TtsCore

private func voices(_ languages: [String]) -> [VoiceInfo] {
    languages.map { VoiceInfo(id: $0, name: $0, language: $0) }
}

final class VoiceCatalogTests: XCTestCase {

    func testAFilterMatchesBareAndFullLocaleTags() {
        XCTAssertTrue(VoiceCatalog.languageMatches("pt-BR", filter: "pt"))
        XCTAssertTrue(VoiceCatalog.languageMatches("pt-BR", filter: "pt-BR"))
        XCTAssertTrue(VoiceCatalog.languageMatches("pt-BR", filter: "PT-br"))
        XCTAssertFalse(VoiceCatalog.languageMatches("en-US", filter: "pt"))
        XCTAssertFalse(VoiceCatalog.languageMatches("pt-BR", filter: "pt-PT"))
    }

    /// A substring match made a "US" filter return the en-US voices, disagreeing with what
    /// speak({ language: "US" }) would have picked.
    func testFilteringMatchesByPrefixNotSubstring() {
        let installed = voices(["en-US", "pt-BR", "pt-PT"])

        XCTAssertEqual(VoiceCatalog.filter(installed, language: nil).count, 3)
        XCTAssertEqual(
            VoiceCatalog.filter(installed, language: "pt").map(\.id),
            ["pt-BR", "pt-PT"]
        )
        XCTAssertEqual(VoiceCatalog.filter(installed, language: "pt-BR").map(\.id), ["pt-BR"])
        XCTAssertTrue(VoiceCatalog.filter(installed, language: "US").isEmpty)
    }

    func testAnExplicitVoiceIdWinsOverLanguage() {
        let installed = voices(["en-US", "pt-BR"])

        let selected = VoiceCatalog.select(installed, voiceId: "pt-BR", language: "en-US")
        XCTAssertEqual(selected?.id, "pt-BR")
    }

    func testLanguageSelectsTheFirstMatchingVoice() {
        let installed = voices(["en-US", "pt-BR", "pt-PT"])

        XCTAssertEqual(VoiceCatalog.select(installed, voiceId: nil, language: "pt")?.id, "pt-BR")
        XCTAssertNil(VoiceCatalog.select(installed, voiceId: nil, language: nil))
        XCTAssertNil(VoiceCatalog.select(installed, voiceId: nil, language: "ja"))
        XCTAssertNil(VoiceCatalog.select(installed, voiceId: "missing", language: nil))
    }

    func testTheWarningNamesWhicheverTheCallerAskedFor() {
        XCTAssertTrue(
            VoiceCatalog.unavailableWarning(voiceId: "v1", language: "pt-BR")!.contains("v1")
        )
        XCTAssertTrue(
            VoiceCatalog.unavailableWarning(voiceId: nil, language: "pt-BR")!.contains("pt-BR")
        )
        XCTAssertNil(VoiceCatalog.unavailableWarning(voiceId: nil, language: nil))
    }
}
