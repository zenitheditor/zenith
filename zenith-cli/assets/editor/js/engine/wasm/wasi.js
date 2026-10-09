// A minimal wasi_snapshot_preview1 host for the zenith-editor-wasm command
// module. A static host runs the engine in a Worker through it (see
// `worker.js`). `zenith edit` uses the native engine and never loads it.
//
// It implements exactly the 13 imports the module declares. stdin is one
// request buffer, stdout and stderr are captured. There is no filesystem,
// no preopen, and no environment. One instance runs one request.

const ERRNO_SUCCESS = 0;
const ERRNO_BADF = 8;
const ERRNO_NOTCAPABLE = 76;
const FILETYPE_CHARACTER_DEVICE = 2;

export class WasiExit extends Error {
  constructor(code) {
    super(`wasi proc_exit(${code})`);
    this.code = code;
  }
}

export class Wasi {
  constructor(stdinBytes) {
    this.stdin = stdinBytes;
    this.stdinPos = 0;
    this.stdout = [];
    this.stderr = [];
    this.memory = null;
  }

  view() {
    return new DataView(this.memory.buffer);
  }

  bytes() {
    return new Uint8Array(this.memory.buffer);
  }

  imports() {
    const self = this;
    const isStdio = (fd) => fd === 0 || fd === 1 || fd === 2;
    return {
      wasi_snapshot_preview1: {
        random_get(buf, len) {
          const out = self.bytes().subarray(buf, buf + len);
          for (let i = 0; i < len; i += 65536) {
            crypto.getRandomValues(out.subarray(i, Math.min(i + 65536, len)));
          }
          return ERRNO_SUCCESS;
        },
        environ_sizes_get(countPtr, sizePtr) {
          const v = self.view();
          v.setUint32(countPtr, 0, true);
          v.setUint32(sizePtr, 0, true);
          return ERRNO_SUCCESS;
        },
        environ_get() {
          return ERRNO_SUCCESS;
        },
        fd_close(fd) {
          return isStdio(fd) ? ERRNO_SUCCESS : ERRNO_BADF;
        },
        fd_fdstat_get(fd, buf) {
          if (!isStdio(fd)) return ERRNO_BADF;
          const v = self.view();
          for (let i = 0; i < 24; i++) v.setUint8(buf + i, 0);
          v.setUint8(buf, FILETYPE_CHARACTER_DEVICE);
          v.setBigUint64(buf + 8, 0xffffffffffffffffn, true);
          v.setBigUint64(buf + 16, 0xffffffffffffffffn, true);
          return ERRNO_SUCCESS;
        },
        fd_filestat_get(fd, buf) {
          if (!isStdio(fd)) return ERRNO_BADF;
          const v = self.view();
          for (let i = 0; i < 64; i++) v.setUint8(buf + i, 0);
          v.setUint8(buf + 16, FILETYPE_CHARACTER_DEVICE);
          return ERRNO_SUCCESS;
        },
        fd_prestat_get() {
          // No preopened directories.
          return ERRNO_BADF;
        },
        fd_prestat_dir_name() {
          return ERRNO_BADF;
        },
        fd_read(fd, iovs, iovsLen, nreadPtr) {
          if (fd !== 0) return ERRNO_BADF;
          const v = self.view();
          const mem = self.bytes();
          let total = 0;
          for (let i = 0; i < iovsLen; i++) {
            const ptr = v.getUint32(iovs + i * 8, true);
            const len = v.getUint32(iovs + i * 8 + 4, true);
            const n = Math.min(len, self.stdin.length - self.stdinPos);
            mem.set(self.stdin.subarray(self.stdinPos, self.stdinPos + n), ptr);
            self.stdinPos += n;
            total += n;
            if (n < len) break;
          }
          v.setUint32(nreadPtr, total, true);
          return ERRNO_SUCCESS;
        },
        fd_write(fd, iovs, iovsLen, nwrittenPtr) {
          if (fd !== 1 && fd !== 2) return ERRNO_BADF;
          const sink = fd === 1 ? self.stdout : self.stderr;
          const v = self.view();
          const mem = self.bytes();
          let total = 0;
          for (let i = 0; i < iovsLen; i++) {
            const ptr = v.getUint32(iovs + i * 8, true);
            const len = v.getUint32(iovs + i * 8 + 4, true);
            sink.push(mem.slice(ptr, ptr + len));
            total += len;
          }
          v.setUint32(nwrittenPtr, total, true);
          return ERRNO_SUCCESS;
        },
        path_filestat_get() {
          // No filesystem.
          return ERRNO_NOTCAPABLE;
        },
        path_open() {
          return ERRNO_NOTCAPABLE;
        },
        proc_exit(code) {
          throw new WasiExit(code);
        },
      },
    };
  }
}

function concat(chunks) {
  const len = chunks.reduce((n, c) => n + c.length, 0);
  const out = new Uint8Array(len);
  let off = 0;
  for (const c of chunks) {
    out.set(c, off);
    off += c.length;
  }
  return out;
}

/** Names of the imports `module` needs that this shim does not provide. */
export function missingImports(module) {
  const provided = new Wasi(new Uint8Array()).imports();
  return WebAssembly.Module.imports(module)
    .filter((i) => !(provided[i.module] && i.name in provided[i.module]))
    .map((i) => `${i.module}.${i.name}`);
}

/**
 * Run one request on a fresh instance of `module`. Returns the response
 * text, the stderr text, the exit code, and the instantiate and run times
 * in ms.
 */
export async function runRequest(module, requestText) {
  const wasi = new Wasi(new TextEncoder().encode(requestText));
  const t0 = performance.now();
  const instance = await WebAssembly.instantiate(module, wasi.imports());
  wasi.memory = instance.exports.memory;
  const t1 = performance.now();
  let exitCode = 0;
  try {
    instance.exports._start();
  } catch (e) {
    if (e instanceof WasiExit) exitCode = e.code;
    else throw e;
  }
  const t2 = performance.now();
  const dec = new TextDecoder();
  return {
    response: dec.decode(concat(wasi.stdout)),
    stderr: dec.decode(concat(wasi.stderr)),
    exitCode,
    instantiateMs: t1 - t0,
    runMs: t2 - t1,
  };
}
