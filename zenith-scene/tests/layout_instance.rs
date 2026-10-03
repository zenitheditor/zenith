//! Auto-layout of `instance` children: an instance hugs its component's
//! content bounds, and a fixed or fill size fits the component into its box.

mod common;
use common::*;
use zenith_scene::layout_boxes;

fn doc(children: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.li" name="LI"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
  }}
  styles {{}}
  components {{
    component id="comp.chip" {{
      rect id="body" x=(px)5 y=(px)5 w=(px)40 h=(px)20 fill=(token)"color.k"
    }}
    component id="comp.card" {{
      frame id="card" x=(px)0 y=(px)0 layout="column" padding=(px)4 {{
        rect id="top" w=(px)60 h=(px)10 fill=(token)"color.k"
        rect id="bottom" w=(px)30 h=(px)20 fill=(token)"color.k"
      }}
    }}
  }}
  document id="doc.li" title="LI" {{
    page id="p" w=(px)800 h=(px)600 {{
{children}
    }}
  }}
}}
"##
    )
}

fn boxes(src: &str) -> std::collections::BTreeMap<String, (f64, f64, f64, f64)> {
    layout_boxes(&parse(src), 0, &default_provider())
        .into_iter()
        .map(|(k, b)| (k, (b.x, b.y, b.w, b.h)))
        .collect()
}

#[test]
fn hugging_instance_takes_its_component_bounds() {
    let src = doc(
        r#"      frame id="f" x=(px)100 y=(px)50 layout="row" gap=(px)10 align="start" {
        instance id="a" component="comp.chip"
        instance id="b" component="comp.card"
        rect id="r" w=(px)10 h=(px)10 fill=(token)"color.k"
      }"#,
    );
    let b = boxes(&src);
    // comp.chip content spans 40x20; comp.card hugs 68x38.
    assert_eq!(b.get("a"), Some(&(100.0, 50.0, 40.0, 20.0)));
    assert_eq!(b.get("b"), Some(&(150.0, 50.0, 68.0, 38.0)));
    assert_eq!(b.get("r"), Some(&(228.0, 50.0, 10.0, 10.0)));
    assert_eq!(b.get("f"), Some(&(100.0, 50.0, 138.0, 38.0)));

    let result = compile(&parse(&src), &default_provider());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let rects = fill_rects(&result);
    // The chip's content top-left lands on the slot's (translate path).
    assert!(rects.contains(&(100.0, 50.0, 40.0, 20.0)), "{rects:?}");
    // The card's lowered frame children draw inside its slot.
    assert!(rects.contains(&(154.0, 54.0, 60.0, 10.0)), "{rects:?}");
    assert!(rects.contains(&(154.0, 64.0, 30.0, 20.0)), "{rects:?}");
}

#[test]
fn fill_instance_fits_the_component_into_its_slot() {
    let src = doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 layout="column" {
        instance id="a" component="comp.chip"
      }"#,
    );
    // Cross-axis stretch: the slot is 200 wide and keeps the 2:1 aspect.
    let b = boxes(&src);
    assert_eq!(b.get("a"), Some(&(0.0, 0.0, 200.0, 100.0)));
    let result = compile(&parse(&src), &default_provider());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let scaled = result.scene.commands.iter().any(|c| {
        matches!(c, SceneCommand::PushScaleTranslate { sx, sy, .. } if *sx == 5.0 && *sy == 5.0)
    });
    assert!(scaled, "{:?}", result.scene.commands);
}

#[test]
fn instance_w_overrides_the_component_width() {
    let src = doc(r#"      frame id="f" x=(px)0 y=(px)0 layout="row" {
        instance id="a" component="comp.chip" w=(px)80
        instance id="b" component="comp.chip" w="fill" max-w=(px)10
      }"#);
    let b = boxes(&src);
    // A fixed w keeps the aspect for the hugging h.
    assert_eq!(b.get("a"), Some(&(0.0, 0.0, 80.0, 40.0)));
    // `fill` in a hugging row hugs (clamped by max-w).
    assert_eq!(b.get("b").map(|b| b.2), Some(10.0));
}

#[test]
fn absolute_instance_counts_from_the_frame() {
    let src = doc(
        r#"      frame id="f" x=(px)100 y=(px)100 w=(px)300 h=(px)200 layout="column" {
        rect id="r" h=(px)10 fill=(token)"color.k"
        instance id="a" component="comp.chip" x=(px)20 y=(px)30 position="absolute"
      }"#,
    );
    let result = compile(&parse(&src), &default_provider());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        fill_rects(&result).contains(&(125.0, 135.0, 40.0, 20.0)),
        "{:?}",
        fill_rects(&result)
    );
}

#[test]
fn instance_without_component_is_unsized() {
    let src = doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)300 layout="row" {
        instance id="a" component="comp.missing"
      }"#,
    );
    let result = compile(&parse(&src), &default_provider());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "layout.unsized_child"),
        "{:?}",
        result.diagnostics
    );
}
