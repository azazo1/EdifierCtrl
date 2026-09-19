import Foundation
import XCTest
@testable import EdifierCtrl

final class GroupSecretTests: XCTestCase {
    func testRoundTripPreservesPassphraseAndIsolatesDirectories() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("edifier-secret-test-" + UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let first = root.appendingPathComponent("first")
        let second = root.appendingPathComponent("second")
        let secret = "  测试口令\nwith spaces  "
        XCTAssertNil(try GroupSecret.load(from: first))
        try GroupSecret.save(secret, in: first)
        XCTAssertEqual(try GroupSecret.load(from: first), secret)
        XCTAssertNil(try GroupSecret.load(from: second))
        try GroupSecret.save("another", in: second)
        try GroupSecret.save("replacement", in: first)
        XCTAssertEqual(try GroupSecret.load(from: first), "replacement")
        XCTAssertEqual(try GroupSecret.load(from: second), "another")
        let permissions = try FileManager.default.attributesOfItem(atPath: GroupSecret.file(in: first).path)[.posixPermissions] as? NSNumber
        XCTAssertEqual(permissions?.intValue, 0o600)
        try GroupSecret.delete(in: first)
        try GroupSecret.delete(in: first)
        XCTAssertNil(try GroupSecret.load(from: first))
        XCTAssertEqual(try GroupSecret.load(from: second), "another")
    }

    func testUnsupportedOrCorruptDocumentIsNotOverwritten() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("edifier-secret-test-" + UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        for text in ["{\"version\":9,\"passphrase\":\"future\"}", "{broken"] {
            let original = Data(text.utf8)
            try original.write(to: GroupSecret.file(in: directory))
            XCTAssertThrowsError(try GroupSecret.load(from: directory))
            XCTAssertThrowsError(try GroupSecret.save("replacement", in: directory))
            XCTAssertEqual(try Data(contentsOf: GroupSecret.file(in: directory)), original)
        }
    }

    func testSavingEmptyPassphraseRemovesOnlyItsOwnFile() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("edifier-secret-test-" + UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        try GroupSecret.save("temporary", in: directory)
        let unrelated = directory.appendingPathComponent("unrelated.txt")
        try Data("keep".utf8).write(to: unrelated)
        try GroupSecret.save("", in: directory)
        XCTAssertNil(try GroupSecret.load(from: directory))
        XCTAssertEqual(try String(contentsOf: unrelated, encoding: .utf8), "keep")
    }
}
