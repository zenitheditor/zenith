const DB_NAME = "zenith-editor-drafts";
const STORE = "drafts";
const VERSION = 1;
const MAX_DRAFTS = 12;
const MAX_DRAFT_BYTES = 1024 * 1024;
const MAX_TOTAL_BYTES = 4 * MAX_DRAFT_BYTES;
// IndexedDB stores strings as UTF-16. Count source and record metadata.
const size = (record) => JSON.stringify(record).length * 2;

export function draftId() {
  return Array.from(crypto.getRandomValues(new Uint8Array(16)), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function supported(record) {
  return record?.schema === VERSION && typeof record.id === "string"
    && typeof record.name === "string" && typeof record.text === "string"
    && Number.isSafeInteger(record.revision) && record.revision > 0
    && Number.isFinite(record.updatedAt)
    && ["starter", "blank", "device"].includes(record.source?.kind)
    && (record.source.kind !== "starter"
      || (typeof record.source.starter === "string" && typeof record.source.digest === "string"));
}

const unsupported = () => new Error("Browser drafts contain an unreadable record. Download your current .zen file.");

export class DraftStore {
  constructor() {
    this.connection = null;
  }

  async database() {
    if (this.connection) return this.connection;
    const opening = new Promise((resolve, reject) => {
      const request = indexedDB.open(DB_NAME, VERSION);
      let blocked = false;
      request.onupgradeneeded = () => request.result.createObjectStore(STORE, { keyPath: "id" });
      request.onerror = () => reject(request.error);
      request.onblocked = () => {
        blocked = true;
        reject(new Error("Close older Zenith tabs, then try browser drafts again."));
      };
      request.onsuccess = () => {
        const db = request.result;
        if (blocked) {
          db.close();
          return;
        }
        db.onversionchange = () => {
          this.connection = null;
          db.close();
        };
        resolve(db);
      };
    });
    this.connection = opening;
    try {
      return await opening;
    } catch (error) {
      if (this.connection === opening) this.connection = null;
      throw error;
    }
  }

  async transaction(mode, query, apply) {
    const db = await this.database();
    return new Promise((resolve, reject) => {
      const tx = db.transaction(STORE, mode);
      const store = tx.objectStore(STORE);
      const request = query(store);
      let result;
      let error;
      request.onsuccess = () => {
        try {
          result = apply(store, request.result);
        } catch (cause) {
          error = cause;
          tx.abort();
        }
      };
      tx.oncomplete = () => resolve(result);
      tx.onabort = () => reject(error ?? tx.error ?? new Error("Browser storage transaction was interrupted."));
    });
  }

  list() {
    return this.transaction("readonly", (store) => store.getAll(), (_, records) => ({
      records: records.filter(supported).sort((a, b) => b.updatedAt - a.updatedAt),
      unreadable: records.some((record) => !supported(record)),
    }));
  }

  read(id) {
    return this.transaction("readonly", (store) => store.get(id), (_, record) => {
      if (record && !supported(record)) throw unsupported();
      return record ?? null;
    });
  }

  save({ id, name, text, source }, expected) {
    return this.transaction("readwrite", (store) => store.getAll(), (store, records) => {
      const prior = records.find((record) => record.id === id);
      if (prior && !supported(prior)) throw unsupported();
      if (expected > 0 && !prior) return null;
      const fork = prior && prior.revision !== expected;
      const replacing = prior && !fork;
      const record = {
        schema: VERSION,
        id: fork ? draftId() : id,
        revision: replacing ? prior.revision + 1 : 1,
        name,
        text,
        source,
        updatedAt: Date.now(),
      };
      const bytes = size(record);
      if (bytes > MAX_DRAFT_BYTES) {
        throw new Error("This draft exceeds the browser draft size limit. Download its .zen file.");
      }
      const remaining = records.filter((stored) => !replacing || stored.id !== id);
      if (remaining.length >= MAX_DRAFTS || bytes + remaining.reduce((sum, stored) => sum + size(stored), 0) > MAX_TOTAL_BYTES) {
        throw new Error("Browser draft storage is full. Download your .zen file or discard a draft.");
      }
      if (replacing) store.put(record);
      else store.add(record);
      return record;
    });
  }

  discard(id, expected) {
    return this.transaction("readwrite", (store) => store.get(id), (store, prior) => {
      if (!prior) return "missing";
      if (!supported(prior)) throw unsupported();
      if (prior.revision !== expected) return "changed";
      store.delete(id);
      return "discarded";
    });
  }
}
