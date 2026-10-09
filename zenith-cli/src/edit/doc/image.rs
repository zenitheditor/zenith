//! [`image_meta`]: the JSON that describes a render next to its PNG.

use serde_json::{Value, json};
use zenith_editor::RenderedImage;

/// `{sha256, width, height, page}` for `image`, plus `rect` (device px),
/// `scale`, `device_size` (device px), and `page_size` (page px) for a
/// viewport render.
pub(crate) fn image_meta(image: &RenderedImage) -> Value {
    let mut meta = json!({
        "sha256": image.sha256(),
        "width": image.width,
        "height": image.height,
        "page": image.page,
    });
    if let (Some(r), Some(m)) = (image.region, meta.as_object_mut()) {
        m.insert(
            "rect".into(),
            json!({"x": r.x, "y": r.y, "w": image.width, "h": image.height}),
        );
        m.insert("scale".into(), json!(r.scale));
        m.insert(
            "device_size".into(),
            json!({"w": r.device_width, "h": r.device_height}),
        );
        m.insert(
            "page_size".into(),
            json!({"w": r.page_width, "h": r.page_height}),
        );
    }
    meta
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_editor::ImageRegion;

    #[test]
    fn region_fields_only_with_a_region() {
        let mut image = RenderedImage {
            png: vec![1, 2, 3],
            width: 4,
            height: 5,
            page: 2,
            region: None,
        };
        let plain = image_meta(&image);
        assert!(plain.get("rect").is_none());
        image.region = Some(ImageRegion {
            x: 7,
            y: 9,
            scale: 2.5,
            device_width: 300,
            device_height: 200,
            page_width: 120.0,
            page_height: 80.0,
        });
        let meta = image_meta(&image);
        assert_eq!(meta["rect"], json!({"x": 7, "y": 9, "w": 4, "h": 5}));
        assert_eq!(meta["scale"], json!(2.5));
        assert_eq!(meta["device_size"], json!({"w": 300, "h": 200}));
        assert_eq!(meta["page_size"], json!({"w": 120.0, "h": 80.0}));
    }
}
