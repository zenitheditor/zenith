//! [`DocState`]: one document's editor session, its saved text, and the
//! disk state it was last synced with.

use std::path::Path;

use serde_json::{Value, json};
use zenith_editor::{
    DeltaOut, EditorError, Env, Outcome, RenderedImage, Request, Session, TextDelta, Work, execute,
    hex_sha256,
};
use zenith_pipeline::{Host, PolicyFlags};

use super::confined::ConfinedFs;
use super::disk::{self, DiskRead, Stamp};
use super::event::Event;
use super::target::Target;

/// What the disk held the last time the session synced with it.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Disk {
    /// The stamp of the last read or write. `None` while the file is
    /// missing.
    pub(super) stamp: Option<Stamp>,
    /// `true` after a poll found the file gone.
    pub(super) missing: bool,
    /// A change seen by one poll. The next poll acts on it only when it
    /// sees the same state, so a write in progress (truncate, then write)
    /// is never taken half done.
    pub(super) pending: Pending,
}

/// A disk change waiting for a second poll.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Pending {
    /// Nothing waits.
    #[default]
    Nothing,
    /// The file has this new stamp.
    Changed(Stamp),
    /// The file is gone.
    Missing,
}

impl Disk {
    /// The disk state right after a read or write that saw `stamp`.
    pub(super) fn synced(stamp: Option<Stamp>) -> Self {
        Self {
            stamp,
            missing: false,
            pending: Pending::Nothing,
        }
    }
}

/// A disk change the session did not take because it has unsaved edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Conflict {
    /// The text on disk.
    pub(super) text: String,
    /// Its modification time, in milliseconds since the Unix epoch.
    pub(super) mtime_ms: u64,
}

/// A text change one command made, for diffs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextChange {
    /// The text before.
    pub(crate) before: String,
    /// The text after.
    pub(crate) after: String,
}

/// What one command did.
#[derive(Debug)]
pub(crate) struct Ran {
    /// The reply, or why the command did not run.
    pub(crate) result: Result<Value, EditorError>,
    /// The PNG of `doc.render` and `gesture.preview`.
    pub(crate) image: Option<RenderedImage>,
    /// Engine work, summed over the engine calls the command made.
    pub(crate) work: Work,
    /// Events for the other clients, in order.
    pub(crate) events: Vec<Event>,
    /// The text change, when the text changed.
    pub(crate) change: Option<TextChange>,
}

impl Ran {
    /// A reply with no image, work, events, or change.
    pub(super) fn reply(result: Result<Value, EditorError>) -> Self {
        Self {
            result,
            image: None,
            work: Work::default(),
            events: Vec::new(),
            change: None,
        }
    }
}

/// One document's editor state.
///
/// `saved_text` is the text on disk as of the last load, save, or accepted
/// disk change. The session is dirty while its text differs from it. A disk
/// change on a clean session reloads it. A disk change on a dirty session
/// becomes a [`Conflict`]: the session keeps its text until a client sends
/// `file.reload` (take the disk text) or `file.save {overwrite: true}`
/// (replace the disk text). Neither side is lost silently.
pub(crate) struct DocState {
    pub(super) target: Target,
    pub(super) session: Session,
    pub(super) saved_text: String,
    pub(super) disk: Disk,
    pub(super) conflict: Option<Conflict>,
    /// The file is read-only: saves fail until it is writable again.
    pub(super) readonly: bool,
}

impl DocState {
    /// Open the document at `target` in a fresh session. Returns the state
    /// and the `doc.open` reply.
    ///
    /// # Errors
    ///
    /// `edit.missing_file` or `edit.read_failed` when the file cannot be
    /// read as UTF-8 text, or the `doc.open` error.
    pub(crate) fn open(target: Target) -> Result<(Self, Value), EditorError> {
        let mut state = Self::read(target)?;
        let reply = state.load()?;
        Ok((state, reply))
    }

    /// Read the document from disk without opening it in the engine: the
    /// session holds the text, not yet validated. [`DocState::load`] opens
    /// it. A server reads first, so it can print its URL before a large
    /// document's first validation.
    ///
    /// # Errors
    ///
    /// As [`DocState::open`], except engine errors.
    pub(crate) fn read(target: Target) -> Result<Self, EditorError> {
        let path = target.path.clone();
        let (text, stamp) = match disk::read(&path) {
            DiskRead::Text(text, stamp) => (text, stamp),
            DiskRead::Missing => {
                return Err(EditorError::new(
                    "edit.missing_file",
                    format!(
                        "cannot open '{}': the file is gone; check the path",
                        target.path.display()
                    ),
                ));
            }
            DiskRead::Unreadable(message) => {
                return Err(EditorError::new("edit.read_failed", message));
            }
        };
        Ok(DocState {
            target,
            session: Session::new(text.clone()),
            saved_text: text,
            disk: Disk::synced(Some(stamp)),
            conflict: None,
            readonly: disk::readonly(&path).unwrap_or(false),
        })
    }

