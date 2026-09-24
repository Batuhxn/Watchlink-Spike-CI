// swift-tools-version: 6.0
// SPDX-License-Identifier: Apache-2.0
import PackageDescription

let package = Package(
    name: "VodozemacM3B4Feasibility",
    platforms: [.iOS(.v16)],
    products: [.library(name: "VodozemacBridge", targets: ["VodozemacBridge"])],
    targets: [
        .binaryTarget(name: "watchlink_vodozemac_feasibilityFFI", path: "Artifacts/WatchlinkVodozemacFFI.xcframework"),
        .target(
            name: "VodozemacBridge",
            dependencies: ["watchlink_vodozemac_feasibilityFFI"],
            path: "generated",
            exclude: ["watchlink_vodozemac_feasibilityFFI.h", "watchlink_vodozemac_feasibilityFFI.modulemap"]
        ),
        .testTarget(name: "VodozemacBridgeTests", dependencies: ["VodozemacBridge"], path: "Tests")
    ]
)
