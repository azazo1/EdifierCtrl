import Foundation
import XCTest
@testable import EdifierCtrl

final class BluetoothBridgeTests: XCTestCase {
    func testRFCOMMServiceUUIDUsesProtocolBytes() throws {
        let uuid = try XCTUnwrap(MacBluetooth.serviceUUID)
        XCTAssertEqual(uuid.length, 16)
        XCTAssertEqual(Data(referencing: uuid), Data([0xED, 0xF0, 0x00, 0x00, 0xED, 0xFE, 0xDF, 0xED, 0xFE, 0xDF, 0xED, 0xFE, 0xDF, 0xED, 0xFE, 0xDF]))
    }

    func testAudioEndpointRequiresExactBluetoothAddress() {
        let address = "AA:BB:CC:DD:EE:FF"
        XCTAssertEqual(canonicalBluetoothAddress("aa-bb-cc-dd-ee-ff"), address)
        XCTAssertTrue(CoreAudioBluetooth.uidMatches("AA-BB-CC-DD-EE-FF:output", address: address))
        XCTAssertTrue(CoreAudioBluetooth.uidMatches("aabbccddeeff:output", address: address))
        XCTAssertFalse(CoreAudioBluetooth.uidMatches("00112233-4455-6677-8899-aabbccddeeff", address: address))
        XCTAssertFalse(CoreAudioBluetooth.uidMatches("AA-BB-CC-DD-EE-00:output", address: address))
        XCTAssertFalse(CoreAudioBluetooth.uidMatches("000aabbccddeeff1", address: address))
        XCTAssertFalse(CoreAudioBluetooth.uidMatches("Edifier Headphones", address: address))
    }
}
