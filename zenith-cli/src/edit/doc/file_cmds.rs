//! The `file.*` commands: save the session to disk, reload it from disk,
//! and report the file state. They run in the host, not the engine,
//! because the engine performs no I/O.

use serde::Deserialize;
use serde_json::{Value, json};
use zenith_editor::{EditorError, Offer, Request, hex_sha256};

use super::disk::{self, DiskRead};
use super::event::Event;
use super::state::{Disk, DocState, Ran, delta};

/// History label of an editor save.
const SAVE_LABEL: &str = "editor.save";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveParams {
    #[serde(default)]
    overwrite: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyParams {}

impl DocState {
    /// `file.save {overwrite?}`: write the session text to disk.
    ///
    /// The write records history and stamps a `doc-id` on first save, as
    /// `tx --apply` does. A stamp changes the session text: the reply and the
    /// `session` event carry that delta. A disk text that changed since the
    /// last sync is a conflict: the save fails with `edit.conflict` unless
    /// `overwrite` is set.
    pub(super) fn save(&mut self, request: &Request, client: Option<&str>) -> Ran {
        let mut ran = Ran::reply(Ok(Value::Null));
        ran.result = self.save_inner(request, client, &mut ran);
        ran
    }

    fn save_inner(
        &mut self,
        request: &Request,
        client: Option<&str>,
        ran: &mut Ran,
    ) -> Result<Value, EditorError> {
        self.check_version(request, false)?;
        let p: SaveParams = params(request)?;
        if !p.overwrite {
            let on_disk = disk::read(&self.target.path);
            if let DiskRead::Text(text, stamp) = &on_disk
                && *text == self.saved_text
            {
                // The disk went back to the saved text (`git checkout`):
                // no conflict is left.
                ran.events.extend(self.back_to_saved(*stamp));
            }
            if let DiskRead::Text(text, stamp) = on_disk
                && text != self.saved_text
                && self.conflict.as_ref().is_none_or(|c| c.text != text)
            {
                self.disk = Disk::synced(Some(stamp));
                ran.events
                    .extend(self.take_disk_change(text, stamp.mtime_ms));
                if self.conflict.is_none() {
                    // The session was clean and took the disk text: nothing
                    // of the session is lost, so the save is moot.
                    return Err(EditorError::new(
                        "edit.reloaded",
                        "the file changed on disk and the clean session reloaded it; review \
                         the text and save again",
                    ));
                }
            }
            if self.conflict.is_some() {
                return Err(conflict_error());
            }
        }
        let path = self.target.path.clone();
        let written =
            crate::edit_io::write_document_io(&path, self.session.text.as_bytes(), SAVE_LABEL)
                .map_err(|e| {
                    let code = if disk::readonly(&path) == Some(true) {
                        "edit.readonly"
                    } else {
                        "edit.write_failed"
                    };
                    EditorError::new(code, crate::output_file::write_failure(&path, &e))
                })?;
        let written_text = String::from_utf8(written.bytes).map_err(|_| {
            EditorError::new(
                "edit.write_failed",
                "history returned bytes that are not UTF-8; report this as a bug",
            )
        })?;
        let stamped = written_text != self.session.text;
        let mut reply_delta = None;
        if stamped {
            let (_, before) = self.set_text(&written_text, ran)?;
            reply_delta = delta(&before.text, &self.session.text);
            self.saved_text = written_text;
            self.conflict = None;
            ran.events
                .push(self.session_event(&before, client, "file.save"));
        } else {
            self.saved_text = written_text;
            self.conflict = None;
        }
        self.disk = Disk::synced(disk::stamp(&path));
        let reply = json!({
            "saved": true,
            "path": path,
            "version": self.session.version,
            "bytes": self.saved_text.len(),
            "sha256": hex_sha256(self.saved_text.as_bytes()),
            "mtime_ms": self.disk.stamp.map(|s| s.mtime_ms),
            "stamped": stamped,
            "delta": reply_delta,
            "warning": written.warning,
            "dirty": self.dirty(),
        });
        ran.events.push(Event::new(
            "saved",
            json!({
                "client": client,
                "path": path,
                "version": self.session.version,
                "sha256": reply["sha256"],
                "mtime_ms": reply["mtime_ms"],
                "stamped": stamped,
            }),
        ));
        Ok(reply)
    }

    /// `file.reload {}` + version: replace the session text with the disk
    /// text, as one undoable history entry. Clears a conflict.
    pub(super) fn reload(&mut self, request: &Request, client: Option<&str>) -> Ran {
        let mut ran = Ran::reply(Ok(Value::Null));
        ran.result = self.reload_inner(request, client, &mut ran);
        ran
    }

    fn reload_inner(
        &mut self,
        request: &Request,
        client: Option<&str>,
        ran: &mut Ran,
    ) -> Result<Value, EditorError> {
        self.check_version(request, true)?;
        let _: EmptyParams = params(request)?;
        let (text, stamp) = match disk::read(&self.target.path) {
            DiskRead::Text(text, stamp) => (text, stamp),
            DiskRead::Missing => {
                return Err(EditorError::new(
                    "edit.missing_file",
                    format!(
                        "'{}' is gone from disk; send file.save to write the session text there",
                        self.target.path.display()
                    ),
                ));
            }
            DiskRead::Unreadable(message) => {
                return Err(EditorError::new("edit.read_failed", message));
            }
        };
        let mut reply = json!({
            "reloaded": true,
            "changed": false,
            "version": self.session.version,
            "delta": null,
            "valid": self.session.valid,
            "stale": self.session.stale(),
            "diagnostics": [],
        });
        if text != self.session.text {
            let (buffer, before) = self.set_text(&text, ran)?;
            reply = json!({
                "reloaded": true,
                "changed": true,
                "version": self.session.version,
                "delta": delta(&before.text, &self.session.text),
                "valid": self.session.valid,
                "stale": self.session.stale(),
                "diagnostics": buffer.get("diagnostics").cloned().unwrap_or(json!([])),
            });
            self.saved_text = text;
            self.conflict = None;
            self.disk = Disk::synced(Some(stamp));
            ran.events
                .push(self.session_event(&before, client, "file.reload"));
        } else {
            self.saved_text = text;
            self.conflict = None;
            self.disk = Disk::synced(Some(stamp));
        }
        Ok(reply)
    }

    /// Reject a stale `version`, and a missing one when `required`. The
    /// messages match the engine's.
    fn check_version(&self, request: &Request, required: bool) -> Result<(), EditorError> {
        match request.version {
            Some(v) if v != self.session.version => Err(EditorError::new(
                "editor.stale_version",
                format!(
                    "'{}' was made at version {v}, but the session is at version {}; apply the \
                     latest delta (or reload the text) and resend",
                    request.command, self.session.version
                ),
            )),
            None if required => Err(EditorError::new(
                "editor.missing_version",
                format!(
                    "'{}' changes the text, so it needs the session version the sender saw; add \
                     \"version\": {}",
                    request.command, self.session.version
                ),
            )),
            Some(_) | None => Ok(()),
        }
    }
}

/// `edit.conflict`, offering both resolutions.
fn conflict_error() -> EditorError {
    EditorError::new(
        "edit.conflict",
        "the file changed on disk while the session has unsaved edits; send file.reload to take \
         the disk text, or file.save with overwrite=true to replace it",
    )
    .with_offers(vec![
        Offer {
            id: "reload".into(),
            label: "Reload from disk".into(),
            command: "file.reload".into(),
            params: json!({}),
        },
        Offer {
            id: "overwrite".into(),
            label: "Overwrite the file".into(),
            command: "file.save".into(),
            params: json!({ "overwrite": true }),
        },
    ])
}

/// Decode the params of `request`. `null` reads as `{}`.
fn params<T: for<'de> Deserialize<'de>>(request: &Request) -> Result<T, EditorError> {
    let raw = match &request.params {
        Value::Null => json!({}),
        other => other.clone(),
    };
    serde_json::from_value(raw).map_err(|e| EditorError::invalid_params(&request.command, e))
}

/// Append the `file.*` command specs to a `commands.list` reply, in the
/// engine's entry shape.
pub(super) fn append_specs(reply: &mut Value, state: &DocState) {
    let Some(list) = reply.get_mut("commands").and_then(Value::as_array_mut) else {
        return;
    };
    let conflict = state.conflict.is_some();
    list.push(json!({
        "id": "file.save",
        "label": "Save to disk",
        "params": "{overwrite?=false}",
        "result": "{saved, path, version, bytes, sha256, mtime_ms, stamped, delta?, warning?, dirty}",
        "mutates": true,
        "needs_version": false,
        "enabled": true,
        "disabled": null,
    }));
    list.push(json!({
        "id": "file.reload",
        "label": "Reload from disk",
        "params": "{}",
        "result": "{reloaded, changed, version, delta?, valid, stale, diagnostics}",
        "mutates": true,
        "needs_version": true,
        "enabled": true,
        "disabled": null,
    }));
    list.push(json!({
        "id": "file.state",
        "label": "File state",
        "params": "{}",
        "result": "{path, root, version, dirty, valid, stale, conflict, missing, page, selection, mtime_ms, saved_sha256}",
        "mutates": false,
        "needs_version": false,
        "enabled": true,
        "disabled": null,
    }));
    if let Some(obj) = reply.as_object_mut() {
        obj.insert("conflict".into(), Value::Bool(conflict));
    }
}

/// Mark `state` as conflicting with `text`. Test helper for the conflict
/// paths that need no disk.
#[cfg(test)]
pub(super) fn force_conflict(state: &mut DocState, text: &str) {
    state.conflict = Some(super::state::Conflict {
        text: text.to_owned(),
        mtime_ms: 0,
    });
}

#[cfg(test)]
mod tests {
    use super::super::target::Target;
    use super::*;

    const DOC: &str = r##"zenith version=1 {
  tokens format="zenith-token-v1" {
    token id="c" type="color" value="#112233"
  }
  document id="d" {
    // The page.
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
    fn conflict_blocks_a_plain_save_and_offers_both_ways() {
        let (_dir, mut state) = open();
        // A real conflict: the disk holds other text (a disk back at the
        // saved text ends a conflict).
        let other = DOC.replace("// The page.", "// Theirs.");
        std::fs::write(state.path(), &other).expect("write");
        force_conflict(&mut state, &other);
        let ran = state.execute(&Request::new("file.save", json!({})), None);
        let err = ran.result.expect_err("conflict");
        assert_eq!(err.code, "edit.conflict");
        let ids: Vec<&str> = err.offers.iter().map(|o| o.id.as_str()).collect();
        assert_eq!(ids, ["reload", "overwrite"]);
    }

    #[test]
    fn reload_needs_a_version_and_bad_params_are_rejected() {
        let (_dir, mut state) = open();
        let ran = state.execute(&Request::new("file.reload", json!({})), None);
        assert_eq!(
            ran.result.expect_err("version").code,
            "editor.missing_version"
        );
        let ran = state.execute(&Request::new("file.save", json!({ "nope": 1 })), None);
        assert_eq!(
            ran.result.expect_err("params").code,
            "editor.invalid_params"
        );
    }

    #[test]
    fn commands_list_names_the_file_commands() {
        let (_dir, mut state) = open();
        let ran = state.execute(&Request::new("commands.list", json!({})), None);
        let reply = ran.result.expect("list");
        let ids: Vec<&str> = reply["commands"]
            .as_array()
            .expect("array")
            .iter()
            .filter_map(|c| c["id"].as_str())
            .collect();
        for id in ["file.save", "file.reload", "file.state", "gesture.commit"] {
            assert!(ids.contains(&id), "{id} missing from {ids:?}");
        }
    }
}
