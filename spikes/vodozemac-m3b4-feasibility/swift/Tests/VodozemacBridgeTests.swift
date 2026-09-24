// SPDX-License-Identifier: Apache-2.0
import Foundation
import XCTest
@testable import VodozemacBridge

final class VodozemacBridgeTests: XCTestCase {
    private final class ConcurrentResults: @unchecked Sendable {
        private let lock = NSLock()
        private var values: [Data] = []
        private var errors: [Error] = []

        func record(_ result: Result<Data, Error>) {
            lock.lock()
            defer { lock.unlock() }
            switch result {
            case .success(let value): values.append(value)
            case .failure(let error): errors.append(error)
            }
        }

        func snapshot() -> (values: [Data], errors: [Error]) {
            lock.lock()
            defer { lock.unlock() }
            return (values, errors)
        }
    }

    private func pair() throws -> (AccountHandle, AccountHandle, SessionHandle, SessionHandle) {
        let alice = AccountHandle()
        let bob = AccountHandle()
        let otk = try bob.generateOneTimeKey()
        let aliceSession = try alice.createOutbound(peerIdentity: bob.identity().curve25519, peerOneTimeKey: otk)
        let initial = try aliceSession.encrypt(plaintext: Data("message-000".utf8))
        let inbound = try bob.createInbound(peerIdentity: alice.identity().curve25519, prekeyJson: initial)
        XCTAssertEqual(inbound.plaintext, Data("message-000".utf8))
        return (alice, bob, aliceSession, inbound.session)
    }

    func testBasicAndSixtyMessages() throws {
        let (_, _, originalSender, originalReceiver) = try pair()
        var sender = originalSender
        var receiver = originalReceiver
        let reply = try receiver.encrypt(plaintext: Data("message-reply".utf8))
        XCTAssertEqual(try sender.decrypt(messageJson: reply), Data("message-reply".utf8))
        for number in 1...60 {
            let payload = Data(String(format: "message-%03d", number).utf8)
            if number % 10 < 5 {
                XCTAssertEqual(try receiver.decrypt(messageJson: sender.encrypt(plaintext: payload)), payload)
            } else {
                XCTAssertEqual(try sender.decrypt(messageJson: receiver.encrypt(plaintext: payload)), payload)
            }
            if number % 7 == 0 {
                sender = try SessionHandle.restore(bytes: sender.serialize())
                receiver = try SessionHandle.restore(bytes: receiver.serialize())
            }
        }
        let delayed = try sender.encrypt(plaintext: Data("message-delayed".utf8))
        let later = try sender.encrypt(plaintext: Data("message-later".utf8))
        XCTAssertEqual(try receiver.decrypt(messageJson: later), Data("message-later".utf8))
        XCTAssertEqual(try receiver.decrypt(messageJson: delayed), Data("message-delayed".utf8))
    }

    func testNonContributoryRejectedByRust() throws {
        let alice = AccountHandle()
        let bob = AccountHandle()
        let zeroPublic = Data(repeating: 0, count: 32).base64EncodedString().replacingOccurrences(of: "=", with: "")
        let otk = try bob.generateOneTimeKey()
        XCTAssertThrowsError(try alice.createOutbound(peerIdentity: zeroPublic, peerOneTimeKey: otk)) { error in
            XCTAssertEqual(error as? BridgeError, .NonContributoryKey)
        }
    }

    func testIssueGPreservesOneTimeKey() throws {
        let alice = AccountHandle()
        let bob = AccountHandle()
        let otk = try bob.generateOneTimeKey()
        let session = try alice.createOutbound(peerIdentity: bob.identity().curve25519, peerOneTimeKey: otk)
        let valid = try session.encrypt(plaintext: Data("message-valid".utf8))
        var object = try XCTUnwrap(JSONSerialization.jsonObject(with: valid) as? [String: Any])
        var messageBytes = try XCTUnwrap(Data(base64Encoded: try XCTUnwrap(object["body"] as? String)))
        messageBytes[messageBytes.index(before: messageBytes.endIndex)] ^= 1
        object["body"] = messageBytes.base64EncodedString()
        let invalid = try JSONSerialization.data(withJSONObject: object)
        let count = try bob.oneTimeKeyCount()
        XCTAssertThrowsError(try bob.createInbound(peerIdentity: alice.identity().curve25519, prekeyJson: invalid))
        XCTAssertEqual(try bob.oneTimeKeyCount(), count)
        XCTAssertEqual(try bob.createInbound(peerIdentity: alice.identity().curve25519, prekeyJson: valid).plaintext,
                       Data("message-valid".utf8))
        XCTAssertEqual(try bob.oneTimeKeyCount(), count - 1)
    }

