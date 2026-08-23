// swift-tools-version:5.5
import PackageDescription

/// Platform-independent logic behind the iOS plugin: input rules, parameter scaling and
/// voice selection. Kept free of UIKit, AVFoundation and Tauri so it builds — and is
/// testable — on the host.
let package = Package(
    name: "TtsCore",
    platforms: [
        .macOS(.v10_13),
        .iOS(.v13),
    ],
    products: [
        .library(name: "TtsCore", targets: ["TtsCore"]),
    ],
    targets: [
        .target(name: "TtsCore"),
        .testTarget(name: "TtsCoreTests", dependencies: ["TtsCore"]),
    ]
)
