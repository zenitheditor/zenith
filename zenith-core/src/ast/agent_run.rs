//! Agent-runs block declaration AST types.
//!
//! The top-level `agent-runs` block records the history of autonomous agent
//! executions that produced or mutated document content. Each `run` entry
//! captures a stable `id`, an optional `brief` description, optional
//! `constraints` and `plan` freeform blocks, and a sequence of `step` children
//! that describe discrete actions. It is a sibling of the
//! `variants`/`recipes`/`provenance`/`document` blocks. The engine
//! round-trips these records but does NOT act on them; auditability and
//! diffability are the sole purpose.

use std::collections::BTreeMap;

use super::Span;
use super::node::UnknownProperty;
use super::value::PropertyValue;

/// A single agent-run record within an `agent-runs` block.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentRun {
    /// The run's own stable id. Required.
    pub id: String,
    /// Short human-readable description of what this run did. Optional.
    pub brief: Option<String>,
    /// Freeform constraints text supplied to the agent. Optional.
    pub constraints: Option<String>,
    /// Freeform plan text produced or consumed by the agent. Optional.
    pub plan: Option<String>,
    /// Ordered list of discrete steps within this run.
    pub steps: Vec<AgentStep>,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
    /// Forward-compat: unrecognized attributes preserved with typed values and
    /// annotations.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
}

/// A single discrete action step within an [`AgentRun`].
#[derive(Debug, Clone, PartialEq)]
pub struct AgentStep {
    /// The step's own stable id. Required.
    pub id: String,
    /// Id of the parent step this step depends on. Optional.
    pub parent: Option<String>,
    /// The action (tool/function) invoked in this step. Required.
    pub action: String,
    /// Optional version pin for the action (e.g. `"read@2"`).
    pub action_version: Option<String>,
    /// Optional content-addressed hash of the action implementation.
    pub action_hash: Option<String>,
    /// Typed input parameters passed to the action.
    pub params: Vec<AgentStepParam>,
    /// Ids of document nodes that this step read or wrote.
    pub affected_nodes: Vec<String>,
    /// Inline diagnostics (warnings, errors) emitted during the step.
    pub diagnostics: Vec<AgentStepDiagnostic>,
    /// Content-addressed hash of the step's source/output snapshot. Optional.
    pub source_hash: Option<String>,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
    /// Forward-compat: unrecognized attributes preserved with typed values and
    /// annotations.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
}

/// A single typed input parameter within an [`AgentStep`].
#[derive(Debug, Clone, PartialEq)]
pub struct AgentStepParam {
    /// Parameter name. Required.
    pub name: String,
    /// Parameter value (number dimension, token ref, or string literal). Required.
    pub value: PropertyValue,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
    /// Forward-compat: unrecognized attributes preserved with typed values and
    /// annotations.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
}

/// An inline diagnostic emitted during an [`AgentStep`].
#[derive(Debug, Clone, PartialEq)]
pub struct AgentStepDiagnostic {
    /// Severity string (e.g. `"warn"`, `"error"`). Required.
    pub severity: String,
    /// Machine-readable diagnostic code. Required.
    pub code: String,
    /// Human-readable diagnostic message. Required.
    pub message: String,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
}
