// Browser file access of the static host: pick a document or a project
// folder, read and write it, and the fallbacks for browsers without the
// File System Access API (Chromium only). The fallbacks use `<input
// type=file>` to open and a download to save.
//
// A handle is a `FileSystemFileHandle`. Nothing here keeps state.

/** A file larger than this is not read into the project. */
export const MAX_FILE_BYTES = 64 * 1024 * 1024;
/** The project stops growing past this many bytes of files. */
export const MAX_PROJECT_BYTES = 256 * 1024 * 1024;

const SKIP_DIRS = new Set(["node_modules", "target", "dist"]);
/** The one dotfile a project needs: the local config the pipeline reads. */
const CONFIG_FILE = ".zenith.kdl";

/** `true` for a dot file or directory the project leaves out. */
const hidden = (name) => name.startsWith(".") && name !== CONFIG_FILE;

/** `true` when the browser can pick a file and write it back. */
export const canPickFile = () => typeof window.showOpenFilePicker === "function";

/** `true` when the browser can pick a folder and write into it. */
export const canPickDirectory = () => typeof window.showDirectoryPicker === "function";

/** `true` when `err` is the user dismissing a picker. */
export const isAbort = (err) => err && err.name === "AbortError";

/** The base64 text of `blob`. */
export function blobToBase64(blob) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error ?? new Error("cannot read the file"));
    reader.onload = () => {
      const url = String(reader.result);
      resolve(url.slice(url.indexOf(",") + 1));
    };
    reader.readAsDataURL(blob);
  });
}

/** Ask the user for files with a hidden `<input type=file>`. Resolves `[]` on cancel. */
function chooseWithInput({ accept = "", multiple = false, directory = false } = {}) {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = accept;
    input.multiple = multiple;
    if (directory) input.webkitdirectory = true;
    input.hidden = true;
    input.dataset.zenithPicker = directory ? "folder" : "file";
    const done = (files) => {
      input.remove();
      resolve(files);
    };
    input.addEventListener("change", () => done([...input.files]), { once: true });
    input.addEventListener("cancel", () => done([]), { once: true });
    document.body.appendChild(input);
    input.click();
  });
}

/**
 * Pick one `.zen` document. Resolves `{name, text, handle}`, with `handle`
 * `null` when the browser has no File System Access API, or `null` when
 * the user cancelled.
 */
export async function pickDocument() {
  if (canPickFile()) {
    try {
      const [handle] = await window.showOpenFilePicker({
        multiple: false,
        types: [{ description: "Zenith design", accept: { "text/plain": [".zen"] } }],
      });
      const file = await handle.getFile();
      return { name: file.name, text: await file.text(), handle };
    } catch (err) {
      if (isAbort(err)) return null;
      throw err;
    }
  }
  const [file] = await chooseWithInput({ accept: ".zen,text/plain" });
  if (!file) return null;
  return { name: file.name, text: await file.text(), handle: null };
}

/**
 * Pick a project folder and read it. Resolves `{root, files, documents,
 * handles, skipped}` or `null` when the user cancelled. `files` maps a path
 * relative to the folder to base64 text, `documents` lists the `.zen`
 * paths, `handles` maps a path to its file handle (File System Access
 * only), `skipped` lists files left out (too large, or over the project
 * size).
 */
export async function pickProject() {
  if (canPickDirectory()) {
    let dir;
    try {
      dir = await window.showDirectoryPicker({ mode: "readwrite" });
    } catch (err) {
      if (isAbort(err)) return null;
      throw err;
    }
    const entries = [];
    await walkDirectory(dir, "", entries);
    return readEntries(dir.name, entries, true);
  }
  const files = await chooseWithInput({ directory: true });
  if (!files.length) return null;
  const root = (files[0].webkitRelativePath || "").split("/")[0] || "project";
  const entries = files.map((file) => ({
    path: (file.webkitRelativePath || file.name).split("/").slice(1).join("/") || file.name,
    file,
    handle: null,
  }));
  return readEntries(root, entries, false);
}

/** Collect `{path, handle}` for every file under `dir`. */
async function walkDirectory(dir, prefix, out) {
  for await (const entry of dir.values()) {
    if (hidden(entry.name)) continue;
    const path = prefix + entry.name;
    if (entry.kind === "directory") {
      if (!SKIP_DIRS.has(entry.name)) await walkDirectory(entry, `${path}/`, out);
    } else {
      out.push({ path, handle: entry, file: null });
    }
  }
}

async function readEntries(root, entries, writable) {
  const files = {};
  const handles = new Map();
  const skipped = [];
  let total = 0;
  entries.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  for (const entry of entries) {
    if (entry.path.split("/").some(hidden)) continue;
    const file = entry.file ?? (await entry.handle.getFile());
    if (file.size > MAX_FILE_BYTES || total + file.size > MAX_PROJECT_BYTES) {
      skipped.push(entry.path);
      continue;
    }
    total += file.size;
    files[entry.path] = await blobToBase64(file);
    if (writable && entry.handle) handles.set(entry.path, entry.handle);
  }
  const documents = Object.keys(files).filter((p) => p.endsWith(".zen"));
  return { root, files, documents, handles, skipped, bytes: total };
}

/** The text of the file behind `handle`, and its modification time. */
export async function readHandle(handle) {
  const file = await handle.getFile();
  return { text: await file.text(), mtimeMs: file.lastModified };
}

/**
 * Write `text` to `handle`. Resolves the new modification time. A failed
 * write aborts the stream, so the file keeps its old bytes, and rejects
 * with the write error (an error of the abort never hides it).
 */
export async function writeHandle(handle, text) {
  const writable = await handle.createWritable();
  try {
    await writable.write(text);
  } catch (err) {
    if (typeof writable.abort === "function") {
      try {
        await writable.abort(err);
      } catch {
        // The write error is the one to report.
      }
    }
    throw err;
  }
  await writable.close();
  return (await handle.getFile()).lastModified;
}

/**
 * Ask where to save a new document and write it. Resolves the handle, or
 * `null` when the user cancelled or the browser cannot pick a save target.
 */
export async function saveAs(name, text) {
  if (typeof window.showSaveFilePicker !== "function") return null;
  try {
    const handle = await window.showSaveFilePicker({
      suggestedName: name,
      types: [{ description: "Zenith design", accept: { "text/plain": [".zen"] } }],
    });
    await writeHandle(handle, text);
    return handle;
  } catch (err) {
    if (isAbort(err)) return null;
    throw err;
  }
}

/**
 * `true` when `err` says the file cannot be written: a read-only file
 * (`NoModificationAllowedError`) or a write permission the user or the
 * system refused (`NotAllowedError`).
 */
export function isReadOnlyError(err) {
  return err?.name === "NoModificationAllowedError" || err?.name === "NotAllowedError";
}

/** Offer `text` as a download named `name`. */
export function download(name, text) {
  const url = URL.createObjectURL(new Blob([text], { type: "text/plain;charset=utf-8" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.hidden = true;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10000);
}
