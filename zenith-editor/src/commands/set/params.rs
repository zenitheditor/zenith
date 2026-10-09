//! The params of `node.set`.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::gesture::flags::Flags;

/// A token-backed property: a token id to bind, or a raw value the engine
/// binds through a token with that value (an existing one, else a new one).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(super) enum TokenOrValue {
    /// A token id.
    Token(String),
    /// A raw value: `#rrggbb` / `#rrggbbaa` for a color, px for a size, a
    /// family name, a weight, or a number.
    Value { value: Value },
}

/// One span's new text.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SpanEdit {
    /// The 0-based span index.
    pub(super) index: usize,
    pub(super) text: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SetParams {
    #[serde(default)]
    pub(super) id: Option<String>,
    #[serde(default)]
    pub(super) x: Option<f64>,
    #[serde(default)]
    pub(super) y: Option<f64>,
    #[serde(default)]
    pub(super) w: Option<f64>,
    #[serde(default)]
    pub(super) h: Option<f64>,
    #[serde(default)]
    pub(super) rotate: Option<f64>,
    #[serde(default)]
    pub(super) opacity: Option<f64>,
    #[serde(default)]
    pub(super) fill: Option<TokenOrValue>,
    #[serde(default)]
    pub(super) stroke: Option<TokenOrValue>,
    #[serde(default)]
    pub(super) stroke_width: Option<TokenOrValue>,
    /// `null` removes the attribute.
    #[serde(default, deserialize_with = "nullable")]
    pub(super) radius: Option<Option<TokenOrValue>>,
    #[serde(default, deserialize_with = "nullable")]
    pub(super) font_family: Option<Option<TokenOrValue>>,
    #[serde(default, deserialize_with = "nullable")]
    pub(super) font_size: Option<Option<TokenOrValue>>,
    #[serde(default, deserialize_with = "nullable")]
    pub(super) font_weight: Option<Option<TokenOrValue>>,
    #[serde(default)]
    pub(super) align: Option<String>,
    #[serde(default)]
    pub(super) text: Option<String>,
    #[serde(default)]
    pub(super) spans: Option<Vec<SpanEdit>>,
    #[serde(default)]
    pub(super) visible: Option<bool>,
    #[serde(default)]
    pub(super) locked: Option<bool>,
    #[serde(default)]
    pub(super) detach: bool,
    #[serde(default)]
    pub(super) detach_anchor: bool,
    #[serde(default)]
    pub(super) confirm_size: bool,
    #[serde(default)]
    pub(super) replace: bool,
    #[serde(default)]
    pub(super) absolute: bool,
}

/// A present field: `Some(value)`, with JSON `null` as `Some(None)`.
fn nullable<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

impl SetParams {
    pub(super) fn flags(&self) -> Flags {
        Flags {
            detach: self.detach,
            detach_anchor: self.detach_anchor,
            confirm_size: self.confirm_size,
            replace: self.replace,
            absolute: self.absolute,
            reorder: false,
        }
    }

    pub(super) fn numbers(&self) -> [Option<f64>; 6] {
        [self.x, self.y, self.w, self.h, self.rotate, self.opacity]
    }

    /// `true` when the request only shows, hides, locks, or unlocks: the
    /// fields a locked or hidden node still takes.
    pub(super) fn only_flags(&self) -> bool {
        let SetParams {
            id: _,
            x,
            y,
            w,
            h,
            rotate,
            opacity,
            fill,
            stroke,
            stroke_width,
            radius,
            font_family,
            font_size,
            font_weight,
            align,
            text,
            spans,
            visible,
            locked,
            detach: _,
            detach_anchor: _,
            confirm_size: _,
            replace: _,
            absolute: _,
        } = self;
        (visible.is_some() || locked.is_some())
            && [x, y, w, h, rotate, opacity].iter().all(|v| v.is_none())
            && [fill, stroke, stroke_width].iter().all(|v| v.is_none())
            && [radius, font_family, font_size, font_weight]
                .iter()
                .all(|v| v.is_none())
            && align.is_none()
            && text.is_none()
            && spans.is_none()
    }
}
