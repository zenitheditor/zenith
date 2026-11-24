// Lightweight IndexedDB store for workspace conveniences that stay separate
// from `draft-store.js` (so draft record schemas stay strictly unchanged):
//   - `handles`: remembered `FileSystemDirectoryHandle` for 1-click reconnect
//   - `thumbs`: cached small preview data URLs keyed by draft ID

const DB_NAME = "zenith-editor-workspace";
const VERSION = 1;
const HANDLES = "handles";
const THUMBS = "thumbs";

let dbPromise = null;

function openDb() {
  if (dbPromise) return dbPromise;
  dbPromise = new Promise((resolve, reject) => {
    try {
      const req = indexedDB.open(DB_NAME, VERSION);
      req.onupgradeneeded = () => {
        const db = req.result;
        if (!db.objectStoreNames.contains(HANDLES)) db.createObjectStore(HANDLES);
        if (!db.objectStoreNames.contains(THUMBS)) db.createObjectStore(THUMBS);
      };
      req.onerror = () => reject(req.error);
      req.onsuccess = () => resolve(req.result);
    } catch (err) {
      reject(err);
    }
  }).catch(() => {
    dbPromise = null;
    return null;
  });
  return dbPromise;
}

async function runTx(storeName, mode, fn) {
  const db = await openDb();
  if (!db) return null;
  return new Promise((resolve) => {
    try {
      const tx = db.transaction(storeName, mode);
      const req = fn(tx.objectStore(storeName));
      tx.oncomplete = () => resolve(req?.result ?? true);
      tx.onabort = () => resolve(null);
      tx.onerror = () => resolve(null);
    } catch {
      resolve(null);
    }
  });
}

export async function saveFolderHandle(handle) {
  if (!handle) return;
  await runTx(HANDLES, "readwrite", (s) => s.put(handle, "last-folder"));
}

export async function loadFolderHandle() {
  const h = await runTx(HANDLES, "readonly", (s) => s.get("last-folder"));
  return h && typeof h.name === "string" && typeof h.values === "function" ? h : null;
}

export async function clearFolderHandle() {
  await runTx(HANDLES, "readwrite", (s) => s.delete("last-folder"));
}

export async function saveThumb(id, dataUrl) {
  if (!id || !dataUrl) return;
  await runTx(THUMBS, "readwrite", (s) => s.put(dataUrl, id));
}

export async function loadThumb(id) {
  if (!id) return null;
  const v = await runTx(THUMBS, "readonly", (s) => s.get(id));
  return typeof v === "string" ? v : null;
}

export async function deleteThumb(id) {
  if (!id) return;
  await runTx(THUMBS, "readwrite", (s) => s.delete(id));
}
