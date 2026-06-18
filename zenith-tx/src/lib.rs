//! Transaction engine and audit contract for Zenith.
//!
//! Owns the v0 transaction op set and the apply/dry-run engine, referential-
//! integrity and ID-uniqueness enforcement, the transaction-result contract
//! (status, diagnostics, source diff, scene diff, affected node IDs, audit
//! preview and audit record), and audit record production. Both the CLI and
//! the future MCP surface sit on top of this crate; neither reimplements it.
