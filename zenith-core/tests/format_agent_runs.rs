//! Integration tests for the `agent-runs` block: parse, serialize, and round-trip.
//!
//! Mirrors the recipes round-trip tests in `format_recipes.rs`. Exercises:
//! - Full parse → field access → format → re-parse → AST equality (spans stripped).
//! - Absent `agent-runs` block → empty vec, no output, byte-identical to before.
//! - Free-form string fields containing `"`, `\`, and newlines escape correctly.
//! - Unknown-prop capture on `run` and `step` nodes survives round-trip.

mod common;

use common::*;
use zenith_core::format::format_document;

// ── agent-runs: parse, serialize, and round-trip ─────────────────────────────

/// **Round-trip**: parse a doc with an `agent-runs` block (one run with
/// id + brief + constraints + plan, containing two steps: step 1 has a param,
/// two affected-nodes, a diagnostic, and a source-hash; step 2 has parent +
/// action-version + action-hash) → format → re-parse → AST equality (spans
/// stripped). Also asserts canonical position (after `recipes`, before
/// `actions`/`document`) and that all fields emit correctly.
#[test]
fn test_agent_runs_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.ar" name="AR"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  recipes {
    recipe id="recipe.x" kind="scatter" {
    }
  }
  agent-runs {
    run id="run.1" brief="Initial layout pass" {
      constraints "Keep all nodes within the safe zone."
      plan "1. Place header. 2. Place body. 3. Validate."
      step id="step.1" action="read_file" {
        affected-node "node.header"
        affected-node "node.body"
        param name="path" value="layout.zen"
        diagnostic severity="warn" code="agent.overlap" message="Two nodes overlap by 2px"
        source-hash "abc123def456"
      }
      step id="step.2" action="write_node" parent="step.1" action-version="write_node@2" action-hash="deadbeef" {
      }
    }
  }
  document id="doc.ar" title="AR" {
    page id="page.main" w=(px)1280 h=(px)720 {
      rect id="node.header" x=(px)0 y=(px)0 w=(px)1280 h=(px)80
      rect id="node.body" x=(px)0 y=(px)80 w=(px)1280 h=(px)640
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    assert_eq!(doc.agent_runs.len(), 1, "expected 1 run");

    let run = &doc.agent_runs[0];
    assert_eq!(run.id, "run.1");
    assert_eq!(run.brief.as_deref(), Some("Initial layout pass"));
    assert_eq!(
        run.constraints.as_deref(),
        Some("Keep all nodes within the safe zone.")
    );
    assert_eq!(
        run.plan.as_deref(),
        Some("1. Place header. 2. Place body. 3. Validate.")
    );
    assert_eq!(run.steps.len(), 2);

    let step1 = &run.steps[0];
    assert_eq!(step1.id, "step.1");
    assert_eq!(step1.action, "read_file");
    assert_eq!(step1.parent, None);
    assert_eq!(step1.action_version, None);
    assert_eq!(step1.action_hash, None);
    assert_eq!(step1.affected_nodes, vec!["node.header", "node.body"]);
    assert_eq!(step1.params.len(), 1);
    assert_eq!(step1.params[0].name, "path");
    assert_eq!(
        step1.params[0].value,
        zenith_core::PropertyValue::Literal("layout.zen".to_owned())
    );
    assert_eq!(step1.diagnostics.len(), 1);
    assert_eq!(step1.diagnostics[0].severity, "warn");
    assert_eq!(step1.diagnostics[0].code, "agent.overlap");
    assert_eq!(step1.diagnostics[0].message, "Two nodes overlap by 2px");
    assert_eq!(step1.source_hash.as_deref(), Some("abc123def456"));

    let step2 = &run.steps[1];
    assert_eq!(step2.id, "step.2");
    assert_eq!(step2.action, "write_node");
    assert_eq!(step2.parent.as_deref(), Some("step.1"));
    assert_eq!(step2.action_version.as_deref(), Some("write_node@2"));
    assert_eq!(step2.action_hash.as_deref(), Some("deadbeef"));
    assert!(step2.affected_nodes.is_empty());
    assert!(step2.params.is_empty());
    assert!(step2.diagnostics.is_empty());
    assert_eq!(step2.source_hash, None);

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");

    // Key fields must appear in the output.
    assert!(
        formatted_str.contains(r#"run id="run.1" brief="Initial layout pass""#),
        "run header must emit; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"constraints "Keep all nodes within the safe zone.""#),
        "constraints child must emit; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"plan "1. Place header. 2. Place body. 3. Validate.""#),
        "plan child must emit; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"step id="step.1" action="read_file""#),
        "step.1 header must emit; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"affected-node "node.header""#),
        "first affected-node must emit; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"affected-node "node.body""#),
        "second affected-node must emit; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"param name="path" value="layout.zen""#),
        "param must emit; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(
            r#"diagnostic severity="warn" code="agent.overlap" message="Two nodes overlap by 2px""#
        ),
        "diagnostic must emit; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"source-hash "abc123def456""#),
        "source-hash must emit; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(
            r#"step id="step.2" action="write_node" parent="step.1" action-version="write_node@2" action-hash="deadbeef""#
        ),
        "step.2 header must emit with parent/version/hash; got:\n{formatted_str}"
    );

    // Canonical order: recipes, then agent-runs, then document.
    let recipes_at = formatted_str.find("recipes {").expect("recipes block");
    let agent_runs_at = formatted_str
        .find("agent-runs {")
        .expect("agent-runs block");
    let doc_at = formatted_str.find("document ").expect("document block");
    assert!(
        recipes_at < agent_runs_at && agent_runs_at < doc_at,
        "agent-runs must be emitted after recipes and before document; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");
    assert_eq!(
        strip_spans(doc).agent_runs,
        strip_spans(reparsed).agent_runs,
        "agent-runs must survive a parse → format → parse round-trip (idempotent)"
    );
}

