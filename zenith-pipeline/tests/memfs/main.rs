//! `MemFs` host tests through the public pipeline API: example parity with
//! the disk host, a multi-file project held only in memory, and config
//! policy through a `ConfigSource`.

#[path = "../common/mod.rs"]
mod disk;
mod multi_file;
mod parity;
mod policy;
