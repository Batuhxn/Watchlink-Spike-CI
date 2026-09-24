// SPDX-License-Identifier: Apache-2.0
//! Narrow, test-only Olm bridge. Rust Mutexes serialize every mutation.

mod storage;
pub use storage::{FaultPoint, ProcessOutcome, PrototypeStore, StoreError};

use std::sync::{Arc, Mutex};
use vodozemac::{
    Curve25519PublicKey,
    olm::{
        Account, AccountPickle, DecryptionError, OlmMessage, Session, SessionConfig, SessionPickle,
    },
};

uniffi::setup_scaffolding!();

/// Spike-only FFI boundary probe; XCTest must observe a typed UniFFI panic error.
#[uniffi::export]
pub fn ffi_panic_probe() -> Result<(), BridgeError> {
    panic!("M3B4 test-only panic probe");
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum BridgeError {
    #[error("InvalidPublicKey")]
    InvalidPublicKey,
    #[error("InvalidMessage")]
    InvalidMessage,
    #[error("InvalidPickle")]
    InvalidPickle,
    #[error("NonContributoryKey")]
    NonContributoryKey,
    #[error("MissingOneTimeKey")]
    MissingOneTimeKey,
    #[error("InvalidInbound")]
    InvalidInbound,
    #[error("InvalidMAC")]
    InvalidMac,
    #[error("MissingMessageKey")]
    MissingMessageKey,
    #[error("TooBigMessageGap")]
    TooBigMessageGap,
    #[error("OtherOlmError")]
    OtherOlmError,
    #[error("SynchronizationError")]
    SynchronizationError,
}

impl From<DecryptionError> for BridgeError {
    fn from(error: DecryptionError) -> Self {
        match error {
            DecryptionError::MissingMessageKey(_) => Self::MissingMessageKey,
            DecryptionError::TooBigMessageGap(_, _) => Self::TooBigMessageGap,
            DecryptionError::NonContributoryKey => Self::NonContributoryKey,
            DecryptionError::InvalidMAC(_) | DecryptionError::InvalidMACLength(_, _) => {
                Self::InvalidMac
            }
            _ => Self::OtherOlmError,
        }
    }
}

fn decode_message(bytes: &[u8]) -> Result<OlmMessage, BridgeError> {
    serde_json::from_slice(bytes).map_err(|_| BridgeError::InvalidMessage)
}

fn encode_message(message: &OlmMessage) -> Result<Vec<u8>, BridgeError> {
    serde_json::to_vec(message).map_err(|_| BridgeError::InvalidMessage)
}

#[derive(uniffi::Record)]
pub struct PublicIdentity {
    pub curve25519: String,
    pub ed25519: String,
}

#[derive(uniffi::Record)]
pub struct InboundResult {
    pub session: Arc<SessionHandle>,
    pub plaintext: Vec<u8>,
}

#[derive(uniffi::Object)]
pub struct AccountHandle {
    inner: Mutex<Account>,
}

#[uniffi::export]
impl AccountHandle {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Account::new()),
        })
    }

    pub fn identity(&self) -> Result<PublicIdentity, BridgeError> {
        let account = self
            .inner
            .lock()
            .map_err(|_| BridgeError::SynchronizationError)?;
        Ok(PublicIdentity {
            curve25519: account.curve25519_key().to_base64(),
            ed25519: account.ed25519_key().to_base64(),
        })
    }

    pub fn generate_one_time_key(&self) -> Result<String, BridgeError> {
        let mut account = self
            .inner
            .lock()
            .map_err(|_| BridgeError::SynchronizationError)?;
        Ok(account.generate_one_time_keys(1).created[0].to_base64())
    }

    pub fn one_time_key_count(&self) -> Result<u32, BridgeError> {
        let account = self
            .inner
            .lock()
            .map_err(|_| BridgeError::SynchronizationError)?;
        Ok(account.stored_one_time_key_count() as u32)
    }

    pub fn create_outbound(
        &self,
        peer_identity: String,
        peer_one_time_key: String,
    ) -> Result<Arc<SessionHandle>, BridgeError> {
        let identity = Curve25519PublicKey::from_base64(&peer_identity)
            .map_err(|_| BridgeError::InvalidPublicKey)?;
        let otk = Curve25519PublicKey::from_base64(&peer_one_time_key)
            .map_err(|_| BridgeError::InvalidPublicKey)?;
        let account = self
            .inner
            .lock()
            .map_err(|_| BridgeError::SynchronizationError)?;
        let session = account
            .create_outbound_session(SessionConfig::version_1(), identity, otk)
            .map_err(|_| BridgeError::NonContributoryKey)?;
        Ok(Arc::new(SessionHandle {
            inner: Mutex::new(session),
        }))
    }

    pub fn create_inbound(
        &self,
        peer_identity: String,
        prekey_json: Vec<u8>,
    ) -> Result<InboundResult, BridgeError> {
        let identity = Curve25519PublicKey::from_base64(&peer_identity)
            .map_err(|_| BridgeError::InvalidPublicKey)?;
        let message = decode_message(&prekey_json)?;
        let OlmMessage::PreKey(prekey) = message else {
            return Err(BridgeError::InvalidMessage);
        };
        let mut account = self
            .inner
            .lock()
            .map_err(|_| BridgeError::SynchronizationError)?;
        let result = account
            .create_inbound_session(SessionConfig::version_1(), identity, &prekey)
            .map_err(|error| match error {
                vodozemac::olm::SessionCreationError::NonContributoryKey => {
                    BridgeError::NonContributoryKey
                }
                vodozemac::olm::SessionCreationError::MissingOneTimeKey(_) => {
                    BridgeError::MissingOneTimeKey
                }
                vodozemac::olm::SessionCreationError::Decryption(error) => error.into(),
                _ => BridgeError::InvalidInbound,
            })?;
        Ok(InboundResult {
            session: Arc::new(SessionHandle {
                inner: Mutex::new(result.session),
            }),
            plaintext: result.plaintext,
        })
    }

    pub fn serialize(&self) -> Result<Vec<u8>, BridgeError> {
        let account = self
            .inner
            .lock()
            .map_err(|_| BridgeError::SynchronizationError)?;
        serde_json::to_vec(&account.pickle()).map_err(|_| BridgeError::InvalidPickle)
    }

    #[uniffi::constructor]
    pub fn restore(bytes: Vec<u8>) -> Result<Arc<Self>, BridgeError> {
        let pickle: AccountPickle =
            serde_json::from_slice(&bytes).map_err(|_| BridgeError::InvalidPickle)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(Account::from_pickle(pickle)),
        }))
    }
}

