use kdl::{KdlDocument, KdlNode, KdlValue};
use zenith_core::{BlendMode, Color, GradientStop};

use crate::error::ZpxError;
use crate::model::{
    Adjustment, AlphaMode, BlobRef, Canvas, ColorSpace, ContentHash, Layer, LayerSource, Mask,
    MaskSource, ZpxDoc,
};

const VERSION: i128 = 1;

pub fn parse_manifest(source: &str) -> Result<ZpxDoc, ZpxError> {
    let kdl_doc: KdlDocument = source
        .parse()
        .map_err(|e: kdl::KdlError| ZpxError::new(format!("manifest KDL parse error: {e}")))?;
    ensure_only_named_nodes(kdl_doc.nodes(), &["zpx"], "manifest root")?;
    let root = only_named_node(kdl_doc.nodes(), "zpx", "manifest root")?;
    let version = required_i128(root, "version")?;
    if version != VERSION {
        return Err(ZpxError::new(format!("unsupported zpx version: {version}")));
    }
    let children = required_children(root, "zpx")?;
    ensure_only_named_nodes(children.nodes(), &["canvas", "layers"], "zpx")?;
    let canvas = parse_canvas(only_named_node(children.nodes(), "canvas", "zpx")?)?;
    let layers = parse_layers(only_named_node(children.nodes(), "layers", "zpx")?)?;
    Ok(ZpxDoc { canvas, layers })
}

pub fn serialize_manifest(doc: &ZpxDoc) -> String {
    let mut out = String::new();
    out.push_str("zpx version=");
    out.push_str(&VERSION.to_string());
    out.push_str(" {\n");
    write_canvas(&mut out, &doc.canvas, 1);
    write_layers(&mut out, &doc.layers, 1);
    out.push_str("}\n");
    out
}

fn parse_canvas(node: &KdlNode) -> Result<Canvas, ZpxError> {
    let color_space = match required_str(node, "color-space")? {
        "srgb" => ColorSpace::Srgb,
        other => {
            return Err(ZpxError::new(format!(
                "unsupported canvas color-space: {other}"
            )));
        }
    };
    let alpha_mode = match required_str(node, "alpha")? {
        "premultiplied" => AlphaMode::Premultiplied,
        other => {
            return Err(ZpxError::new(format!(
                "unsupported canvas alpha mode: {other}"
            )));
        }
    };
    let width_px = required_u32(node, "width")?;
    let height_px = required_u32(node, "height")?;
    if width_px == 0 || height_px == 0 {
        return Err(ZpxError::new("canvas width and height must be positive"));
    }

    Ok(Canvas {
        width_px,
        height_px,
        color_space,
        alpha_mode,
    })
}

fn parse_layers(node: &KdlNode) -> Result<Vec<Layer>, ZpxError> {
    let children = required_children(node, "layers")?;
    let mut layers = Vec::new();
    for child in children.nodes() {
        match child.name().value() {
            "layer" => layers.push(parse_layer(child)?),
            other => return Err(ZpxError::new(format!("unexpected node in layers: {other}"))),
        }
    }
    Ok(layers)
}

fn parse_layer(node: &KdlNode) -> Result<Layer, ZpxError> {
    let children = required_children(node, "layer")?;
    ensure_only_named_nodes(children.nodes(), &["source", "mask"], "layer")?;
    let source_node = only_named_node(children.nodes(), "source", "layer")?;
    ensure_at_most_one_named_node(children.nodes(), "mask", "layer")?;
    let mask = optional_named_node(children.nodes(), "mask")
        .map(parse_mask)
        .transpose()?;
    Ok(Layer {
        id: required_str(node, "id")?.to_owned(),
        blend_mode: parse_blend_mode(required_str(node, "blend")?)?,
        opacity: required_unit_f64(node, "opacity")?,
        visible: required_bool(node, "visible")?,
        clipping: required_bool(node, "clipping")?,
        mask,
        source: parse_layer_source(source_node)?,
    })
}

