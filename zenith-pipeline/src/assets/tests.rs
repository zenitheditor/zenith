use std::path::Path;

use zenith_core::{
    AssetProvider, Document, FontProvider, FontSource, FontStyle, KdlAdapter, KdlSource,
};

use super::*;
use crate::imports::load_import_graph;
use crate::io::NoConfig;
use crate::{CollectWarnings, Host, MemFs};

const NOTO: &[u8] = include_bytes!("../../../zenith-core/assets/fonts/NotoSans-Regular.ttf");

fn parse(src: &str) -> Document {
    KdlAdapter
        .parse(src.as_bytes())
        .expect("test document must parse")
}

const HOST_IMPORTING_BRAND: &str = r#"zenith version=1 {
  project id="proj.host" name="Host"
  imports {
    import id="brand" kind="zen" src="brand/brand.zen"
  }
  document id="doc.host" title="Host" {
    page id="page.host" w=(px)10 h=(px)10
  }
}
"#;

#[test]
fn build_asset_provider_registers_imported_assets_from_import_directory() {
    let fs = MemFs::new()
        .with("p/brand/logo.bin", b"imported-logo".to_vec())
        .with(
            "p/brand/brand.zen",
            r#"zenith version=1 {
  project id="proj.brand" name="Brand"
  assets {
    asset id="logo" kind="image" src="logo.bin"
  }
  document id="doc.brand" title="Brand" {
    page id="page.brand" w=(px)10 h=(px)10
  }
}
"#,
        );
    let root = parse(HOST_IMPORTING_BRAND);
    let imports = load_import_graph(&fs, &root, Some(Path::new("p")));

    let provider = build_asset_provider_with_imports(&fs, &root, Path::new("p"), &imports, false)
        .expect("provider");

    let asset = provider
        .by_id("brand/logo")
        .expect("imported asset must be registered under import namespace");
    assert_eq!(&asset.bytes[..], b"imported-logo");
}

#[test]
fn collect_missing_import_asset_diagnostics_reports_missing_imported_asset() {
    // The imported document declares an asset whose file is absent.
    let fs = MemFs::new().with(
        "p/brand/brand.zen",
        r#"zenith version=1 {
  project id="proj.brand" name="Brand"
  assets {
    asset id="logo" kind="image" src="missing.png"
  }
  document id="doc.brand" title="Brand" {
    page id="page.brand" w=(px)10 h=(px)10
  }
}
"#,
    );
    let root = parse(HOST_IMPORTING_BRAND);
    let imports = load_import_graph(&fs, &root, Some(Path::new("p")));

    let diagnostics = collect_missing_import_asset_diagnostics(&fs, &imports);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "import.asset_missing");
    assert_eq!(diagnostics[0].subject_id.as_deref(), Some("logo"));
    assert_eq!(
        diagnostics[0].message,
        "import 'brand' asset 'logo' file not found: 'p/brand/missing.png'"
    );
}

#[test]
fn build_font_provider_registers_imported_font_under_import_namespace() {
    // An imported font whose real family is "Noto Sans" registers as
    // "brand/Noto Sans", so the plain family still resolves to the bundled
    // face while the namespaced family resolves to the imported face.
    let fs = MemFs::new().with("p/brand/brand.ttf", NOTO).with(
        "p/brand/brand.zen",
        r#"zenith version=1 {
  project id="proj.brand" name="Brand"
  assets {
    asset id="font.brand" kind="font" src="brand.ttf"
  }
  tokens format="zenith-token-v1" {
    token id="font.brand" type="fontFamily" value="Noto Sans"
  }
  document id="doc.brand" title="Brand" {
    page id="page.brand" w=(px)10 h=(px)10
  }
}
"#,
    );
    let root = parse(HOST_IMPORTING_BRAND);
    let dir = Path::new("p");
    let imports = load_import_graph(&fs, &root, Some(dir));

    let provider = build_font_provider_with_imports(
        Host::new(&fs, &NoConfig),
        &root,
        Some(dir),
        &imports,
        false,
    )
    .expect("provider");

    let plain = provider
        .resolve(&["Noto Sans".to_owned()], 400, FontStyle::Normal)
        .expect("plain family resolves to bundled");
    assert_eq!(plain.source, FontSource::Bundled);

    let namespaced = provider
        .resolve(&["brand/Noto Sans".to_owned()], 400, FontStyle::Normal)
        .expect("namespaced imported font asset must resolve");
    assert_eq!(namespaced.source, FontSource::Project);
    assert!(namespaced.id.starts_with("brand/"), "{}", namespaced.id);
}

#[test]
fn host_and_imported_same_family_do_not_shadow_each_other() {
    // Host and import each declare their own font asset with the same real
    // family. Both register as distinct faces.
    let fs = MemFs::new()
        .with("p/host.ttf", NOTO)
        .with("p/brand/brand.ttf", NOTO)
        .with(
            "p/brand/brand.zen",
            r#"zenith version=1 {
  project id="proj.brand" name="Brand"
  assets {
    asset id="font.brand" kind="font" src="brand.ttf"
  }
  document id="doc.brand" title="Brand" {
    page id="page.brand" w=(px)10 h=(px)10
  }
}
"#,
        );
    let root = parse(
        r#"zenith version=1 {
  project id="proj.host" name="Host"
  imports {
    import id="brand" kind="zen" src="brand/brand.zen"
  }
  assets {
    asset id="font.host" kind="font" src="host.ttf"
  }
  document id="doc.host" title="Host" {
    page id="page.host" w=(px)10 h=(px)10
  }
}
"#,
    );
    let dir = Path::new("p");
    let imports = load_import_graph(&fs, &root, Some(dir));

    let provider = build_font_provider_with_imports(
        Host::new(&fs, &NoConfig),
        &root,
        Some(dir),
        &imports,
        false,
    )
    .expect("provider");

    let host = provider
        .resolve(&["Noto Sans".to_owned()], 400, FontStyle::Normal)
        .expect("host family resolves");
    assert_eq!(host.source, FontSource::Project);
    assert_eq!(host.id, "noto-sans-400-normal");

    let imported = provider
        .resolve(&["brand/Noto Sans".to_owned()], 400, FontStyle::Normal)
        .expect("imported namespaced family resolves");
    assert_eq!(imported.source, FontSource::Project);
    assert_ne!(imported.id, host.id);
    assert!(imported.id.starts_with("brand/"), "{}", imported.id);
}

