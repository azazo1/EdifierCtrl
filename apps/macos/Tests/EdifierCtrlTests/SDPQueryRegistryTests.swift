import Foundation
import IOBluetooth
import IOKit
import XCTest
@testable import EdifierCtrl

final class SDPQueryRegistryTests: XCTestCase {
    private let address = "AA:BB:CC:DD:EE:FF"

    func testCallbackRespondsToSDKSelector() throws {
        try MacBluetooth.onMain {
            let registry = SDPQueryRegistry()
            let callback = try registry.begin(address: address) { _, _ in }
            let selector = #selector(IOBluetoothDeviceAsyncCallbacks.sdpQueryComplete(_:status:))
            XCTAssertEqual(NSStringFromSelector(selector), "sdpQueryComplete:status:")
            XCTAssertTrue(callback.responds(to: selector))
            XCTAssertEqual(#selector(SDPQueryCallback.sdpQueryComplete(_:status:)), selector)
        }
    }

    func testCancelledQueryAllowsRetryAndLateCallbackCannotCompleteOrUnlockRetry() throws {
        try MacBluetooth.onMain {
            let registry = SDPQueryRegistry()
            var oldCompletions = 0
            var newStatuses: [IOReturn] = []
            let old = try registry.begin(address: address) { _, _ in oldCompletions += 1 }
            registry.cancel(old.id)
            let retry = try registry.begin(address: address) { _, status in newStatuses.append(status) }
            old.sdpQueryComplete(nil, status: kIOReturnSuccess)
            XCTAssertEqual(oldCompletions, 0)
            XCTAssertTrue(newStatuses.isEmpty)
            XCTAssertThrowsError(try registry.begin(address: address) { _, _ in })
            retry.sdpQueryComplete(nil, status: kIOReturnNotFound)
            XCTAssertEqual(newStatuses, [kIOReturnNotFound])
            XCTAssertNoThrow(try registry.begin(address: address) { _, _ in })
        }
    }

    func testCancelledTargetSurvivesUntilLateCallbackWithoutRetainingItsCompletion() throws {
        try MacBluetooth.onMain {
            let registry = SDPQueryRegistry()
            weak var retainedTarget: SDPQueryCallback?
            weak var retainedOwner: NSObject?
            do {
                let owner = NSObject()
                retainedOwner = owner
                let callback = try registry.begin(address: address) { [owner] _, _ in _ = owner }
                retainedTarget = callback
                registry.cancel(callback.id)
            }
            XCTAssertNil(retainedOwner)
            XCTAssertNotNil(retainedTarget)
            retainedTarget?.sdpQueryComplete(nil, status: kIOReturnSuccess)
            XCTAssertNil(retainedTarget)
        }
    }

    func testRejectedSubmissionReleasesCapacityAndOnlyCompletesOnce() throws {
        try MacBluetooth.onMain {
            let registry = SDPQueryRegistry(maximumPending: 1)
            var statuses: [IOReturn] = []
            let callback = try registry.begin(address: address) { _, status in statuses.append(status) }
            registry.reject(callback.id, status: kIOReturnError)
            callback.sdpQueryComplete(nil, status: kIOReturnSuccess)
            XCTAssertEqual(statuses, [kIOReturnError])
            XCTAssertNoThrow(try registry.begin(address: address) { _, _ in })
        }
    }

    func testMissingCallbacksHaveBoundedRetentionAndRecoveredCapacity() throws {
        try MacBluetooth.onMain {
            let registry = SDPQueryRegistry(maximumPending: 2)
            let first = try registry.begin(address: address) { _, _ in }
            registry.cancel(first.id)
            let second = try registry.begin(address: address) { _, _ in }
            registry.cancel(second.id)
            XCTAssertThrowsError(try registry.begin(address: address) { _, _ in })
            first.sdpQueryComplete(nil, status: kIOReturnTimeout)
            XCTAssertNoThrow(try registry.begin(address: address) { _, _ in })
        }
    }
}