fn parse_layer_source(node: &KdlNode) -> Result<LayerSource, ZpxError> {
    match required_str(node, "kind")? {
        "buffer" => Ok(LayerSource::Buffer(parse_blob_ref(node)?)),
        "adjustment" => parse_adjustment_source(node),
        "group" => {
            let children = required_children(node, "group source")?;
            ensure_only_named_nodes(children.nodes(), &["layers"], "group source")?;
            let layers_node = only_named_node(children.nodes(), "layers", "group source")?;
            Ok(LayerSource::Group(parse_layers(layers_node)?))
        }
        other => Err(ZpxError::new(format!("unknown layer source kind: {other}"))),
    }
}

fn parse_adjustment_source(node: &KdlNode) -> Result<LayerSource, ZpxError> {
    match required_str(node, "adjustment")? {
        "gradient-map" => {
            let children = required_children(node, "gradient-map source")?;
            ensure_only_named_nodes(children.nodes(), &["stops"], "gradient-map source")?;
            let stops_node = only_named_node(children.nodes(), "stops", "gradient-map source")?;
            Ok(LayerSource::Adjustment(Adjustment::GradientMap {
                stops: parse_gradient_stops(stops_node)?,
            }))
        }
        other => Err(ZpxError::new(format!("unknown adjustment kind: {other}"))),
    }
}

fn parse_gradient_stops(node: &KdlNode) -> Result<Vec<GradientStop>, ZpxError> {
    let children = required_children(node, "stops")?;
    let mut stops = Vec::new();
    let mut previous_offset = None;
    for child in children.nodes() {
        match child.name().value() {
            "stop" => {
                let offset = required_unit_f64(child, "offset")?;
                if let Some(previous_offset) = previous_offset
                    && offset < previous_offset
                {
                    return Err(ZpxError::new("gradient stops must be sorted by offset"));
                }
                previous_offset = Some(offset);
                stops.push(GradientStop {
                    offset,
                    color: parse_color(required_str(child, "color")?)?,
                });
            }
            other => return Err(ZpxError::new(format!("unexpected node in stops: {other}"))),
        }
    }
    if stops.len() < 2 {
        return Err(ZpxError::new("gradient map requires at least two stops"));
    }
    Ok(stops)
}

fn parse_mask(node: &KdlNode) -> Result<Mask, ZpxError> {
    let source = match required_str(node, "source")? {
        "alpha" => MaskSource::Alpha,
        "luminance" => MaskSource::Luminance,
        other => return Err(ZpxError::new(format!("unknown mask source: {other}"))),
    };
    Ok(Mask {
        source,
        blob: parse_blob_ref(node)?,
        invert: required_bool(node, "invert")?,
    })
}

fn parse_blob_ref(node: &KdlNode) -> Result<BlobRef, ZpxError> {
    Ok(BlobRef::new(ContentHash::parse(required_str(
        node, "hash",
    )?)?))
}

fn write_canvas(out: &mut String, canvas: &Canvas, depth: usize) {
    indent(out, depth);
    out.push_str("canvas width=");
    out.push_str(&canvas.width_px.to_string());
    out.push_str(" height=");
    out.push_str(&canvas.height_px.to_string());
    out.push_str(" color-space=");
    out.push_str(&quoted(color_space_name(canvas.color_space)));
    out.push_str(" alpha=");
    out.push_str(&quoted(alpha_mode_name(canvas.alpha_mode)));
    out.push('\n');
}

fn write_layers(out: &mut String, layers: &[Layer], depth: usize) {
    indent(out, depth);
    out.push_str("layers {\n");
    for layer in layers {
        write_layer(out, layer, depth + 1);
    }
    indent(out, depth);
    out.push_str("}\n");
}

