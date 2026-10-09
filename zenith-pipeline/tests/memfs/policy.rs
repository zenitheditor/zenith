//! Config policy through a `ConfigSource` over `MemFs`: a local
//! `.zenith.kdl` and a global config govern validate and render exactly as
//! the CLI's config files do.

use std::path::{Path, PathBuf};

use zenith_core::Severity;
use zenith_pipeline::render::render_png;
use zenith_pipeline::{FsConfig, Host, MemFs, PolicyFlags, RenderOptions, validate_source};

/// One unused token: `token.unused`, an advisory by default.
const DOC: &str = r##"zenith version=1 {
  project id="proj.p" name="Policy"
  tokens format="zenith-token-v1" {
    token id="color.unused" type="color" value="#abcdef"
  }
  styles {}
  document id="doc.p" title="Policy" {
    page id="page.p" w=(px)100 h=(px)100 {
      rect id="r.one" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
  }
}
"##;

fn severity_of(diagnostics: &[zenith_core::Diagnostic], code: &str) -> Option<Severity> {
    diagnostics
        .iter()
        .find(|d| d.code == code)
        .map(|d| d.severity)
}

#[test]
fn local_deny_blocks_render_and_fails_validate() {
    let fs = MemFs::new().with(
        "p/.zenith.kdl",
        b"diagnostics {\n  deny \"token.unused\"\n}\n".to_vec(),
    );
    let config = FsConfig::new(&fs, None);
    let host = Host::new(&fs, &config);
    let dir = Some(Path::new("p/docs"));
    let flags = PolicyFlags::default();

    let validation = validate_source(host, DOC, dir, &flags);
    assert_eq!(validation.exit_code, 1);
    assert_eq!(
        severity_of(&validation.diagnostics, "token.unused"),
        Some(Severity::Error)
    );

    let err = render_png(host, DOC, dir, 1, RenderOptions::new(&flags)).expect_err("blocked");
    assert_eq!(err.exit_code, 1);
    assert!(
        err.message.contains("error[token.unused]"),
        "{}",
        err.message
    );
}

#[test]
fn global_warn_relabels_and_flags_override() {
    let fs = MemFs::new().with(
        "home/config.kdl",
        b"diagnostics {\n  warn \"token.unused\"\n}\n".to_vec(),
    );
    let config = FsConfig::new(&fs, Some(PathBuf::from("home/config.kdl")));
    let host = Host::new(&fs, &config);
    let flags = PolicyFlags::default();

    let artifact = render_png(host, DOC, None, 1, RenderOptions::new(&flags)).expect("renders");
    assert_eq!(
        severity_of(&artifact.diagnostics, "token.unused"),
        Some(Severity::Warning)
    );

    let allow = PolicyFlags {
        allow: vec!["token.unused".to_owned()],
        ..Default::default()
    };
    let validation = validate_source(host, DOC, None, &allow);
    assert_eq!(severity_of(&validation.diagnostics, "token.unused"), None);
}

#[test]
fn malformed_config_is_a_config_error() {
    let fs = MemFs::new().with("p/.zenith.kdl", b"diagnostics {{{ not kdl".to_vec());
    let config = FsConfig::new(&fs, None);
    let host = Host::new(&fs, &config);
    let validation = validate_source(host, DOC, Some(Path::new("p")), &PolicyFlags::default());
    assert_eq!(validation.exit_code, 2);
    assert_eq!(validation.diagnostics[0].code, "config.error");
}
