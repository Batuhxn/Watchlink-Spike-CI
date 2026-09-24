// SPDX-License-Identifier: Apache-2.0
use std::sync::Arc;
use vodozemac::{Curve25519PublicKey, olm::OlmMessage};
use watchlink_vodozemac_feasibility::{
    AccountHandle, BridgeError, FaultPoint, ProcessOutcome, PrototypeStore, SessionHandle,
    StoreError,
};

type TestPair = (
    Arc<AccountHandle>,
    Arc<AccountHandle>,
    Arc<SessionHandle>,
    Arc<SessionHandle>,
    Vec<u8>,
);

fn pair() -> TestPair {
    let alice = AccountHandle::new();
    let bob = AccountHandle::new();
    let otk = bob.generate_one_time_key().unwrap();
    let sender = alice
        .create_outbound(bob.identity().unwrap().curve25519, otk)
        .unwrap();
    let initial = sender.encrypt(b"message-000".to_vec()).unwrap();
    let result = bob
        .create_inbound(alice.identity().unwrap().curve25519, initial.clone())
        .unwrap();
    assert_eq!(result.plaintext, b"message-000");
    (alice, bob, sender, result.session, initial)
}

fn corrupt_mac(encoded: &[u8]) -> Vec<u8> {
    let message: OlmMessage = serde_json::from_slice(encoded).unwrap();
    let (kind, mut bytes) = message.to_parts();
    *bytes.last_mut().unwrap() ^= 1;
    let malformed = OlmMessage::from_parts(kind, &bytes).unwrap();
    serde_json::to_vec(&malformed).unwrap()
}

#[test]
fn basic_bidirectional_and_60_messages_with_restore() {
    let (_alice, _bob, mut sender, mut receiver, _) = pair();
    let reply = receiver.encrypt(b"message-reply".to_vec()).unwrap();
    assert_eq!(sender.decrypt(reply).unwrap(), b"message-reply");
    for n in 1..=60 {
        let label = format!("message-{n:03}").into_bytes();
        if n % 10 < 5 {
            let encrypted = sender.encrypt(label.clone()).unwrap();
            assert_eq!(receiver.decrypt(encrypted).unwrap(), label);
        } else {
            let encrypted = receiver.encrypt(label.clone()).unwrap();
            assert_eq!(sender.decrypt(encrypted).unwrap(), label);
        }
        if n % 7 == 0 {
            sender = SessionHandle::restore(sender.serialize().unwrap()).unwrap();
            receiver = SessionHandle::restore(receiver.serialize().unwrap()).unwrap();
        }
    }
    let held = sender.encrypt(b"message-held".to_vec()).unwrap();
    let later = sender.encrypt(b"message-later".to_vec()).unwrap();
    assert_eq!(receiver.decrypt(later).unwrap(), b"message-later");
    assert_eq!(receiver.decrypt(held).unwrap(), b"message-held");
}

#[test]
fn upstream_non_contributory_rejection() {
    let alice = AccountHandle::new();
    let bob = AccountHandle::new();
    let bad = Curve25519PublicKey::from_bytes([0; 32]).to_base64();
    let valid_otk = bob.generate_one_time_key().unwrap();
    assert!(matches!(
        alice.create_outbound(bad.clone(), valid_otk),
        Err(BridgeError::NonContributoryKey)
    ));
    assert!(matches!(
        alice.create_outbound(bob.identity().unwrap().curve25519, bad),
        Err(BridgeError::NonContributoryKey)
    ));
}

#[test]
fn issue_g_invalid_prekey_does_not_consume_otk() {
    let alice = AccountHandle::new();
    let bob = AccountHandle::new();
    let otk = bob.generate_one_time_key().unwrap();
    let session = alice
        .create_outbound(bob.identity().unwrap().curve25519, otk)
        .unwrap();
    let valid = session.encrypt(b"message-valid".to_vec()).unwrap();
    let bad = corrupt_mac(&valid);
    let count = bob.one_time_key_count().unwrap();
    assert!(matches!(
        bob.create_inbound(alice.identity().unwrap().curve25519, bad),
        Err(BridgeError::InvalidMac)
    ));
    assert_eq!(bob.one_time_key_count().unwrap(), count);
    let result = bob
        .create_inbound(alice.identity().unwrap().curve25519, valid)
        .unwrap();
    assert_eq!(result.plaintext, b"message-valid");
    assert_eq!(bob.one_time_key_count().unwrap(), count - 1);
}

#[test]
fn skipped_keys_45_before_1_and_gap() {
    let (_alice, _bob, sender, receiver, _) = pair();
    let mut messages = Vec::new();
    for n in 1..=45 {
        messages.push(
            sender
                .encrypt(format!("message-{n:03}").into_bytes())
                .unwrap(),
        );
    }
    assert_eq!(
        receiver.decrypt(messages[44].clone()).unwrap(),
        b"message-045"
    );
    assert!(matches!(
        receiver.decrypt(messages[0].clone()),
        Err(BridgeError::MissingMessageKey)
    ));
    assert_eq!(
        receiver.decrypt(messages[4].clone()).unwrap(),
        b"message-005"
    );
    let older = SessionHandle::restore(receiver.serialize().unwrap()).unwrap();
    let mut far = Vec::new();
    for _ in 0..2002 {
        far = sender.encrypt(b"message-gap".to_vec()).unwrap();
    }
    assert!(matches!(
        older.decrypt(far),
        Err(BridgeError::TooBigMessageGap)
    ));
}