fn write_layer(out: &mut String, layer: &Layer, depth: usize) {
    indent(out, depth);
    out.push_str("layer id=");
    out.push_str(&quoted(&layer.id));
    out.push_str(" blend=");
    out.push_str(&quoted(blend_mode_name(layer.blend_mode)));
    out.push_str(" opacity=");
    out.push_str(&format_f64(layer.opacity));
    out.push_str(" visible=");
    out.push_str(kdl_bool(layer.visible));
    out.push_str(" clipping=");
    out.push_str(kdl_bool(layer.clipping));
    out.push_str(" {\n");
    write_layer_source(out, &layer.source, depth + 1);
    if let Some(mask) = &layer.mask {
        write_mask(out, mask, depth + 1);
    }
    indent(out, depth);
    out.push_str("}\n");
}

fn write_layer_source(out: &mut String, source: &LayerSource, depth: usize) {
    match source {
        LayerSource::Buffer(blob) => {
            indent(out, depth);
            out.push_str("source kind=\"buffer\" hash=");
            out.push_str(&quoted(blob.hash.as_str()));
            out.push('\n');
        }
        LayerSource::Adjustment(Adjustment::GradientMap { stops }) => {
            indent(out, depth);
            out.push_str("source kind=\"adjustment\" adjustment=\"gradient-map\" {\n");
            write_gradient_stops(out, stops, depth + 1);
            indent(out, depth);
            out.push_str("}\n");
        }
        LayerSource::Group(layers) => {
            indent(out, depth);
            out.push_str("source kind=\"group\" {\n");
            write_layers(out, layers, depth + 1);
            indent(out, depth);
            out.push_str("}\n");
        }
    }
}

fn write_gradient_stops(out: &mut String, stops: &[GradientStop], depth: usize) {
    indent(out, depth);
    out.push_str("stops {\n");
    for stop in stops {
        indent(out, depth + 1);
        out.push_str("stop offset=");
        out.push_str(&format_f64(stop.offset));
        out.push_str(" color=");
        out.push_str(&quoted(&format_color(stop.color)));
        out.push('\n');
    }
    indent(out, depth);
    out.push_str("}\n");
}

fn write_mask(out: &mut String, mask: &Mask, depth: usize) {
    indent(out, depth);
    out.push_str("mask source=");
    out.push_str(&quoted(mask_source_name(mask.source)));
    out.push_str(" hash=");
    out.push_str(&quoted(mask.blob.hash.as_str()));
    out.push_str(" invert=");
    out.push_str(kdl_bool(mask.invert));
    out.push('\n');
}

fn only_named_node<'a>(
    nodes: &'a [KdlNode],
    name: &str,
    context: &str,
) -> Result<&'a KdlNode, ZpxError> {
    let mut found: Option<&KdlNode> = None;
    for node in nodes {
        if node.name().value() == name {
            if found.is_some() {
                return Err(ZpxError::new(format!("duplicate {name} node in {context}")));
            }
            found = Some(node);
        }
    }
    found.ok_or_else(|| ZpxError::new(format!("missing {name} node in {context}")))
}

fn optional_named_node<'a>(nodes: &'a [KdlNode], name: &str) -> Option<&'a KdlNode> {
    nodes.iter().find(|node| node.name().value() == name)
}

fn ensure_only_named_nodes(
    nodes: &[KdlNode],
    allowed: &[&str],
    context: &str,
) -> Result<(), ZpxError> {
    for node in nodes {
        let name = node.name().value();
        if !allowed.contains(&name) {
            return Err(ZpxError::new(format!(
                "unexpected node in {context}: {name}"
            )));
        }
    }
    Ok(())
}

fn ensure_at_most_one_named_node(
    nodes: &[KdlNode],
    name: &str,
    context: &str,
) -> Result<(), ZpxError> {
    let mut found = false;
    for node in nodes {
        if node.name().value() == name {
            if found {
                return Err(ZpxError::new(format!("duplicate {name} node in {context}")));
            }
            found = true;
        }
    }
    Ok(())
}

fn required_children<'a>(node: &'a KdlNode, context: &str) -> Result<&'a KdlDocument, ZpxError> {
    node.children()
        .ok_or_else(|| ZpxError::new(format!("missing child block for {context}")))
}

