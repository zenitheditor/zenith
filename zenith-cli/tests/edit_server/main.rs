//! Integration tests for `zenith edit`: the agent flow, live events, the
//! conflict policy, every security defense, and the server lifecycle. Each
//! test runs the real binary on an ephemeral port with its own history
//! store.

mod agent_flow;
mod events;
mod lifecycle;
mod security;
mod support;
