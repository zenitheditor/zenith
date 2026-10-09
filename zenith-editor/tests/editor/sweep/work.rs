//! Engine work of `gesture.preview` against `gesture.commit`, counted per
//! call (parses, validations, transaction runs, patches, compiles,
//! rasters). Deterministic, so it can gate regressions.

use serde_json::json;
use zenith_editor::Work;

use crate::common::Driver;

#[test]
fn preview_compiles_one_page_and_commit_validates_once() {
    let mut d = Driver::example("flowchart.zen");
    d.ok("select.hit", json!({ "x": 180, "y": 60 }));
    let preview = d
        .outcome("gesture.preview", json!({ "dx": 5, "dy": 5 }))
        .work;
    let ghost = d
        .outcome(
            "gesture.preview",
            json!({ "dx": 5, "dy": 5, "render": false }),
        )
        .work;
    let commit = d
        .outcome("gesture.commit", json!({ "dx": 5, "dy": 5 }))
        .work;
    let typing = d
        .outcome(
            "buffer.set",
            json!({ "text": format!("{} ", d.session.text) }),
        )
        .work;
    println!("work preview {preview:?}");
    println!("work preview without raster {ghost:?}");
    println!("work commit {commit:?}");
    println!("work buffer.set {typing:?}");
    assert_eq!(
        preview,
        Work {
            parses: 1,
            validations: 0,
            tx_runs: 1,
            patches: 0,
            compiles: 2,
            rasters: 1,
        }
    );
    assert_eq!(ghost.rasters, 0);
    assert_eq!(
        commit,
        Work {
            parses: 2,
            validations: 1,
            tx_runs: 1,
            patches: 1,
            compiles: 1,
            rasters: 0,
        }
    );
    assert_eq!(typing.validations, 1);
    assert_eq!(typing.compiles, 0);
}