    /// Open the text [`DocState::read`] read in the engine (`doc.open`:
    /// parse, validate, render state) and return the engine's reply.
    ///
    /// # Errors
    ///
    /// The engine error of `doc.open`.
    pub(crate) fn load(&mut self) -> Result<Value, EditorError> {
        let text = self.saved_text.clone();
        let outcome = self.engine_call(Request::new("doc.open", json!({ "text": text })));
        let reply = outcome.result?;
        self.session = outcome.session;
        Ok(reply)
    }

    /// The canonical document path.
    pub(crate) fn path(&self) -> &Path {
        &self.target.path
    }

    /// `true` while the session text differs from the saved text.
    pub(crate) fn dirty(&self) -> bool {
        self.session.text != self.saved_text
    }

    /// `true` while a disk change waits for `file.reload` or an
    /// overwriting `file.save`.
    pub(crate) fn conflict(&self) -> bool {
        self.conflict.is_some()
    }

    /// `true` while the file is read-only, so a save would fail.
    pub(crate) fn readonly(&self) -> bool {
        self.readonly
    }

    /// The session.
    pub(crate) fn session(&self) -> &Session {
        &self.session
    }

    /// Run `request`. `client` names the sender in the `session` event.
    ///
    /// `file.save`, `file.reload`, and `file.state` are handled here. Every
    /// other command goes to the engine. `commands.list` also lists the
    /// `file.*` commands.
    pub(crate) fn execute(&mut self, request: &Request, client: Option<&str>) -> Ran {
        match request.command.as_str() {
            "file.save" => self.save(request, client),
            "file.reload" => self.reload(request, client),
            "file.state" => Ran::reply(Ok(self.summary(false))),
            _ => self.engine(request, client),
        }
    }

    /// The session summary. `with_text` adds the session text, and the disk
    /// text while there is a conflict.
    pub(crate) fn summary(&self, with_text: bool) -> Value {
        let mut out = json!({
            "path": self.target.path,
            "root": self.target.root,
            "version": self.session.version,
            "dirty": self.dirty(),
            "valid": self.session.valid,
            "stale": self.session.stale(),
            "conflict": self.conflict.is_some(),
            "missing": self.disk.missing,
            "readonly": self.readonly,
            "page": self.session.page,
            "selection": self.session.selection,
            "mtime_ms": self.disk.stamp.map(|s| s.mtime_ms),
            "saved_sha256": hex_sha256(self.saved_text.as_bytes()),
        });
        if with_text && let Some(obj) = out.as_object_mut() {
            obj.insert("text".into(), Value::String(self.session.text.clone()));
            if let Some(c) = &self.conflict {
                obj.insert("disk_text".into(), Value::String(c.text.clone()));
            }
        }
        out
    }

    /// Run one engine request against the current session, over the
    /// confined native host.
    pub(super) fn engine_call(&self, request: Request) -> Outcome {
        let fs = ConfinedFs::new(&self.target.root);
        let host = Host {
            fs: &fs,
            ..crate::native::host()
        };
        let flags = PolicyFlags::default();
        let env = Env::new(host, Some(&self.target.dir), &flags);
        execute(&env, self.session.clone(), &request)
    }

    /// Run an engine command and record what changed.
    fn engine(&mut self, request: &Request, client: Option<&str>) -> Ran {
        let outcome = self.engine_call(request.clone());
        let mut ran = Ran {
            result: outcome.result,
            image: outcome.image,
            work: outcome.work,
            events: Vec::new(),
            change: None,
        };
        if request.command == "commands.list"
            && let Ok(reply) = &mut ran.result
        {
            super::file_cmds::append_specs(reply, self);
        }
        let before = std::mem::replace(&mut self.session, outcome.session);
        if before != self.session {
            ran.events
                .push(self.session_event(&before, client, &request.command));
            if before.text != self.session.text {
                ran.change = Some(TextChange {
                    before: before.text,
                    after: self.session.text.clone(),
                });
            }
        }
        ran
    }