#[test]
fn replay_after_persistence_and_old_snapshot() {
    let (_alice, _bob, sender, receiver, _) = pair();
    let encoded = sender.encrypt(b"message-replay".to_vec()).unwrap();
    let before = receiver.serialize().unwrap();
    assert_eq!(
        receiver.decrypt(encoded.clone()).unwrap(),
        b"message-replay"
    );
    let after = SessionHandle::restore(receiver.serialize().unwrap()).unwrap();
    assert!(matches!(
        after.decrypt(encoded.clone()),
        Err(BridgeError::MissingMessageKey)
    ));
    let restored_old = SessionHandle::restore(before).unwrap();
    assert_eq!(restored_old.decrypt(encoded).unwrap(), b"message-replay");
}

fn db_path(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "m3b4-{label}-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn fault_boundaries_are_atomic_and_generation_detects_rollback() {
    for fault in [
        FaultPoint::BeforeDecrypt,
        FaultPoint::AfterDecrypt,
        FaultPoint::AfterBegin,
        FaultPoint::AfterStateWrite,
        FaultPoint::AfterInboxWrite,
        FaultPoint::BeforeCommit,
        FaultPoint::AfterCommit,
        FaultPoint::BeforeAck,
    ] {
        let (_alice, _bob, sender, receiver, _) = pair();
        let encrypted = sender.encrypt(b"message-db".to_vec()).unwrap();
        let path = db_path("fault");
        let initial = receiver.serialize().unwrap();
        let store = PrototypeStore::create(&path, &initial).unwrap();
        assert!(matches!(
            store.process("event-1", encrypted.clone(), fault),
            Err(StoreError::InjectedFailure(_))
        ));
        let committed = matches!(fault, FaultPoint::AfterCommit | FaultPoint::BeforeAck);
        let expected = u64::from(committed);
        assert_eq!(store.generation().unwrap(), expected);
        assert_eq!(store.inbox_count().unwrap(), expected);
        if committed {
            assert_ne!(store.session_state().unwrap(), initial);
            assert!(matches!(
                PrototypeStore::reopen(&path, 0),
                Err(StoreError::GenerationMismatch)
            ));
            let reopened = PrototypeStore::reopen(&path, 1).unwrap();
            assert_eq!(
                reopened
                    .process("event-1", encrypted, FaultPoint::None)
                    .unwrap(),
                ProcessOutcome::AlreadyProcessed { generation: 1 }
            );
            assert_eq!(reopened.inbox_count().unwrap(), 1);
        } else {
            assert_eq!(store.session_state().unwrap(), initial);
            let reopened = PrototypeStore::reopen(&path, 0).unwrap();
            assert_eq!(
                reopened
                    .process("event-1", encrypted, FaultPoint::None)
                    .unwrap(),
                ProcessOutcome::Committed { generation: 1 }
            );
        }
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(path.with_extension("sqlite-shm"));
    }
}

#[test]
fn concurrent_calls_share_one_rust_mutex() {
    let (_alice, _bob, sender, receiver, _) = pair();
    let mut threads = Vec::new();
    for n in 0..24 {
        let sender = Arc::clone(&sender);
        threads.push(std::thread::spawn(move || {
            sender
                .encrypt(format!("message-{n:03}").into_bytes())
                .unwrap()
        }));
    }
    for thread in threads {
        assert!(receiver.decrypt(thread.join().unwrap()).is_ok());
    }
    // Delivery order remains within the 40-key retention window.
    assert!(sender.serialize().is_ok());
    assert!(receiver.serialize().is_ok());
}

#[test]
fn receiving_chain_eviction_is_observable_on_old_ciphertext() {
    let (_alice, _bob, alice, bob, _) = pair();
    let first_reply = bob.encrypt(b"message-reply".to_vec()).unwrap();
    assert_eq!(alice.decrypt(first_reply).unwrap(), b"message-reply");
    let delayed = alice.encrypt(b"message-delayed-chain".to_vec()).unwrap();
    let current = alice.encrypt(b"message-current-chain".to_vec()).unwrap();
    assert_eq!(bob.decrypt(current).unwrap(), b"message-current-chain");
    for _ in 0..6 {
        let reply = bob.encrypt(b"message-turn".to_vec()).unwrap();
        assert_eq!(alice.decrypt(reply).unwrap(), b"message-turn");
        let next = alice.encrypt(b"message-turn".to_vec()).unwrap();
        assert_eq!(bob.decrypt(next).unwrap(), b"message-turn");
    }
    let error = bob.decrypt(delayed).unwrap_err();
    eprintln!("receiving-chain eviction error type: {error:?}");
    assert!(matches!(error, BridgeError::InvalidMac), "{error:?}");
}

#[test]
fn current_encrypted_pickle_is_deterministic_under_key_reuse() {
    let key = [7_u8; 32]; // test-only key, never logged or persisted
    let account = vodozemac::olm::Account::new();
    let first = account.pickle().encrypt(&key);
    let second = account.pickle().encrypt(&key);
    assert_eq!(first, second);
    let restored = vodozemac::olm::AccountPickle::from_encrypted(&first, &key).unwrap();
    assert_eq!(
        vodozemac::olm::Account::from_pickle(restored).curve25519_key(),
        account.curve25519_key()
    );
}
