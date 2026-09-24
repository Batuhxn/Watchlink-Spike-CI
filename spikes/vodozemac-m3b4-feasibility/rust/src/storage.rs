// SPDX-License-Identifier: Apache-2.0
//! Disposable SQLite crash-boundary model; not a production storage API.

use crate::{BridgeError, SessionHandle};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FaultPoint {
    None,
    BeforeDecrypt,
    AfterDecrypt,
    AfterBegin,
    AfterStateWrite,
    AfterInboxWrite,
    BeforeCommit,
    AfterCommit,
    BeforeAck,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Sqlite")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Crypto")]
    Crypto(#[from] BridgeError),
    #[error("InjectedFailure({0:?})")]
    InjectedFailure(FaultPoint),
    #[error("GenerationMismatch")]
    GenerationMismatch,
    #[error("NoSession")]
    NoSession,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProcessOutcome {
    Committed { generation: u64 },
    AlreadyProcessed { generation: u64 },
}

pub struct PrototypeStore {
    path: PathBuf,
}

impl PrototypeStore {
    pub fn create(path: impl AsRef<Path>, initial_state: &[u8]) -> Result<Self, StoreError> {
        let store = Self {
            path: path.as_ref().to_owned(),
        };
        let connection = store.connection()?;
        connection.execute_batch("CREATE TABLE IF NOT EXISTS session_state (id INTEGER PRIMARY KEY CHECK(id=1), state BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS processed_inbox (event_id TEXT PRIMARY KEY, generation INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS generation (id INTEGER PRIMARY KEY CHECK(id=1), value INTEGER NOT NULL);")?;
        connection.execute(
            "INSERT OR IGNORE INTO session_state(id,state) VALUES(1,?1)",
            params![initial_state],
        )?;
        connection.execute("INSERT OR IGNORE INTO generation(id,value) VALUES(1,0)", [])?;
        Ok(store)
    }

    pub fn reopen(path: impl AsRef<Path>, external_generation: u64) -> Result<Self, StoreError> {
        let store = Self {
            path: path.as_ref().to_owned(),
        };
        if store.generation()? != external_generation {
            return Err(StoreError::GenerationMismatch);
        }
        Ok(store)
    }

    fn connection(&self) -> Result<Connection, StoreError> {
        let connection = Connection::open(&self.path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(connection)
    }

    pub fn generation(&self) -> Result<u64, StoreError> {
        let connection = self.connection()?;
        Ok(connection.query_row("SELECT value FROM generation WHERE id=1", [], |r| r.get(0))?)
    }

    pub fn inbox_count(&self) -> Result<u64, StoreError> {
        let connection = self.connection()?;
        Ok(connection.query_row("SELECT count(*) FROM processed_inbox", [], |r| r.get(0))?)
    }

    pub fn session_state(&self) -> Result<Vec<u8>, StoreError> {
        let connection = self.connection()?;
        connection
            .query_row("SELECT state FROM session_state WHERE id=1", [], |r| {
                r.get(0)
            })
            .map_err(Into::into)
    }

    pub fn process(
        &self,
        event_id: &str,
        ciphertext: Vec<u8>,
        fault: FaultPoint,
    ) -> Result<ProcessOutcome, StoreError> {
        let mut connection = self.connection()?;
        let generation: u64 =
            connection.query_row("SELECT value FROM generation WHERE id=1", [], |r| r.get(0))?;
        let seen: Option<u64> = connection
            .query_row(
                "SELECT generation FROM processed_inbox WHERE event_id=?1",
                [event_id],
                |r| r.get(0),
            )
            .optional()?;
        if seen.is_some() {
            return Ok(ProcessOutcome::AlreadyProcessed { generation });
        }
        fail(fault, FaultPoint::BeforeDecrypt)?;
        let old_state: Vec<u8> =
            connection.query_row("SELECT state FROM session_state WHERE id=1", [], |r| {
                r.get(0)
            })?;
        let session = SessionHandle::restore(old_state)?;
        let _plaintext = session.decrypt(ciphertext)?;
        fail(fault, FaultPoint::AfterDecrypt)?;
        let new_state = session.serialize()?;
        let transaction = connection.transaction()?;
        fail(fault, FaultPoint::AfterBegin)?;
        transaction.execute(
            "UPDATE session_state SET state=?1 WHERE id=1",
            params![new_state],
        )?;
        fail(fault, FaultPoint::AfterStateWrite)?;
        let next = generation + 1;
        transaction.execute(
            "INSERT INTO processed_inbox(event_id,generation) VALUES(?1,?2)",
            params![event_id, next],
        )?;
        fail(fault, FaultPoint::AfterInboxWrite)?;
        transaction.execute("UPDATE generation SET value=?1 WHERE id=1", [next])?;
        fail(fault, FaultPoint::BeforeCommit)?;
        transaction.commit()?;
        fail(fault, FaultPoint::AfterCommit)?;
        fail(fault, FaultPoint::BeforeAck)?;
        Ok(ProcessOutcome::Committed { generation: next })
    }
}

fn fail(selected: FaultPoint, here: FaultPoint) -> Result<(), StoreError> {
    if selected == here {
        Err(StoreError::InjectedFailure(here))
    } else {
        Ok(())
    }
}