#[derive(uniffi::Object)]
pub struct SessionHandle {
    inner: Mutex<Session>,
}

#[uniffi::export]
impl SessionHandle {
    pub fn encrypt(&self, plaintext: Vec<u8>) -> Result<Vec<u8>, BridgeError> {
        let mut session = self
            .inner
            .lock()
            .map_err(|_| BridgeError::SynchronizationError)?;
        let message = session
            .encrypt(plaintext)
            .map_err(|_| BridgeError::NonContributoryKey)?;
        encode_message(&message)
    }

    pub fn decrypt(&self, message_json: Vec<u8>) -> Result<Vec<u8>, BridgeError> {
        let message = decode_message(&message_json)?;
        let mut session = self
            .inner
            .lock()
            .map_err(|_| BridgeError::SynchronizationError)?;
        session.decrypt(&message).map_err(Into::into)
    }

    pub fn serialize(&self) -> Result<Vec<u8>, BridgeError> {
        let session = self
            .inner
            .lock()
            .map_err(|_| BridgeError::SynchronizationError)?;
        serde_json::to_vec(&session.pickle()).map_err(|_| BridgeError::InvalidPickle)
    }

    #[uniffi::constructor]
    pub fn restore(bytes: Vec<u8>) -> Result<Arc<Self>, BridgeError> {
        let pickle: SessionPickle =
            serde_json::from_slice(&bytes).map_err(|_| BridgeError::InvalidPickle)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(Session::from_pickle(pickle)),
        }))
    }
}
