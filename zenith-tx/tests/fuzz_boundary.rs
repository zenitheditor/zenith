//! Fast deterministic adversarial / fuzz-boundary suite for the MCP trust
//! boundary (`KdlAdapter::parse`, `zenith_core::validate`, `Transaction::from_json`,
//! and `run_transaction`).
//!
//! Runs in a few milliseconds as part of `cargo test` and guarantees zero panics
//! on malformed, truncated, deeply nested, extreme-numeric, or mutated inputs.

use zenith_core::{KdlAdapter, KdlSource, validate};
use zenith_tx::{Transaction, TxStatus, run_transaction};

const VALID_BASE_DOC: &str = r##"zenith version=1 {
  project id="proj.fuzz" name="Fuzz"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#111827"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="size.body" type="dimension" value=(px)14
  }
  styles {}
  document id="doc.fuzz" title="Fuzz" {
    page id="page.1" w=(px)400 h=(px)300 background=(token)"color.bg" {
      rect id="r1" x=(px)10 y=(px)10 w=(px)100 h=(px)50 fill=(token)"color.ink"
      text id="t1" x=(px)10 y=(px)80 w=(px)200 h=(px)40 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" { span "Hello" }
    }
  }
}
"##;

/// Deterministic xorshift64 PRNG (no external crate, fixed seed).
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn next_usize(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        (self.next_u64() as usize) % bound
    }
}

fn mutate_bytes(seed_bytes: &[u8], rng: &mut Rng) -> Vec<u8> {
    let mut buf = seed_bytes.to_vec();
    let ops = 1 + rng.next_usize(6);
    for _ in 0..ops {
        if buf.is_empty() {
            buf.push((rng.next_u64() & 0xff) as u8);
            continue;
        }
        match rng.next_usize(4) {
            0 => {
                // Flip a byte.
                let idx = rng.next_usize(buf.len());
                buf[idx] ^= 1u8 << rng.next_usize(8);
            }
            1 => {
                // Insert a random byte (including NUL, quotes, braces, high bytes).
                let idx = rng.next_usize(buf.len() + 1);
                buf.insert(idx, (rng.next_u64() & 0xff) as u8);
            }
            2 => {
                // Delete a short slice.
                let start = rng.next_usize(buf.len());
                let len = (1 + rng.next_usize(8)).min(buf.len() - start);
                buf.drain(start..start + len);
            }
            _ => {
                // Truncate.
                let cut = rng.next_usize(buf.len());
                buf.truncate(cut);
            }
        }
    }
    buf
}

#[test]
fn kdl_parse_and_validate_never_panic_on_adversarial_or_mutated_inputs() {
    let deep_braces = format!("{}{}", "a { ".repeat(256), "}".repeat(256));
    let huge_coords = r##"zenith version=1 {
      project id="p" name="P"
      tokens format="zenith-token-v1" { token id="c" type="color" value="#000000" }
      styles {}
      document id="d" title="D" {
        page id="pg" w=(px)1e308 h=(px)-1e308 {
          rect id="r" x=(px)1e308 y=(px)-1e308 w=(px)0 h=(px)-5 fill=(token)"c"
        }
      }
    }"##;
    let adversarial: Vec<&[u8]> = vec![
        b"",
        b"\0\0\0\xff\xfe\xfd",
        b"zenith version=999999999999999999999999999 {}",
        b"zenith version=\"abc\" { document { page {} } }",
        b"\"unterminated string",
        deep_braces.as_bytes(),
        huge_coords.as_bytes(),
        b"zenith version=1 { document id=\"\" { page id=\"\" w=(px)10 h=(px)10 { rect id=\"\" } } }",
    ];

    for input in adversarial {
        if let Ok(doc) = KdlAdapter.parse(input) {
            let _ = validate(&doc);
            let _ = KdlAdapter.format(&doc);
        }
    }

    let mut rng = Rng(0x5EED_CAFE_1234_5678);
    for _ in 0..150 {
        let mutated = mutate_bytes(VALID_BASE_DOC.as_bytes(), &mut rng);
        if let Ok(doc) = KdlAdapter.parse(&mutated) {
            let _ = validate(&doc);
        }
    }
}

#[test]
fn tx_json_and_engine_never_panic_on_adversarial_or_mutated_inputs() {
    let doc = KdlAdapter
        .parse(VALID_BASE_DOC.as_bytes())
        .expect("valid base doc");

    let deep_json = format!("{{\"ops\":[{}null{}]}}", "[".repeat(128), "]".repeat(128));
    let adversarial_json: Vec<&str> = vec![
        "",
        "null",
        "{}",
        r#"{"ops":null}"#,
        r#"{"ops":[{}]}"#,
        r#"{"ops":[{"op":"unknown_op_xyz"}]}"#,
        r#"{"ops":[{"op":"set_geometry","node":"r1","x":1e309}]}"#,
        r#"{"ops":[{"op":"set_geometry","node":"missing","x":-1e308,"w":0}]}"#,
        r#"{"ops":[{"op":"set_span_text","node":"t1","span":999999,"text":"\u0000\u0007\n\"\\"}]}"#,
        r#"{"ops":[{"op":"duplicate_page","page":"page.1","new_id":"page.1","id_suffix":""}]}"#,
        r#"{"ops":[{"op":"reparent","node":"r1","new_parent":"r1"}]}"#,
        r#"{"ops":[{"op":"delete_node","node":"page.1"}]}"#,
        r#"{"ops":[{"op":"set_fill","node":"t1","fill":"not a valid token or hex"}]}"#,
        &deep_json,
    ];

    for raw in adversarial_json {
        if let Ok(tx) = Transaction::from_json(raw)
            && let Ok(res) = run_transaction(&doc, &tx)
        {
            assert!(matches!(
                res.status,
                TxStatus::Accepted | TxStatus::Rejected
            ));
        }
    }

    let seed_tx = r#"{"ops":[{"op":"set_geometry","node":"r1","x":25,"y":30},{"op":"set_span_text","node":"t1","span":0,"text":"Updated"}]}"#;
    let mut rng = Rng(0xDEAD_BEEF_9001_CAFE);
    for _ in 0..150 {
        let mutated = mutate_bytes(seed_tx.as_bytes(), &mut rng);
        if let Ok(s) = std::str::from_utf8(&mutated)
            && let Ok(tx) = Transaction::from_json(s)
        {
            let _ = run_transaction(&doc, &tx);
        }
    }
}
