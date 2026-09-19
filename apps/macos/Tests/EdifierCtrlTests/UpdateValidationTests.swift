import Foundation
import XCTest
@testable import EdifierCtrl

final class UpdateValidationTests: XCTestCase {
    func testVersionNormalizesBuildCommitWithoutCreatingUpgrade() throws {
        let tagged = try XCTUnwrap(UpdateVersion("v1.2.3"))
        XCTAssertEqual(tagged, UpdateVersion("v1.2.3-a1b2c3d"))
        XCTAssertEqual(tagged, UpdateVersion("1.2.3^a1b2c3d"))
        XCTAssertEqual(tagged, UpdateVersion("1.2.3+build.42"))
        XCTAssertNil(UpdateVersion("dev-build"))
        XCTAssertNil(UpdateVersion("v01.2.3"))
        XCTAssertNil(UpdateVersion("1.2"))
        XCTAssertNil(UpdateVersion("1.2.3-01"))
        XCTAssertTrue(try XCTUnwrap(UpdateVersion("1.2.4")) > tagged)
    }

    func testVersionUsesSemverPrereleaseOrdering() throws {
        let versions = ["1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-alpha.beta", "1.0.0-beta", "1.0.0-beta.2", "1.0.0-beta.11", "1.0.0-rc.1", "1.0.0"]
        for (left, right) in zip(versions, versions.dropFirst()) {
            XCTAssertLessThan(try XCTUnwrap(UpdateVersion(left)), try XCTUnwrap(UpdateVersion(right)))
        }
    }

    func testChecksumSupportsBinaryMarkerAndRejectsAmbiguity() throws {
        let name = "EdifierCtrl-v1.0.0-macos-aarch64.dmg"
        let digest = String(repeating: "Ab", count: 32)
        XCTAssertEqual(try UpdateValidation.checksum("\(digest) *\(name)\r\n", named: name), digest.lowercased())
        XCTAssertEqual(try UpdateValidation.checksum("\(digest)  \(name)\n", named: name), digest.lowercased())
        XCTAssertEqual(try UpdateValidation.checksum("\(digest)  other.dmg\r\n\(digest) *\(name)\r\n", named: name), digest.lowercased())
        XCTAssertThrowsError(try UpdateValidation.checksum("\(digest)  ../\(name)\n", named: name))
        XCTAssertThrowsError(try UpdateValidation.checksum("\(digest)  \(name)\n\(digest)  \(name)\n", named: name))
        XCTAssertThrowsError(try UpdateValidation.checksum("\(String(repeating: "z", count: 64))  \(name)", named: name))
    }

    func testAssetNameAndURLStayInsideExactRelease() throws {
        let name = try UpdateValidation.assetName(tag: "v1.0.0", architecture: "aarch64")
        XCTAssertEqual(name, "EdifierCtrl-v1.0.0-macos-aarch64.dmg")
        XCTAssertThrowsError(try UpdateValidation.assetName(tag: "../../other", architecture: "aarch64"))
        XCTAssertThrowsError(try UpdateValidation.assetName(tag: "v1.0.0", architecture: "arm64"))
        XCTAssertNoThrow(try UpdateValidation.assetURL(URL(string: "https://github.com/azazo1/EdifierCtrl/releases/download/v1.0.0/\(name)")!, repository: "azazo1/EdifierCtrl", tag: "v1.0.0", name: name))
        XCTAssertThrowsError(try UpdateValidation.assetURL(URL(string: "https://github.com/other/repo/releases/download/v1.0.0/\(name)")!, repository: "azazo1/EdifierCtrl", tag: "v1.0.0", name: name))
        XCTAssertThrowsError(try UpdateValidation.repository(nil))
        XCTAssertThrowsError(try UpdateValidation.repository("../EdifierCtrl"))
    }

    func testInstallPathsAreDerivedFromBundleAndNeverApplicationsDirectory() throws {
        let bundle = URL(fileURLWithPath: "/Applications/EdifierCtrl.app")
        let paths = try UpdateInstallPaths(bundle: bundle, token: "test-token")
        XCTAssertEqual(paths.staging.path, "/Applications/.EdifierCtrl.app.update-test-token")
        XCTAssertEqual(paths.backup.path, "/Applications/EdifierCtrl.app.old")
        XCTAssertEqual(paths.bundle.deletingLastPathComponent(), paths.staging.deletingLastPathComponent())
        XCTAssertEqual(paths.bundle.deletingLastPathComponent(), paths.backup.deletingLastPathComponent())
        XCTAssertThrowsError(try UpdateInstallPaths(bundle: URL(fileURLWithPath: "/Applications"), token: "test"))
        XCTAssertThrowsError(try UpdateInstallPaths(bundle: URL(fileURLWithPath: "/EdifierCtrl.app"), token: "test"))
        XCTAssertThrowsError(try UpdateInstallPaths(bundle: bundle, token: "../../other"))
    }

    func testArchiveHashRejectsSymbolicLinksAndSupportsCancellation() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("edifier-update-test-" + UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let archive = directory.appendingPathComponent("archive.dmg")
        try Data("abc".utf8).write(to: archive)
        XCTAssertEqual(try UpdateFiles.hash(archive), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        XCTAssertThrowsError(try UpdateFiles.hash(archive, isCancelled: { true }))
        let link = directory.appendingPathComponent("link.dmg")
        try FileManager.default.createSymbolicLink(at: link, withDestinationURL: archive)
        XCTAssertThrowsError(try UpdateFiles.hash(link))
        XCTAssertThrowsError(try UpdateFiles.regularFile(link))
    }

    func testPortableAndMountedAppCannotBeReplaced() {
        XCTAssertNil(UpdateInstaller.installedBundle(executable: URL(fileURLWithPath: "/tmp/EdifierCtrl"), bundle: URL(fileURLWithPath: "/tmp")))
        let mounted = URL(fileURLWithPath: "/Volumes/EdifierCtrl/EdifierCtrl.app")
        XCTAssertNil(UpdateInstaller.installedBundle(executable: mounted.appendingPathComponent("Contents/MacOS/EdifierCtrl"), bundle: mounted))
    }
}