fn required_str<'a>(node: &'a KdlNode, key: &str) -> Result<&'a str, ZpxError> {
    match node.get(key) {
        Some(KdlValue::String(value)) => Ok(value.as_str()),
        Some(other) => Err(ZpxError::new(format!(
            "property {key} must be a string, got {other:?}"
        ))),
        None => Err(ZpxError::new(format!("missing required property {key}"))),
    }
}

fn required_i128(node: &KdlNode, key: &str) -> Result<i128, ZpxError> {
    match node.get(key) {
        Some(KdlValue::Integer(value)) => Ok(*value),
        Some(other) => Err(ZpxError::new(format!(
            "property {key} must be an integer, got {other:?}"
        ))),
        None => Err(ZpxError::new(format!("missing required property {key}"))),
    }
}

fn required_u32(node: &KdlNode, key: &str) -> Result<u32, ZpxError> {
    let value = required_i128(node, key)?;
    u32::try_from(value).map_err(|_| ZpxError::new(format!("property {key} is out of u32 range")))
}

fn required_f64(node: &KdlNode, key: &str) -> Result<f64, ZpxError> {
    let value = match node.get(key) {
        Some(KdlValue::Integer(value)) => Ok(*value as f64),
        Some(KdlValue::Float(value)) => Ok(*value),
        Some(other) => Err(ZpxError::new(format!(
            "property {key} must be numeric, got {other:?}"
        ))),
        None => Err(ZpxError::new(format!("missing required property {key}"))),
    }?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ZpxError::new(format!("property {key} must be finite")))
    }
}

fn required_unit_f64(node: &KdlNode, key: &str) -> Result<f64, ZpxError> {
    let value = required_f64(node, key)?;
    if (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(ZpxError::new(format!("property {key} must be in 0..=1")))
    }
}

fn required_bool(node: &KdlNode, key: &str) -> Result<bool, ZpxError> {
    match node.get(key) {
        Some(KdlValue::Bool(value)) => Ok(*value),
        Some(other) => Err(ZpxError::new(format!(
            "property {key} must be a boolean, got {other:?}"
        ))),
        None => Err(ZpxError::new(format!("missing required property {key}"))),
    }
}

fn parse_blend_mode(value: &str) -> Result<BlendMode, ZpxError> {
    match value {
        "normal" => Ok(BlendMode::Normal),
        "multiply" => Ok(BlendMode::Multiply),
        "screen" => Ok(BlendMode::Screen),
        "overlay" => Ok(BlendMode::Overlay),
        "darken" => Ok(BlendMode::Darken),
        "lighten" => Ok(BlendMode::Lighten),
        "color-dodge" => Ok(BlendMode::ColorDodge),
        "color-burn" => Ok(BlendMode::ColorBurn),
        "hard-light" => Ok(BlendMode::HardLight),
        "soft-light" => Ok(BlendMode::SoftLight),
        "difference" => Ok(BlendMode::Difference),
        "exclusion" => Ok(BlendMode::Exclusion),
        "hue" => Ok(BlendMode::Hue),
        "saturation" => Ok(BlendMode::Saturation),
        "color" => Ok(BlendMode::Color),
        "luminosity" => Ok(BlendMode::Luminosity),
        other => Err(ZpxError::new(format!("unknown blend mode: {other}"))),
    }
}

fn blend_mode_name(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Normal => "normal",
        BlendMode::Multiply => "multiply",
        BlendMode::Screen => "screen",
        BlendMode::Overlay => "overlay",
        BlendMode::Darken => "darken",
        BlendMode::Lighten => "lighten",
        BlendMode::ColorDodge => "color-dodge",
        BlendMode::ColorBurn => "color-burn",
        BlendMode::HardLight => "hard-light",
        BlendMode::SoftLight => "soft-light",
        BlendMode::Difference => "difference",
        BlendMode::Exclusion => "exclusion",
        BlendMode::Hue => "hue",
        BlendMode::Saturation => "saturation",
        BlendMode::Color => "color",
        BlendMode::Luminosity => "luminosity",
    }
}

