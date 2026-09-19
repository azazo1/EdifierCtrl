// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "EdifierCtrl",
    platforms: [
        .macOS(.v13),
    ],
    products: [
        .executable(name: "EdifierCtrl", targets: ["EdifierCtrl"]),
    ],
    targets: [
        .executableTarget(
            name: "EdifierCtrl",
            path: "Sources/EdifierCtrl",
            linkerSettings: [
                .linkedFramework("IOBluetooth"),
                .unsafeFlags(["-Xlinker", "-export_dynamic"]),
            ]
        ),
        .testTarget(
            name: "EdifierCtrlTests",
            dependencies: ["EdifierCtrl"]
        ),
    ]
)
