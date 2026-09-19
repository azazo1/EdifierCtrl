import XCTest
@testable import EdifierCtrl

final class SessionStateTests: XCTestCase {
    func testBluetoothAddressesNormalizeAcrossPlatforms() {
        XCTAssertEqual(BluetoothAddress.normalize(" a1-b2-c3-d4-e5-f6 "), "A1:B2:C3:D4:E5:F6")
        XCTAssertEqual(BluetoothAddress.normalize("a1b2c3d4e5f6"), "A1:B2:C3:D4:E5:F6")
        XCTAssertNil(BluetoothAddress.normalize("not-a-device"))
        XCTAssertNil(BluetoothAddress.normalize("AA:BB:CC:DD:EE:FF:00"))
    }

    func testSwitchingHeadphonesDoesNotRetainReadings() throws {
        var state = SessionState()
        state.apply(try event(#"{"kind":"bt_state","connected":true,"address":"AA:BB:CC:DD:EE:01"}"#))
        state.apply(try event(#"{"kind":"headset","notification":{"kind":"battery","percent":87}}"#))
        state.apply(try event(#"{"kind":"headset","notification":{"kind":"name","name":"First"}}"#))
        state.apply(try event(#"{"kind":"bt_state","connected":true,"address":"AA-BB-CC-DD-EE-02"}"#))
        XCTAssertTrue(state.connected)
        XCTAssertEqual(state.address, "AA:BB:CC:DD:EE:02")
        XCTAssertNil(state.battery)
        XCTAssertNil(state.name)
    }

    func testDisconnectedStateIgnoresLateHeadsetPackets() throws {
        var state = SessionState()
        state.apply(try event(#"{"kind":"bt_state","connected":true,"address":"AA:BB:CC:DD:EE:01"}"#))
        state.apply(try event(#"{"kind":"headset","notification":{"kind":"battery","percent":50}}"#))
        state.apply(try event(#"{"kind":"bt_state","connected":false}"#))
        state.apply(try event(#"{"kind":"headset","notification":{"kind":"battery","percent":99}}"#))
        XCTAssertFalse(state.connected)
        XCTAssertNil(state.battery)
    }

    func testControlLinkNeverImpliesAudioOwnership() throws {
        var state = SessionState()
        state.apply(try event(#"{"kind":"bt_state","connected":true,"address":"AA:BB:CC:DD:EE:01"}"#))
        XCTAssertEqual(state.audio, "unknown")
        XCTAssertNil(state.holding)
        state.holding = "AA:BB:CC:DD:EE:01"
        state.apply(try event(#"{"kind":"audio","state":"disconnected"}"#))
        XCTAssertNil(state.holding)
    }

    func testRealNotificationSchemaDecodesAndRejectsInvalidBattery() throws {
        var state = SessionState()
        state.connected = true
        state.apply(try event(#"{"kind":"headset","notification":{"kind":"noise","mode":"ambient","ambient_volume":-2}}"#))
        XCTAssertEqual(state.noise, "ambient")
        XCTAssertEqual(state.ambientVolume, -2)
        state.apply(try event(#"{"kind":"headset","notification":{"kind":"battery","percent":255}}"#))
        XCTAssertNil(state.battery)
        state.apply(try event(#"{"kind":"headset","notification":{"kind":"battery","percent":0}}"#))
        XCTAssertEqual(state.battery, 0)
    }

    func testHandoffFailureDoesNotClaimSuccess() throws {
        var state = SessionState()
        state.apply(try event(#"{"kind":"handoff","progress":{"kind":"waiting_peer"}}"#))
        XCTAssertEqual(state.handoff?.isActive, true)
        state.apply(try event(#"{"kind":"handoff","progress":{"kind":"failed","reason":"timeout"}}"#))
        XCTAssertEqual(state.handoff?.isActive, false)
        XCTAssertEqual(state.handoff?.reason, "timeout")
        XCTAssertNil(state.holding)
    }

    func testSharedPeerAndProfileWireFormat() throws {
        let peer = try NativeJSON.decode(GroupPeer.self, #"{"id":"android-1","hostname":"Phone","os":"android","can_audio":true,"holding":"AA:BB:CC:DD:EE:FF","app_version":"0.1.0"}"#)
        XCTAssertTrue(peer.canAudio)
        XCTAssertEqual(peer.platformName, "Android")
        let profile = try NativeJSON.decode(HeadphoneProfile.self, #"{"id":"w820nb","display_name":"W820NB","max_name_len":24,"features":["noise","playback"]}"#)
        XCTAssertTrue(profile.supports("noise"))
        XCTAssertFalse(profile.supports("sound_effect"))
        XCTAssertEqual(profile.maxNameLen, 24)
    }

    private func event(_ json: String) throws -> NativeEvent { try NativeJSON.decode(NativeEvent.self, json) }
}
