//! Test helpers for embedded vector captures.

pub fn svg_capture_dimensions(svg: &str) -> Vec<(u32, u32)> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    svg.split("data:image/png;base64,")
        .skip(1)
        .map(|data| {
            let mut bits = 0u32;
            let mut available = 0;
            let mut header = Vec::with_capacity(24);
            for symbol in data.bytes().take(32) {
                let value = ALPHABET
                    .iter()
                    .position(|&entry| entry == symbol)
                    .expect("base64 symbol");
                bits = (bits << 6) | value as u32;
                available += 6;
                if available >= 8 {
                    available -= 8;
                    header.push((bits >> available) as u8);
                }
            }
            assert!(header.starts_with(b"\x89PNG\r\n\x1a\n"));
            assert_eq!(&header[12..16], b"IHDR");
            (
                u32::from_be_bytes(header[16..20].try_into().unwrap()),
                u32::from_be_bytes(header[20..24].try_into().unwrap()),
            )
        })
        .collect()
}

pub fn svg_without_capture_data(svg: &str) -> String {
    let mut parts = svg.split("data:image/png;base64,");
    let mut output = parts.next().unwrap_or_default().to_owned();
    for part in parts {
        let (_, remainder) = part.split_once('"').expect("image attribute end");
        output.push_str("data:image/png;base64,\"");
        output.push_str(remainder);
    }
    output
}