fn color_space_name(value: ColorSpace) -> &'static str {
    match value {
        ColorSpace::Srgb => "srgb",
    }
}

fn alpha_mode_name(value: AlphaMode) -> &'static str {
    match value {
        AlphaMode::Premultiplied => "premultiplied",
    }
}

fn mask_source_name(value: MaskSource) -> &'static str {
    match value {
        MaskSource::Alpha => "alpha",
        MaskSource::Luminance => "luminance",
    }
}

fn parse_color(value: &str) -> Result<Color, ZpxError> {
    let hex = value
        .strip_prefix('#')
        .ok_or_else(|| ZpxError::new("color must start with #"))?;
    if hex.len() != 8 {
        return Err(ZpxError::new("color must be #rrggbbaa"));
    }
    Ok(Color::srgb(
        parse_hex_byte(hex, 0)?,
        parse_hex_byte(hex, 2)?,
        parse_hex_byte(hex, 4)?,
        parse_hex_byte(hex, 6)?,
    ))
}

fn parse_hex_byte(hex: &str, start: usize) -> Result<u8, ZpxError> {
    let end = start + 2;
    let pair = hex
        .get(start..end)
        .ok_or_else(|| ZpxError::new("color contains an incomplete hex byte"))?;
    u8::from_str_radix(pair, 16).map_err(|_| ZpxError::new("color contains non-hex digits"))
}

fn format_color(color: Color) -> String {
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        color.r, color.g, color.b, color.a
    )
}

