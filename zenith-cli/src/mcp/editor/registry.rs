//! The editor sessions of this MCP server process.

use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

use crate::edit::client::Remote;
use crate::edit::doc::DocState;

/// One editor session.
pub(super) enum Entry {
    /// A session this process holds.
    Local(Box<DocState>),
    /// A running `zenith edit` server this process forwards to.
    Remote {
        remote: Remote,
        /// The server's document path, from its state.
        path: String,
    },
}

/// Every session, by id: `e<N>` for local, `r<N>` for attached.
#[derive(Default)]
pub(super) struct Registry {
    next: u64,
    pub(super) sessions: BTreeMap<String, Entry>,
}

impl Registry {
    /// A fresh id with `prefix`.
    pub(super) fn next_id(&mut self, prefix: char) -> String {
        self.next += 1;
        format!("{prefix}{}", self.next)
    }

    /// The id of the local session for canonical `path`.
    pub(super) fn local_id(&self, path: &std::path::Path) -> Option<String> {
        self.sessions.iter().find_map(|(id, e)| match e {
            Entry::Local(doc) if doc.path() == path => Some(id.clone()),
            Entry::Local(_) | Entry::Remote { .. } => None,
        })
    }

    /// The id of the attached session at the same server address.
    pub(super) fn remote_id(&self, remote: &Remote) -> Option<String> {
        self.sessions.iter().find_map(|(id, e)| match e {
            Entry::Remote { remote: r, .. } if r.addr() == remote.addr() => Some(id.clone()),
            Entry::Local(_) | Entry::Remote { .. } => None,
        })
    }
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(|| Mutex::new(Registry::default()));

/// The registry, locked. Every change is a whole-entry insert or a session
/// swap, so a poisoned lock is taken as is.
pub(super) fn registry() -> MutexGuard<'static, Registry> {
    REGISTRY.lock().unwrap_or_else(PoisonError::into_inner)
}
