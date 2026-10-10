//! The editor sessions of this MCP server process.
//!
//! At most [`MAX_SESSIONS`] sessions are held. Opening one more evicts the
//! least recently used session that has no unsaved edits. When every held
//! session has unsaved edits, the open is refused with the next action.

use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

use crate::edit::client::Remote;
use crate::edit::doc::DocState;

/// Most sessions held at once. A local session keeps its text and up to
/// 1 MiB of undo history.
pub(super) const MAX_SESSIONS: usize = 16;

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

impl Entry {
    /// `true` for a local session with unsaved edits. An attached session
    /// keeps its edits in the server, so dropping it loses nothing.
    pub(super) fn holds_unsaved(&self) -> bool {
        match self {
            Entry::Local(doc) => doc.dirty(),
            Entry::Remote { .. } => false,
        }
    }
}

/// A held session and when it was last used.
pub(super) struct Held {
    pub(super) entry: Entry,
    /// The use counter at the last call that named this session.
    used: u64,
}

/// Every session, by id: `e<N>` for local, `r<N>` for attached.
#[derive(Default)]
pub(super) struct Registry {
    next: u64,
    clock: u64,
    sessions: BTreeMap<String, Held>,
}

impl Registry {
    /// A fresh id with `prefix`.
    pub(super) fn next_id(&mut self, prefix: char) -> String {
        self.next += 1;
        format!("{prefix}{}", self.next)
    }

    /// The id of the local session for canonical `path`.
    pub(super) fn local_id(&self, path: &std::path::Path) -> Option<String> {
        self.sessions.iter().find_map(|(id, h)| match &h.entry {
            Entry::Local(doc) if doc.path() == path => Some(id.clone()),
            Entry::Local(_) | Entry::Remote { .. } => None,
        })
    }

    /// The id of the attached session at the same server address.
    pub(super) fn remote_id(&self, remote: &Remote) -> Option<String> {
        self.sessions.iter().find_map(|(id, h)| match &h.entry {
            Entry::Remote { remote: r, .. } if r.addr() == remote.addr() => Some(id.clone()),
            Entry::Local(_) | Entry::Remote { .. } => None,
        })
    }

    /// The session `id`, marked as used.
    pub(super) fn get_mut(&mut self, id: &str) -> Option<&mut Entry> {
        self.clock += 1;
        let clock = self.clock;
        self.sessions.get_mut(id).map(|h| {
            h.used = clock;
            &mut h.entry
        })
    }

    /// The session `id`, without marking it.
    pub(super) fn get(&self, id: &str) -> Option<&Entry> {
        self.sessions.get(id).map(|h| &h.entry)
    }

    /// `true` when `id` names a session.
    pub(super) fn contains(&self, id: &str) -> bool {
        self.sessions.contains_key(id)
    }

    /// Every session, by id.
    pub(super) fn iter(&self) -> impl Iterator<Item = (&String, &Entry)> {
        self.sessions.iter().map(|(id, h)| (id, &h.entry))
    }

    /// Hold `entry` as `id`. A new id past [`MAX_SESSIONS`] first evicts the
    /// least recently used session without unsaved edits. Returns the
    /// evicted id.
    ///
    /// # Errors
    ///
    /// A message with the next action when every held session has unsaved
    /// edits.
    pub(super) fn insert(&mut self, id: String, entry: Entry) -> Result<Option<String>, String> {
        let mut evicted = None;
        if !self.sessions.contains_key(&id) && self.sessions.len() >= MAX_SESSIONS {
            let victim = self
                .sessions
                .iter()
                .filter(|(_, h)| !h.entry.holds_unsaved())
                .min_by_key(|(_, h)| h.used)
                .map(|(id, _)| id.clone())
                .ok_or_else(|| {
                    format!(
                        "{MAX_SESSIONS} editor sessions are open and each has unsaved edits; save \
                         one with zenith_editor_command file.save, or drop one with \
                         zenith_editor_close {{session, discard: true}}, then open again"
                    )
                })?;
            self.sessions.remove(&victim);
            evicted = Some(victim);
        }
        self.clock += 1;
        self.sessions.insert(
            id,
            Held {
                entry,
                used: self.clock,
            },
        );
        Ok(evicted)
    }

    /// Drop the session `id`. Returns it.
    pub(super) fn remove(&mut self, id: &str) -> Option<Entry> {
        self.sessions.remove(id).map(|h| h.entry)
    }
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(|| Mutex::new(Registry::default()));

/// The registry, locked. Every change is a whole-entry insert or a session
/// swap, so a poisoned lock is taken as is.
pub(super) fn registry() -> MutexGuard<'static, Registry> {
    REGISTRY.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(port: u16) -> Entry {
        Entry::Remote {
            remote: Remote::parse(&format!("http://127.0.0.1:{port}"), Some("t")).expect("url"),
            path: String::new(),
        }
    }

    #[test]
    fn past_the_cap_the_least_recently_used_session_goes() {
        let mut reg = Registry::default();
        for i in 0..MAX_SESSIONS {
            let id = reg.next_id('r');
            let port = u16::try_from(1000 + i).expect("port");
            assert_eq!(reg.insert(id, remote(port)).expect("insert"), None);
        }
        // Use r1, so r2 is now the oldest.
        assert!(reg.get_mut("r1").is_some());
        let id = reg.next_id('r');
        let evicted = reg.insert(id, remote(9)).expect("insert");
        assert_eq!(evicted.as_deref(), Some("r2"));
        assert!(reg.contains("r1"));
        assert_eq!(reg.iter().count(), MAX_SESSIONS);
        assert!(reg.remove("r1").is_some());
        assert!(!reg.contains("r1"));
    }

    #[test]
    fn sessions_with_unsaved_edits_are_never_evicted() {
        use crate::edit::doc::Target;
        use serde_json::json;
        use zenith_editor::Request;

        let dir = tempfile::tempdir().expect("dir");
        let mut reg = Registry::default();
        for i in 0..MAX_SESSIONS {
            let path = dir.path().join(format!("d{i}.zen"));
            std::fs::write(
                &path,
                "zenith version=1 {\n  tokens format=\"zenith-token-v1\" {\n    token \
                 id=\"c\" type=\"color\" value=\"#112233\"\n  }\n  document id=\"d\" {\n    \
                 page id=\"p\" w=(px)100 h=(px)100 {\n      rect id=\"r\" x=(px)1 y=(px)1 \
                 w=(px)5 h=(px)5 fill=(token)\"c\"\n    }\n  }\n}\n",
            )
            .expect("write");
            let target = Target::resolve(&path, None).expect("target");
            let (mut doc, _) = DocState::open(target).expect("open");
            let v = doc.session().version;
            let edit =
                Request::new("gesture.commit", json!({ "node": "r", "dx": 1, "dy": 0 })).at(v);
            assert!(doc.execute(&edit, None).result.is_ok());
            assert!(doc.dirty());
            let id = reg.next_id('e');
            reg.insert(id, Entry::Local(Box::new(doc))).expect("insert");
        }
        let id = reg.next_id('r');
        let err = reg.insert(id, remote(9)).expect_err("all dirty");
        assert!(err.contains("zenith_editor_close"), "{err}");
        assert_eq!(reg.iter().count(), MAX_SESSIONS);
    }
}