/// **Absent `agent-runs` block is an empty vec**: a document with no
/// `agent-runs` block must parse with `doc.agent_runs` empty, produce no
/// `agent-runs { … }` output, and be byte-identical across two format passes.
#[test]
fn test_absent_agent_runs_is_empty_and_byte_identical() {
    let src = r##"zenith version=1 {
  project id="proj.noar" name="NoAR"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.noar" title="NoAR" {
    page id="p" w=(px)640 h=(px)360 {
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    assert!(
        doc.agent_runs.is_empty(),
        "absent agent-runs block must yield an empty vec"
    );

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");

    assert!(
        !formatted_str.contains("agent-runs"),
        "no agent-runs block must be emitted for an empty vec; got:\n{formatted_str}"
    );

    // Idempotency: output is byte-identical on a second pass.
    let reparsed = adapter.parse(&formatted).expect("re-parse");
    let formatted2 = format_document(&reparsed).expect("format 2");
    assert_eq!(
        formatted, formatted2,
        "absent agent-runs must be byte-identical across two format passes"
    );
}

/// **Free-form escaping round-trip**: `brief`, `constraints`, `plan`,
/// `source-hash`, and a diagnostic `message` each containing embedded `"` and
/// `\n` survive parse → format → parse with the exact same string value.
#[test]
fn test_agent_run_free_form_escaping_round_trip() {
    let tricky = "first line\nsecond \"quoted\" line\\backslash";
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.esc2" name="ESC2"
  tokens format="zenith-token-v1" {{
  }}
  styles {{
  }}
  agent-runs {{
    run id="run.esc" brief={brief:?} {{
      constraints {constraints:?}
      plan {plan:?}
      step id="step.esc" action="tool" {{
        diagnostic severity="error" code="x.y" message={message:?}
        source-hash {source_hash:?}
      }}
    }}
  }}
  document id="doc.esc2" title="ESC2" {{
    page id="p" w=(px)640 h=(px)360 {{
    }}
  }}
}}
"##,
        brief = tricky,
        constraints = tricky,
        plan = tricky,
        message = tricky,
        source_hash = tricky,
    );

    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    let run = &doc.agent_runs[0];
    assert_eq!(
        run.brief.as_deref(),
        Some(tricky),
        "brief must parse back exactly"
    );
    assert_eq!(
        run.constraints.as_deref(),
        Some(tricky),
        "constraints must parse back exactly"
    );
    assert_eq!(
        run.plan.as_deref(),
        Some(tricky),
        "plan must parse back exactly"
    );
    assert_eq!(
        run.steps[0].diagnostics[0].message, tricky,
        "diagnostic message must parse back exactly"
    );
    assert_eq!(
        run.steps[0].source_hash.as_deref(),
        Some(tricky),
        "source-hash must parse back exactly"
    );

    let formatted = format_document(&doc).expect("format");
    let reparsed = adapter.parse(&formatted).expect("re-parse escaped output");

    assert_eq!(
        reparsed.agent_runs[0].brief.as_deref(),
        Some(tricky),
        "brief with special chars must survive round-trip"
    );
    assert_eq!(
        strip_spans(doc).agent_runs,
        strip_spans(reparsed).agent_runs,
        "escaped agent-runs must be round-trip identical"
    );
}

/// **Unknown-prop round-trip**: an unrecognized annotated prop on `run` and on
/// `step` is captured in `unknown_props` and survives parse → format → parse
/// byte-identically.
#[test]
fn test_agent_run_unknown_props_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.uknar" name="UKNAR"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  agent-runs {
    run id="run.ukn" priority=(token)"fmt.token" {
      step id="step.ukn" action="tool" weight=(px)2 {
      }
    }
  }
  document id="doc.uknar" title="UKNAR" {
    page id="p" w=(px)640 h=(px)360 {
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    assert_eq!(doc.agent_runs.len(), 1);
    let run = &doc.agent_runs[0];

    let priority_prop = run
        .unknown_props
        .get("priority")
        .expect("annotated unknown prop `priority` must be captured on run");
    assert_eq!(
        priority_prop.ty.as_deref(),
        Some("token"),
        "annotation on run unknown prop must survive"
    );

    assert_eq!(run.steps.len(), 1);
    let step = &run.steps[0];
    let weight_prop = step
        .unknown_props
        .get("weight")
        .expect("annotated unknown prop `weight` must be captured on step");
    assert_eq!(
        weight_prop.ty.as_deref(),
        Some("px"),
        "annotation on step unknown prop must survive"
    );

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");

    assert!(
        formatted_str.contains(r#"priority=(token)"fmt.token""#),
        "annotated unknown prop on run must round-trip; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains("weight=(px)2"),
        "annotated unknown prop on step must round-trip; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");
    assert_eq!(
        strip_spans(doc).agent_runs,
        strip_spans(reparsed).agent_runs,
        "agent-runs with unknown props must survive full round-trip"
    );
}