    /// Replace the session text with `text` through `buffer.set` (one
    /// history entry, validated). Returns the `buffer.set` reply and the
    /// session before.
    pub(super) fn set_text(
        &mut self,
        text: &str,
        ran: &mut Ran,
    ) -> Result<(Value, Session), EditorError> {
        let request = Request::new("buffer.set", json!({ "text": text, "coalesce": false }))
            .at(self.session.version);
        let outcome = self.engine_call(request);
        add_work(&mut ran.work, outcome.work);
        let reply = outcome.result?;
        let before = std::mem::replace(&mut self.session, outcome.session);
        if before.text != self.session.text {
            ran.change = Some(TextChange {
                before: before.text.clone(),
                after: self.session.text.clone(),
            });
        }
        Ok((reply, before))
    }

    /// The `session` event for a change from `before` to the current
    /// session.
    pub(super) fn session_event(
        &self,
        before: &Session,
        client: Option<&str>,
        command: &str,
    ) -> Event {
        Event::new(
            "session",
            json!({
                "client": client,
                "command": command,
                "base_version": before.version,
                "version": self.session.version,
                "delta": delta(&before.text, &self.session.text),
                "selection": self.session.selection,
                "page": self.session.page,
                "valid": self.session.valid,
                "stale": self.session.stale(),
                "dirty": self.dirty(),
                "conflict": self.conflict.is_some(),
            }),
        )
    }

    /// Check the disk for a change made outside the session. Returns the
    /// events to send.
    ///
    /// A changed file on a clean session reloads it (`external_change` with
    /// a delta). On a dirty session it becomes a conflict
    /// (`external_change` with `conflict: true`). A removed file sends
    /// `external_change` with `deleted: true` once.
    ///
    /// With `settle`, a change acts only once two polls in a row see it, so
    /// a write in progress is never taken half done. The server's 250 ms
    /// poller settles. A caller that checks only before each command (the
    /// MCP tools) does not.
    pub(crate) fn poll_disk(&mut self, settle: bool) -> Vec<Event> {
        let mut events = self.poll_text(settle);
        if let Some(readonly) = disk::readonly(&self.target.path)
            && readonly != self.readonly
        {
            self.readonly = readonly;
            events.push(Event::new("state", self.summary(false)));
        }
        events
    }

    /// The text part of [`DocState::poll_disk`].
    fn poll_text(&mut self, settle: bool) -> Vec<Event> {
        let Some(stamp) = disk::stamp(&self.target.path) else {
            if self.disk.missing {
                return Vec::new();
            }
            if settle && self.disk.pending != Pending::Missing {
                self.disk.pending = Pending::Missing;
                return Vec::new();
            }
            self.disk = Disk {
                stamp: None,
                missing: true,
                pending: Pending::Nothing,
            };
            return vec![Event::new(
                "external_change",
                json!({
                    "path": self.target.path,
                    "deleted": true,
                    "conflict": self.conflict.is_some(),
                    "dirty": self.dirty(),
                    "version": self.session.version,
                }),
            )];
        };
        if self.disk.stamp == Some(stamp) && !self.disk.missing && !stamp.recent() {
            self.disk.pending = Pending::Nothing;
            return Vec::new();
        }
        let DiskRead::Text(text, read_stamp) = disk::read(&self.target.path) else {
            return Vec::new();
        };
        if read_stamp != stamp {
            // The file changed during the read: look again next poll.
            self.disk.pending = Pending::Changed(read_stamp);
            return Vec::new();
        }
        if text == self.saved_text {
            return self.back_to_saved(stamp).into_iter().collect();
        }
        if self.conflict.as_ref().is_some_and(|c| c.text == text) {
            self.disk = Disk::synced(Some(stamp));
            return Vec::new();
        }
        if settle && self.disk.pending != Pending::Changed(stamp) {
            self.disk.pending = Pending::Changed(stamp);
            return Vec::new();
        }
        self.disk = Disk::synced(Some(stamp));
        self.take_disk_change(text, stamp.mtime_ms)
    }

    /// The disk holds the saved text again. A conflict or the removed
    /// state ends: the event tells the clients. `None` when there was
    /// neither.
    pub(super) fn back_to_saved(&mut self, stamp: Stamp) -> Option<Event> {
        let was = self.conflict.is_some() || self.disk.missing;
        self.disk = Disk::synced(Some(stamp));
        self.conflict = None;
        was.then(|| self.external_event(None, None, stamp.mtime_ms))
    }