fn format_f64(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

fn quoted(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn kdl_bool(value: bool) -> &'static str {
    match value {
        true => "#true",
        false => "#false",
    }
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("    ");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(byte: u8) -> ContentHash {
        ContentHash::from_bytes(&[byte])
    }

    fn buffer_doc() -> ZpxDoc {
        ZpxDoc {
            canvas: Canvas::new(640, 480),
            layers: vec![Layer {
                id: "base".to_owned(),
                blend_mode: BlendMode::Normal,
                opacity: 1.0,
                visible: true,
                clipping: false,
                mask: Some(Mask {
                    source: MaskSource::Alpha,
                    blob: BlobRef::new(hash(2)),
                    invert: false,
                }),
                source: LayerSource::Buffer(BlobRef::new(hash(1))),
            }],
        }
    }

    #[test]
    fn parse_serialize_roundtrip_including_buffer_layer() {
        let doc = buffer_doc();
        let manifest = serialize_manifest(&doc);
        let parsed = parse_manifest(&manifest).expect("manifest should parse");
        assert_eq!(parsed, doc);
        assert_eq!(serialize_manifest(&parsed), manifest);
    }

    #[test]
    fn deterministic_serialize_repeated() {
        let doc = buffer_doc();
        assert_eq!(serialize_manifest(&doc), serialize_manifest(&doc));
    }

    #[test]
    fn invalid_hash_rejected() {
        let manifest = r#"zpx version=1 {
    canvas width=1 height=1 color-space="srgb" alpha="premultiplied"
    layers {
        layer id="bad" blend="normal" opacity=1.0 visible=#true clipping=#false {
            source kind="buffer" hash="ABC"
        }
    }
}
"#;
        let err = parse_manifest(manifest).expect_err("invalid hash must fail");
        assert!(err.message().contains("64 lowercase hex"));
    }

    #[test]
    fn missing_required_canvas_rejected() {
        let manifest = r#"zpx version=1 {
    layers {
    }
}
"#;
        let err = parse_manifest(manifest).expect_err("missing canvas must fail");
        assert!(err.message().contains("missing canvas"));
    }

    #[test]
    fn missing_required_layers_rejected() {
        let manifest = r#"zpx version=1 {
    canvas width=1 height=1 color-space="srgb" alpha="premultiplied"
}
"#;
        let err = parse_manifest(manifest).expect_err("missing layers must fail");
        assert!(err.message().contains("missing layers"));
    }

    #[test]
    fn missing_required_source_rejected() {
        let manifest = r#"zpx version=1 {
    canvas width=1 height=1 color-space="srgb" alpha="premultiplied"
    layers {
        layer id="bad" blend="normal" opacity=1.0 visible=#true clipping=#false {
        }
    }
}
"#;
        let err = parse_manifest(manifest).expect_err("missing source must fail");
        assert!(err.message().contains("missing source"));
    }

    #[test]
    fn zero_sized_canvas_rejected() {
        let manifest = r#"zpx version=1 {
    canvas width=0 height=1 color-space="srgb" alpha="premultiplied"
    layers {
    }
}
"#;
        let err = parse_manifest(manifest).expect_err("zero width must fail");
        assert!(err.message().contains("positive"));
    }

    #[test]
    fn opacity_out_of_range_rejected() {
        let manifest = r#"zpx version=1 {
    canvas width=1 height=1 color-space="srgb" alpha="premultiplied"
    layers {
        layer id="bad" blend="normal" opacity=1.5 visible=#true clipping=#false {
            source kind="buffer" hash="6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d"
        }
    }
}
"#;
        let err = parse_manifest(manifest).expect_err("invalid opacity must fail");
        assert!(err.message().contains("opacity"));
    }

    #[test]
    fn gradient_map_adjustment_parse_serialize() {
        let doc = ZpxDoc {
            canvas: Canvas::new(256, 256),
            layers: vec![Layer {
                id: "grade".to_owned(),
                blend_mode: BlendMode::SoftLight,
                opacity: 0.75,
                visible: true,
                clipping: false,
                mask: None,
                source: LayerSource::Adjustment(Adjustment::GradientMap {
                    stops: vec![
                        GradientStop {
                            offset: 0.0,
                            color: Color::srgb(0, 0, 0, 255),
                        },
                        GradientStop {
                            offset: 1.0,
                            color: Color::srgb(255, 240, 128, 255),
                        },
                    ],
                }),
            }],
        };
        let manifest = serialize_manifest(&doc);
        let parsed = parse_manifest(&manifest).expect("gradient map should parse");
        assert_eq!(parsed, doc);
    }

    #[test]
    fn gradient_map_requires_sorted_stops() {
        let manifest = r##"zpx version=1 {
    canvas width=1 height=1 color-space="srgb" alpha="premultiplied"
    layers {
        layer id="grade" blend="normal" opacity=1.0 visible=#true clipping=#false {
            source kind="adjustment" adjustment="gradient-map" {
                stops {
                    stop offset=1.0 color="#ffffffff"
                    stop offset=0.0 color="#000000ff"
                }
            }
        }
    }
}
"##;
        let err = parse_manifest(manifest).expect_err("unsorted stops must fail");
        assert!(err.message().contains("sorted"));
    }

    #[test]
    fn group_nesting_parse_serialize() {
        let doc = ZpxDoc {
            canvas: Canvas::new(128, 128),
            layers: vec![Layer {
                id: "group".to_owned(),
                blend_mode: BlendMode::Normal,
                opacity: 1.0,
                visible: true,
                clipping: false,
                mask: None,
                source: LayerSource::Group(vec![Layer {
                    id: "child".to_owned(),
                    blend_mode: BlendMode::Multiply,
                    opacity: 0.5,
                    visible: false,
                    clipping: true,
                    mask: None,
                    source: LayerSource::Buffer(BlobRef::new(hash(3))),
                }]),
            }],
        };
        let manifest = serialize_manifest(&doc);
        let parsed = parse_manifest(&manifest).expect("group should parse");
        assert_eq!(parsed, doc);
    }

    #[test]
    fn content_hash_from_bytes_matches_session_object_hash() {
        let content = b"zpx blob";
        assert_eq!(
            ContentHash::from_bytes(content).as_str(),
            zenith_session::object_hash(content)
        );
    }
}
