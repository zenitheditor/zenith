const REGULAR: &[u8] = include_bytes!("../../../zenith-core/assets/fonts/NotoSans-Regular.ttf");
const BOLD: &[u8] = include_bytes!("../../../zenith-core/assets/fonts/NotoSans-Bold.ttf");

fn u16_at(bytes: &[u8], index: usize) -> u16 {
    u16::from_be_bytes(bytes[index..index + 2].try_into().expect("u16 field"))
}

fn u32_at(bytes: &[u8], index: usize) -> u32 {
    u32::from_be_bytes(bytes[index..index + 4].try_into().expect("u32 field"))
}

fn relocate(bytes: &[u8], start: usize) -> Vec<u8> {
    let mut font = bytes.to_vec();
    for table in 0..usize::from(u16_at(bytes, 4)) {
        let offset_field = 12 + table * 16 + 8;
        let offset = u32_at(bytes, offset_field) + start as u32;
        font[offset_field..offset_field + 4].copy_from_slice(&offset.to_be_bytes());
    }
    font
}

pub fn font_collection() -> Vec<u8> {
    let first = 20;
    let second = (first + REGULAR.len() + 3) & !3;
    let mut collection = b"ttcf\0\x01\0\0\0\0\0\x02".to_vec();
    collection.extend_from_slice(&(first as u32).to_be_bytes());
    collection.extend_from_slice(&(second as u32).to_be_bytes());
    collection.extend_from_slice(&relocate(REGULAR, first));
    collection.resize(second, 0);
    collection.extend_from_slice(&relocate(BOLD, second));
    collection
}

pub fn bitmap_font(png: &[u8], character: char) -> (Vec<u8>, u16) {
    let face = ttf_parser::Face::parse(REGULAR, 0).expect("bundled font");
    let glyph = face.glyph_index(character).expect("bitmap glyph").0;
    let count = u32::from(face.number_of_glyphs());
    let data_start = 4 + 4 * (count + 1);
    let data_end = data_start + 8 + png.len() as u32;
    let mut sbix = vec![0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 12, 0, 32, 0, 72];
    for index in 0..=count {
        let offset = if index <= u32::from(glyph) {
            data_start
        } else {
            data_end
        };
        sbix.extend_from_slice(&offset.to_be_bytes());
    }
    sbix.extend_from_slice(&[0, 0, 0, 0]);
    sbix.extend_from_slice(b"png ");
    sbix.extend_from_slice(png);

    let table_count = usize::from(u16_at(REGULAR, 4));
    let mut tables = Vec::new();
    for index in 0..table_count {
        let record = 12 + index * 16;
        let offset = u32_at(REGULAR, record + 8) as usize;
        let length = u32_at(REGULAR, record + 12) as usize;
        tables.push((
            REGULAR[record..record + 4].to_vec(),
            REGULAR[offset..offset + length].to_vec(),
        ));
    }
    tables.push((b"sbix".to_vec(), sbix));
    tables.sort_by(|left, right| left.0.cmp(&right.0));
    let mut font = REGULAR[..12].to_vec();
    font[4..6].copy_from_slice(&(tables.len() as u16).to_be_bytes());
    font.resize(12 + tables.len() * 16, 0);
    for (index, (tag, bytes)) in tables.into_iter().enumerate() {
        while font.len() % 4 != 0 {
            font.push(0);
        }
        let offset = font.len() as u32;
        let record = 12 + index * 16;
        font[record..record + 4].copy_from_slice(&tag);
        font[record + 8..record + 12].copy_from_slice(&offset.to_be_bytes());
        font[record + 12..record + 16].copy_from_slice(&(bytes.len() as u32).to_be_bytes());
        font.extend_from_slice(&bytes);
    }
    (font, glyph)
}