    /// Apply a disk text that differs from the saved text.
    pub(super) fn take_disk_change(&mut self, text: String, mtime_ms: u64) -> Vec<Event> {
        if self.session.text == text {
            self.saved_text = text;
            self.conflict = None;
            return vec![self.external_event(None, None, mtime_ms)];
        }
        if !self.dirty() {
            let mut ran = Ran::reply(Ok(Value::Null));
            if let Ok((_, before)) = self.set_text(&text, &mut ran) {
                self.saved_text = text;
                self.conflict = None;
                return vec![self.external_event(Some(&before), None, mtime_ms)];
            }
        }
        self.conflict = Some(Conflict {
            text: text.clone(),
            mtime_ms,
        });
        vec![self.external_event(None, Some(&text), mtime_ms)]
    }

    /// The `external_change` event. `before` is set when the session
    /// reloaded. `disk_text` is set on a conflict.
    fn external_event(
        &self,
        before: Option<&Session>,
        disk_text: Option<&str>,
        mtime_ms: u64,
    ) -> Event {
        let text = disk_text.unwrap_or(&self.saved_text);
        Event::new(
            "external_change",
            json!({
                "path": self.target.path,
                "deleted": false,
                "mtime_ms": mtime_ms,
                "text": text,
                "conflict": self.conflict.is_some(),
                "reloaded": before.is_some(),
                "base_version": before.map_or(self.session.version, |b| b.version),
                "version": self.session.version,
                "delta": before.and_then(|b| delta(&b.text, &self.session.text)),
                "valid": self.session.valid,
                "stale": self.session.stale(),
                "dirty": self.dirty(),
            }),
        )
    }
}

/// The wire delta from `before` to `after`, or `None` when equal.
pub(super) fn delta(before: &str, after: &str) -> Option<DeltaOut> {
    TextDelta::between(before, after).map(|d| DeltaOut::new(&d, before))
}