#[test]
fn unparseable_font_asset_warns_and_locked_fails() {
    let fs = MemFs::new().with("p/bad.ttf", b"not a font".to_vec());
    let doc = parse(
        r#"zenith version=1 {
  project id="proj.host" name="Host"
  assets {
    asset id="font.bad" kind="font" src="bad.ttf"
  }
  document id="doc.host" title="Host" {
    page id="page.host" w=(px)10 h=(px)10
  }
}
"#,
    );
    let warnings = CollectWarnings::new();
    let host = Host::new(&fs, &NoConfig).with_warnings(&warnings);
    build_font_provider(host, &doc, Some(Path::new("p")), false).expect("skips");
    let messages = warnings.take();
    assert_eq!(messages.len(), 1);
    assert!(
        messages[0].starts_with("font asset 'font.bad' could not be parsed:"),
        "{messages:?}"
    );

    let err = build_font_provider(host, &doc, Some(Path::new("p")), true).expect_err("locked");
    assert_eq!(err.diagnostics[0].code, "asset.sha256_missing");
}

#[test]
fn missing_asset_and_text_source_are_reported_through_the_fs() {
    let fs = MemFs::new().with("p/copy.txt", b"Hello".to_vec());
    let mut doc = parse(
        r#"zenith version=1 {
  project id="proj.host" name="Host"
  assets {
    asset id="logo" kind="image" src="logo.png"
  }
  document id="doc.host" title="Host" {
    page id="page.host" w=(px)100 h=(px)100 {
      text id="t.ok" src="copy.txt" x=(px)0 y=(px)0 w=(px)100 h=(px)20
      text id="t.bad" src="gone.txt" x=(px)0 y=(px)20 w=(px)100 h=(px)20
    }
  }
}
"#,
    );
    let missing = collect_missing_asset_diagnostics(&fs, &doc, Path::new("p"));
    assert_eq!(missing.len(), 1);
    assert_eq!(
        missing[0].message,
        "asset 'logo' file not found: 'p/logo.png'"
    );

    let mut diagnostics = Vec::new();
    resolve_text_sources(&fs, &mut doc, Some(Path::new("p")), &mut diagnostics);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].subject_id.as_deref(), Some("t.bad"));
    assert_eq!(diagnostics[0].code, "text.src_missing");
}

#[test]
fn extra_fonts_layer_over_the_shared_base() {
    let fs = MemFs::new();
    let extra = [crate::ExtraFont {
        family: "Extra Face".to_owned(),
        weight: 400,
        style: FontStyle::Normal,
        bytes: std::sync::Arc::from(NOTO),
        index: 0,
        source: FontSource::Project,
    }];
    let host = Host::new(&fs, &NoConfig).with_extra_fonts(&extra);
    let doc = parse(HOST_IMPORTING_BRAND);

    let provider = build_font_provider(host, &doc, None, false).expect("provider");
    assert!(
        provider
            .resolve(&["Extra Face".to_owned()], 400, FontStyle::Normal)
            .is_some(),
        "the extra face must resolve"
    );

    let plain =
        build_font_provider(Host::new(&fs, &NoConfig), &doc, None, false).expect("provider");
    assert!(
        plain
            .resolve(&["Extra Face".to_owned()], 400, FontStyle::Normal)
            .is_none(),
        "the extra face must not leak into the shared base"
    );
}

#[test]
fn each_provider_shares_the_bundled_bytes() {
    let fs = MemFs::new();
    let doc = parse(HOST_IMPORTING_BRAND);
    let bytes = |p: &zenith_core::BytesFontProvider| {
        p.resolve(&["Noto Sans".to_owned()], 400, FontStyle::Normal)
            .expect("bundled sans")
            .bytes
    };
    let a = build_font_provider(Host::new(&fs, &NoConfig), &doc, None, false).expect("a");
    let b = build_font_provider(Host::new(&fs, &NoConfig), &doc, None, false).expect("b");
    assert!(
        std::sync::Arc::ptr_eq(&bytes(&a), &bytes(&b)),
        "two providers must share one copy of the bundled bytes"
    );
}

#[test]
fn font_log_records_the_requests_that_miss() {
    let fs = MemFs::new();
    let log = zenith_core::FontMissLog::new();
    let host = Host::new(&fs, &NoConfig).with_font_log(&log);
    let doc = parse(HOST_IMPORTING_BRAND);
    let provider = build_font_provider(host, &doc, None, false).expect("provider");
    assert!(
        provider
            .resolve(&["Missing Family".to_owned()], 400, FontStyle::Normal)
            .is_none()
    );
    let got = log.requests();
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].family, "Missing Family");
    assert!(!got[0].family_registered);
}