    func testSkippedKeyAndReplay() throws {
        let (_, _, sender, receiver) = try pair()
        let messages = try (1...45).map { try sender.encrypt(plaintext: Data(String(format: "message-%03d", $0).utf8)) }
        XCTAssertEqual(try receiver.decrypt(messageJson: messages[44]), Data("message-045".utf8))
        XCTAssertThrowsError(try receiver.decrypt(messageJson: messages[0])) { error in
            XCTAssertEqual(error as? BridgeError, .MissingMessageKey)
        }
        XCTAssertEqual(try receiver.decrypt(messageJson: messages[4]), Data("message-005".utf8))
        let next = try sender.encrypt(plaintext: Data("message-replay".utf8))
        let old = try receiver.serialize()
        XCTAssertEqual(try receiver.decrypt(messageJson: next), Data("message-replay".utf8))
        let saved = try SessionHandle.restore(bytes: receiver.serialize())
        XCTAssertThrowsError(try saved.decrypt(messageJson: next))
        XCTAssertEqual(try SessionHandle.restore(bytes: old).decrypt(messageJson: next), Data("message-replay".utf8))
    }

    func testTooBigGap() throws {
        let (_, _, sender, receiver) = try pair()
        var far = Data()
        for _ in 0..<2002 {
            far = try sender.encrypt(plaintext: Data("message-gap".utf8))
        }
        XCTAssertThrowsError(try receiver.decrypt(messageJson: far)) { error in
            XCTAssertEqual(error as? BridgeError, .TooBigMessageGap)
        }
    }

    func testConcurrentMutationSerializedByRust() throws {
        let account = AccountHandle()
        let keys = ConcurrentResults()
        DispatchQueue.concurrentPerform(iterations: 64) { _ in
            keys.record(Result { Data(try account.generateOneTimeKey().utf8) })
        }
        let keyResults = keys.snapshot()
        XCTAssertTrue(keyResults.errors.isEmpty)
        XCTAssertEqual(keyResults.values.count, 64)
        XCTAssertEqual(Set(keyResults.values).count, 64)
        XCTAssertEqual(try account.oneTimeKeyCount(), 64)

        let (_, _, sender, _) = try pair()
        let messages = ConcurrentResults()
        DispatchQueue.concurrentPerform(iterations: 16) { index in
            messages.record(Result { try sender.encrypt(plaintext: Data(String(format: "message-%03d", index).utf8)) })
        }
        let messageResults = messages.snapshot()
        XCTAssertTrue(messageResults.errors.isEmpty)
        XCTAssertEqual(messageResults.values.count, 16)
        XCTAssertEqual(Set(messageResults.values).count, 16)
        XCTAssertFalse(try sender.serialize().isEmpty)
    }

    func testMalformedInputsSurfaceTypedErrors() throws {
        let alice = AccountHandle()
        let bob = AccountHandle()
        let otk = try bob.generateOneTimeKey()
        XCTAssertThrowsError(try alice.createOutbound(peerIdentity: "malformed", peerOneTimeKey: otk)) { error in
            XCTAssertEqual(error as? BridgeError, .InvalidPublicKey)
        }
        XCTAssertThrowsError(try AccountHandle.restore(bytes: Data("malformed".utf8))) { error in
            XCTAssertEqual(error as? BridgeError, .InvalidPickle)
        }
        let (_, _, _, receiver) = try pair()
        XCTAssertThrowsError(try receiver.decrypt(messageJson: Data("malformed".utf8))) { error in
            XCTAssertEqual(error as? BridgeError, .InvalidMessage)
        }
        XCTAssertFalse(try receiver.serialize().isEmpty)
    }

    func testRustPanicIsConvertedToSwiftError() {
        XCTAssertThrowsError(try ffiPanicProbe()) { error in
            XCTAssertTrue(error.localizedDescription.contains("M3B4 test-only panic probe"))
        }
    }
}