/// Add `more` to `work`.
pub(super) fn add_work(work: &mut Work, more: Work) {
    work.parses += more.parses;
    work.validations += more.validations;
    work.tx_runs += more.tx_runs;
    work.patches += more.patches;
    work.compiles += more.compiles;
    work.rasters += more.rasters;
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r##"zenith version=1 {
  tokens format="zenith-token-v1" {
    token id="c" type="color" value="#112233"
  }
  document id="d" {
    page id="p" w=(px)100 h=(px)100 {
      rect id="r" x=(px)10 y=(px)10 w=(px)20 h=(px)20 fill=(token)"c"
    }
  }
}
"##;

    fn open() -> (tempfile::TempDir, DocState) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("d.zen");
        std::fs::write(&path, DOC).expect("write");
        let target = Target::resolve(&path, None).expect("target");
        (dir, DocState::open(target).expect("open").0)
    }

    #[test]
    fn read_defers_validation_to_load() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("d.zen");
        std::fs::write(&path, DOC).expect("write");
        let target = Target::resolve(&path, None).expect("target");
        let mut doc = DocState::read(target).expect("read");
        assert_eq!(doc.session().text, DOC);
        assert!(!doc.session().valid, "not validated yet");
        assert!(!doc.dirty());
        doc.load().expect("load");
        assert!(doc.session().valid);
        assert!(!doc.dirty());
    }

    #[test]
    fn settled_poll_waits_for_a_second_look() {
        let (_dir, mut doc) = open();
        let changed = DOC.replace("w=(px)20", "w=(px)25");
        std::fs::write(doc.path(), &changed).expect("write");
        assert!(doc.poll_disk(true).is_empty(), "first look only marks it");
        let events = doc.poll_disk(true);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "external_change");
        assert_eq!(events[0].data["reloaded"], true);
        assert_eq!(doc.session().text, changed);
        assert!(!doc.dirty());
        assert!(doc.poll_disk(true).is_empty(), "no repeat");
    }

    #[test]
    fn unsettled_poll_takes_a_change_at_once() {
        let (_dir, mut doc) = open();
        let changed = DOC.replace("w=(px)20", "w=(px)26");
        std::fs::write(doc.path(), &changed).expect("write");
        let events = doc.poll_disk(false);
        assert_eq!(events.len(), 1);
        assert_eq!(doc.session().text, changed);
    }

    #[test]
    fn dirty_session_keeps_its_text_on_a_disk_change() {
        let (_dir, mut doc) = open();
        let v = doc.session().version;
        let request =
            Request::new("gesture.commit", json!({ "node": "r", "dx": 5, "dy": 0 })).at(v);
        assert!(doc.execute(&request, None).result.is_ok());
        let ours = doc.session().text.clone();
        let theirs = DOC.replace("h=(px)20", "h=(px)22");
        std::fs::write(doc.path(), &theirs).expect("write");
        let events = doc.poll_disk(false);
        assert_eq!(events[0].data["conflict"], true);
        assert_eq!(events[0].data["text"], theirs.as_str());
        assert_eq!(doc.session().text, ours);
        assert!(doc.conflict());
        assert!(
            doc.poll_disk(false).is_empty(),
            "the same conflict is not sent twice"
        );
    }

    #[test]
    fn conflict_ends_when_the_disk_returns_to_the_saved_text() {
        let (_dir, mut doc) = open();
        let v = doc.session().version;
        let request =
            Request::new("gesture.commit", json!({ "node": "r", "dx": 5, "dy": 0 })).at(v);
        assert!(doc.execute(&request, None).result.is_ok());
        std::fs::write(doc.path(), DOC.replace("h=(px)20", "h=(px)22")).expect("write");
        assert_eq!(doc.poll_disk(false)[0].data["conflict"], true);
        // `git checkout`: the disk holds the saved text again.
        std::fs::write(doc.path(), DOC).expect("write");
        let events = doc.poll_disk(false);
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0].name, "external_change");
        assert_eq!(events[0].data["conflict"], false);
        assert_eq!(events[0].data["reloaded"], false);
        assert!(!doc.conflict());
        assert!(doc.dirty(), "the session keeps its edits");
        let save = doc.execute(&Request::new("file.save", json!({})), None);
        assert!(save.result.is_ok(), "{:?}", save.result);
        assert!(!doc.dirty());
    }

    #[test]
    fn save_ends_a_conflict_the_disk_already_resolved() {
        let (_dir, mut doc) = open();
        let v = doc.session().version;
        let request =
            Request::new("gesture.commit", json!({ "node": "r", "dx": 5, "dy": 0 })).at(v);
        assert!(doc.execute(&request, None).result.is_ok());
        std::fs::write(doc.path(), DOC.replace("h=(px)20", "h=(px)22")).expect("write");
        assert!(!doc.poll_disk(false).is_empty());
        std::fs::write(doc.path(), DOC).expect("write");
        // No poll in between: the save itself sees the saved text.
        let save = doc.execute(&Request::new("file.save", json!({})), None);
        assert!(save.result.is_ok(), "{:?}", save.result);
        assert!(
            save.events
                .iter()
                .any(|e| e.name == "external_change" && e.data["conflict"] == false),
            "{:?}",
            save.events
        );
    }

    #[test]
    fn restored_file_ends_the_removed_state() {
        let (_dir, mut doc) = open();
        std::fs::remove_file(doc.path()).expect("remove");
        assert_eq!(doc.poll_disk(false)[0].data["deleted"], true);
        std::fs::write(doc.path(), DOC).expect("write");
        let events = doc.poll_disk(false);
        assert_eq!(events[0].data["deleted"], false, "{events:?}");
        assert_eq!(doc.summary(false)["missing"], false);
    }

    #[test]
    fn read_only_files_are_flagged_and_changes_send_state() {
        let (_dir, mut doc) = open();
        assert_eq!(doc.summary(false)["readonly"], false);
        let original = std::fs::metadata(doc.path()).expect("meta").permissions();
        let mut perms = original.clone();
        perms.set_readonly(true);
        std::fs::set_permissions(doc.path(), perms).expect("chmod");
        let events = doc.poll_disk(true);
        assert!(
            events
                .iter()
                .any(|e| e.name == "state" && e.data["readonly"] == true),
            "{events:?}"
        );
        assert!(doc.readonly());
        assert!(
            doc.poll_disk(true).iter().all(|e| e.name != "state"),
            "once"
        );
        std::fs::set_permissions(doc.path(), original).expect("chmod");
        assert!(doc.poll_disk(true).iter().any(|e| e.name == "state"));
        assert!(!doc.readonly());
    }

    #[test]
    fn removed_file_is_reported_once() {
        let (_dir, mut doc) = open();
        std::fs::remove_file(doc.path()).expect("remove");
        assert!(doc.poll_disk(true).is_empty());
        let events = doc.poll_disk(true);
        assert_eq!(events[0].data["deleted"], true);
        assert!(doc.poll_disk(true).is_empty());
        assert_eq!(doc.summary(false)["missing"], true);
    }
}
