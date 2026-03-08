use anyhow::{Context, Result};
use pdf_lib_rs::api::{PageSizes, PdfDocument};
use pdf_lib_rs::core::context::PdfContext;
use pdf_lib_rs::core::objects::{PdfDict, PdfName, PdfObject, PdfRawStream, PdfRef};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn load_pdf(path: &Path) -> Result<PdfDocument> {
    let bytes = fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
    PdfDocument::load(&bytes).map_err(|e| anyhow::anyhow!("Failed to parse PDF: {}", e))
}

pub fn parse_page_size(name: &str) -> Result<[f64; 2]> {
    match name.to_lowercase().as_str() {
        "letter" => Ok(PageSizes::LETTER),
        "a4" => Ok(PageSizes::A4),
        "legal" => Ok(PageSizes::LEGAL),
        "tabloid" => Ok(PageSizes::TABLOID),
        "ledger" => Ok(PageSizes::LEDGER),
        "a3" => Ok(PageSizes::A3),
        "a5" => Ok(PageSizes::A5),
        "executive" => Ok(PageSizes::EXECUTIVE),
        _ => anyhow::bail!(
            "Unknown page size '{}'. Use: letter, a4, legal, tabloid, ledger, a3, a5, executive",
            name
        ),
    }
}

pub fn parse_page_list(pages: &str, max_pages: usize) -> Result<Vec<usize>> {
    let mut result = Vec::new();
    for part in pages.split(',') {
        let part = part.trim();
        if let Some((start, end)) = part.split_once('-') {
            let start: usize = start.trim().parse().context("Invalid page number")?;
            let end: usize = end.trim().parse().context("Invalid page number")?;
            if start == 0 || end == 0 {
                anyhow::bail!("Page numbers are 1-based");
            }
            if start > max_pages || end > max_pages {
                anyhow::bail!(
                    "Page number out of range (document has {} pages)",
                    max_pages
                );
            }
            for p in start..=end {
                result.push(p - 1);
            }
        } else {
            let p: usize = part.parse().context("Invalid page number")?;
            if p == 0 {
                anyhow::bail!("Page numbers are 1-based");
            }
            if p > max_pages {
                anyhow::bail!(
                    "Page {} out of range (document has {} pages)",
                    p,
                    max_pages
                );
            }
            result.push(p - 1);
        }
    }
    Ok(result)
}

pub fn parse_page_range(pages: &str) -> Result<(u32, u32)> {
    if let Some((start, end)) = pages.split_once('-') {
        let s: u32 = start.trim().parse().context("Invalid page number")?;
        let e: u32 = end.trim().parse().context("Invalid page number")?;
        Ok((s, e))
    } else {
        let p: u32 = pages.trim().parse().context("Invalid page number")?;
        Ok((p, p))
    }
}

// --- PDF object helpers ---

pub fn stream_raw_size(ctx: &PdfContext, r: &PdfRef) -> usize {
    if let Some(PdfObject::Stream(s)) = ctx.lookup(r) {
        s.contents.len()
    } else {
        0
    }
}

pub fn resolve_dict<'a>(ctx: &'a PdfContext, obj: &'a PdfObject) -> Option<&'a PdfDict> {
    match obj {
        PdfObject::Dict(d) => Some(d),
        PdfObject::Ref(r) => {
            if let Some(PdfObject::Dict(d)) = ctx.lookup(r) {
                Some(d)
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn get_page_resources<'a>(ctx: &'a PdfContext, page_dict: &'a PdfDict) -> Option<&'a PdfDict> {
    let res = page_dict.get(&PdfName::of("Resources")).or_else(|| {
        if let Some(PdfObject::Ref(parent_ref)) = page_dict.get(&PdfName::of("Parent")) {
            if let Some(PdfObject::Dict(parent)) = ctx.lookup(parent_ref) {
                return parent.get(&PdfName::of("Resources"));
            }
        }
        None
    })?;
    resolve_dict(ctx, res)
}

// --- Image helpers ---

pub struct ImageInfo<'a> {
    pub stream: &'a PdfRawStream,
    pub page: usize,
}

pub fn collect_images<'a>(ctx: &'a PdfContext, page_refs: &[PdfRef]) -> Vec<ImageInfo<'a>> {
    let mut images = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (i, page_ref) in page_refs.iter().enumerate() {
        let Some(PdfObject::Dict(page_dict)) = ctx.lookup(page_ref) else {
            continue;
        };
        let Some(res) = get_page_resources(ctx, page_dict) else {
            continue;
        };
        let Some(xobj_dict) = res
            .get(&PdfName::of("XObject"))
            .and_then(|o| resolve_dict(ctx, o))
        else {
            continue;
        };
        for (_name, value) in xobj_dict.entries() {
            let Some(r) = (match value {
                PdfObject::Ref(r) => Some(r),
                _ => None,
            }) else {
                continue;
            };
            let key = (r.object_number, r.generation_number);
            if !seen.insert(key) {
                continue;
            }
            let Some(PdfObject::Stream(s)) = ctx.lookup(r) else {
                continue;
            };
            if let Some(PdfObject::Name(subtype)) = s.dict.get(&PdfName::of("Subtype")) {
                if subtype.as_string() == "/Image" {
                    images.push(ImageInfo {
                        stream: s,
                        page: i + 1,
                    });
                }
            }
        }
    }
    images
}

pub fn get_filter_name(stream: &PdfRawStream) -> Option<String> {
    match stream.dict.get(&PdfName::of("Filter")) {
        Some(PdfObject::Name(n)) => Some(n.as_string().to_string()),
        Some(PdfObject::Array(arr)) => {
            if arr.size() == 1 {
                if let Some(PdfObject::Name(n)) = arr.get(0) {
                    return Some(n.as_string().to_string());
                }
            }
            None
        }
        _ => None,
    }
}

pub fn image_extension(stream: &PdfRawStream) -> &'static str {
    match get_filter_name(stream).as_deref() {
        Some("/DCTDecode") => "jpg",
        Some("/JPXDecode") => "jp2",
        Some("/FlateDecode") => "png",
        _ => "bin",
    }
}

pub fn write_image(stream: &PdfRawStream, path: &Path) -> Result<usize> {
    let filter = get_filter_name(stream);
    match filter.as_deref() {
        Some("/DCTDecode") | Some("/JPXDecode") => {
            fs::write(path, &stream.contents)?;
            Ok(stream.contents.len())
        }
        Some("/FlateDecode") => {
            let mut decoder = flate2::read::ZlibDecoder::new(&stream.contents[..]);
            let mut raw = Vec::new();
            std::io::Read::read_to_end(&mut decoder, &mut raw)?;

            let width = match stream.dict.get(&PdfName::of("Width")) {
                Some(PdfObject::Number(n)) => n.as_number() as u32,
                _ => anyhow::bail!("Image missing Width"),
            };
            let height = match stream.dict.get(&PdfName::of("Height")) {
                Some(PdfObject::Number(n)) => n.as_number() as u32,
                _ => anyhow::bail!("Image missing Height"),
            };
            let bpc = match stream.dict.get(&PdfName::of("BitsPerComponent")) {
                Some(PdfObject::Number(n)) => n.as_number() as u8,
                _ => 8,
            };

            let (color_type, components) = determine_png_color(stream, bpc);

            let file = fs::File::create(path)?;
            let w = &mut std::io::BufWriter::new(file);
            let mut encoder = png::Encoder::new(w, width, height);
            encoder.set_color(color_type);
            let bit_depth = match bpc {
                1 => png::BitDepth::One,
                2 => png::BitDepth::Two,
                4 => png::BitDepth::Four,
                16 => png::BitDepth::Sixteen,
                _ => png::BitDepth::Eight,
            };
            encoder.set_depth(bit_depth);

            if color_type == png::ColorType::Indexed {
                if let Some(palette) = extract_palette(stream) {
                    encoder.set_palette(palette.clone());
                }
            }

            let mut writer = encoder.write_header()?;

            let row_bytes = if bpc < 8 {
                (width as usize * components * bpc as usize).div_ceil(8)
            } else {
                width as usize * components * (bpc as usize / 8)
            };
            let expected = row_bytes * height as usize;
            if raw.len() >= expected {
                writer.write_image_data(&raw[..expected])?;
            } else {
                let mut padded = raw;
                padded.resize(expected, 0);
                writer.write_image_data(&padded)?;
            }
            drop(writer);

            Ok(expected)
        }
        _ => {
            fs::write(path, &stream.contents)?;
            Ok(stream.contents.len())
        }
    }
}

fn determine_png_color(stream: &PdfRawStream, _bpc: u8) -> (png::ColorType, usize) {
    match stream.dict.get(&PdfName::of("ColorSpace")) {
        Some(PdfObject::Name(n)) => match n.as_string() {
            "/DeviceRGB" => (png::ColorType::Rgb, 3),
            "/DeviceCMYK" => (png::ColorType::Rgb, 3),
            "/DeviceGray" => (png::ColorType::Grayscale, 1),
            _ => (png::ColorType::Grayscale, 1),
        },
        Some(PdfObject::Array(arr)) => {
            if let Some(PdfObject::Name(n)) = arr.get(0) {
                if n.as_string() == "/Indexed" {
                    return (png::ColorType::Indexed, 1);
                }
            }
            (png::ColorType::Grayscale, 1)
        }
        _ => (png::ColorType::Grayscale, 1),
    }
}

fn extract_palette(stream: &PdfRawStream) -> Option<Vec<u8>> {
    if let Some(PdfObject::Array(arr)) = stream.dict.get(&PdfName::of("ColorSpace")) {
        if arr.size() >= 4 {
            if let Some(PdfObject::String(s)) = arr.get(3) {
                return Some(s.as_bytes_decoded());
            }
        }
    }
    None
}

// --- Stream / text helpers ---

pub fn decompress_stream(stream: &PdfRawStream) -> Vec<u8> {
    match get_filter_name(stream).as_deref() {
        Some("/FlateDecode") => {
            let mut decoder = flate2::read::ZlibDecoder::new(&stream.contents[..]);
            let mut out = Vec::new();
            if std::io::Read::read_to_end(&mut decoder, &mut out).is_ok() {
                out
            } else {
                stream.contents.clone()
            }
        }
        None => stream.contents.clone(),
        _ => stream.contents.clone(),
    }
}

pub fn get_page_content_bytes(ctx: &PdfContext, page_dict: &PdfDict) -> Vec<u8> {
    let mut all = Vec::new();
    match page_dict.get(&PdfName::of("Contents")) {
        Some(PdfObject::Ref(r)) => {
            if let Some(PdfObject::Stream(s)) = ctx.lookup(r) {
                all.extend(decompress_stream(s));
            }
        }
        Some(PdfObject::Array(arr)) => {
            for i in 0..arr.size() {
                if let Some(PdfObject::Ref(r)) = arr.get(i) {
                    if let Some(PdfObject::Stream(s)) = ctx.lookup(r) {
                        all.extend(decompress_stream(s));
                        all.push(b' ');
                    }
                }
            }
        }
        _ => {}
    }
    all
}

// --- Font / text extraction ---

pub struct FontInfo {
    pub char_map: HashMap<u16, String>,
    pub is_two_byte: bool,
}

pub type FontMap = HashMap<String, FontInfo>;

fn is_type0_font(fd: &PdfDict) -> bool {
    if let Some(PdfObject::Name(subtype)) = fd.get(&PdfName::of("Subtype")) {
        if subtype.as_string() == "/Type0" {
            return true;
        }
    }
    if let Some(PdfObject::Name(enc)) = fd.get(&PdfName::of("Encoding")) {
        let enc_name = enc.as_string();
        if enc_name == "/Identity-H" || enc_name == "/Identity-V" {
            return true;
        }
    }
    false
}

pub fn build_font_map(ctx: &PdfContext, page_dict: &PdfDict) -> FontMap {
    let mut map = FontMap::new();
    let Some(res) = get_page_resources(ctx, page_dict) else {
        return map;
    };
    let font_obj = match res.get(&PdfName::of("Font")) {
        Some(PdfObject::Dict(d)) => Some(d),
        Some(PdfObject::Ref(r)) => {
            if let Some(PdfObject::Dict(d)) = ctx.lookup(r) {
                Some(d)
            } else {
                None
            }
        }
        _ => None,
    };
    let Some(fonts) = font_obj else { return map };

    for (name, value) in fonts.entries() {
        let font_name = name.as_string().to_string();
        let font_dict = match value {
            PdfObject::Dict(d) => Some(d),
            PdfObject::Ref(r) => {
                if let Some(PdfObject::Dict(d)) = ctx.lookup(r) {
                    Some(d)
                } else {
                    None
                }
            }
            _ => None,
        };
        let Some(fd) = font_dict else { continue };

        let two_byte = is_type0_font(fd);
        let mut char_map: HashMap<u16, String> = HashMap::new();

        if let Some(tounicode) = fd.get(&PdfName::of("ToUnicode")) {
            let cmap_stream = match tounicode {
                PdfObject::Ref(r) => {
                    if let Some(PdfObject::Stream(s)) = ctx.lookup(r) {
                        Some(s)
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(s) = cmap_stream {
                let data = decompress_stream(s);
                parse_tounicode_cmap(&data, &mut char_map);
            }
        }

        if char_map.is_empty() {
            if let Some(PdfObject::Name(enc)) = fd.get(&PdfName::of("Encoding")) {
                let _enc_name = enc.as_string();
                // For MacRomanEncoding/WinAnsiEncoding, leave char_map empty
                // and fall through to Latin-1 decoding
            }
        }

        map.insert(
            font_name,
            FontInfo {
                char_map,
                is_two_byte: two_byte,
            },
        );
    }
    map
}

fn parse_tounicode_cmap(data: &[u8], map: &mut HashMap<u16, String>) {
    let text = String::from_utf8_lossy(data);

    for section in text.split("beginbfchar") {
        let Some(body) = section.split("endbfchar").next() else {
            continue;
        };
        let mut chars = body.chars().peekable();
        loop {
            while chars.peek().is_some() && chars.peek() != Some(&'<') {
                chars.next();
            }
            if chars.peek().is_none() {
                break;
            }
            chars.next();
            let src_hex: String = chars.by_ref().take_while(|&c| c != '>').collect();
            while chars.peek().is_some() && chars.peek() != Some(&'<') {
                chars.next();
            }
            if chars.peek().is_none() {
                break;
            }
            chars.next();
            let dst_hex: String = chars.by_ref().take_while(|&c| c != '>').collect();

            if let Ok(src) = u16::from_str_radix(src_hex.trim(), 16) {
                if let Some(ch) = hex_to_unicode_string(&dst_hex) {
                    map.insert(src, ch);
                }
            }
        }
    }

    for section in text.split("beginbfrange") {
        let Some(body) = section.split("endbfrange").next() else {
            continue;
        };
        let mut chars = body.chars().peekable();
        loop {
            let mut hexes = Vec::new();
            for _ in 0..3 {
                while chars.peek().is_some() && chars.peek() != Some(&'<') {
                    chars.next();
                }
                if chars.peek().is_none() {
                    break;
                }
                chars.next();
                let hex: String = chars.by_ref().take_while(|&c| c != '>').collect();
                hexes.push(hex);
            }
            if hexes.len() < 3 {
                break;
            }

            let Ok(lo) = u16::from_str_radix(hexes[0].trim(), 16) else {
                continue;
            };
            let Ok(hi) = u16::from_str_radix(hexes[1].trim(), 16) else {
                continue;
            };
            let Ok(dst_start) = u32::from_str_radix(hexes[2].trim(), 16) else {
                continue;
            };

            for code in lo..=hi {
                let unicode_val = dst_start + (code - lo) as u32;
                if let Some(ch) = char::from_u32(unicode_val) {
                    map.insert(code, ch.to_string());
                }
            }
        }
    }
}

fn hex_to_unicode_string(hex: &str) -> Option<String> {
    let hex = hex.trim();
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect();
    let mut result = String::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let code = ((bytes[i] as u16) << 8) | bytes[i + 1] as u16;
        if let Some(ch) = char::from_u32(code as u32) {
            result.push(ch);
        }
        i += 2;
    }
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

pub fn decode_pdf_string(bytes: &[u8], font_info: Option<&FontInfo>) -> String {
    let mut result = String::new();
    if let Some(info) = font_info {
        if info.is_two_byte {
            let mut i = 0;
            while i + 1 < bytes.len() {
                let code = ((bytes[i] as u16) << 8) | bytes[i + 1] as u16;
                if let Some(s) = info.char_map.get(&code) {
                    result.push_str(s);
                } else if let Some(ch) = char::from_u32(code as u32) {
                    result.push(ch);
                }
                i += 2;
            }
            return result;
        }
        if !info.char_map.is_empty() {
            for &b in bytes {
                if let Some(s) = info.char_map.get(&(b as u16)) {
                    result.push_str(s);
                } else {
                    result.push(b as char);
                }
            }
            return result;
        }
    }
    for &b in bytes {
        result.push(b as char);
    }
    result
}

#[derive(Debug, Clone)]
pub enum ContentToken {
    Operator(String),
    Name(String),
    Number(f64),
    LiteralString(Vec<u8>),
    HexString(Vec<u8>),
    ArrayStart,
    ArrayEnd,
}

pub fn tokenize_content_stream(data: &[u8]) -> Vec<ContentToken> {
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut _in_array = false;

    while i < data.len() {
        let b = data[i];

        if b == b' ' || b == b'\n' || b == b'\r' || b == b'\t' || b == 0 || b == 12 {
            i += 1;
            continue;
        }

        if b == b'%' {
            while i < data.len() && data[i] != b'\n' && data[i] != b'\r' {
                i += 1;
            }
            continue;
        }

        if b == b'(' {
            i += 1;
            let mut depth = 1;
            let mut bytes = Vec::new();
            let mut escaped = false;
            while i < data.len() && depth > 0 {
                let c = data[i];
                if escaped {
                    match c {
                        b'n' => bytes.push(b'\n'),
                        b'r' => bytes.push(b'\r'),
                        b't' => bytes.push(b'\t'),
                        b'b' => bytes.push(8),
                        b'f' => bytes.push(12),
                        b'0'..=b'7' => {
                            let mut oct = (c - b'0') as u16;
                            if i + 1 < data.len() && data[i + 1] >= b'0' && data[i + 1] <= b'7' {
                                i += 1;
                                oct = oct * 8 + (data[i] - b'0') as u16;
                                if i + 1 < data.len()
                                    && data[i + 1] >= b'0'
                                    && data[i + 1] <= b'7'
                                {
                                    i += 1;
                                    oct = oct * 8 + (data[i] - b'0') as u16;
                                }
                            }
                            bytes.push(oct as u8);
                        }
                        _ => bytes.push(c),
                    }
                    escaped = false;
                } else if c == b'\\' {
                    escaped = true;
                } else if c == b'(' {
                    depth += 1;
                    bytes.push(c);
                } else if c == b')' {
                    depth -= 1;
                    if depth > 0 {
                        bytes.push(c);
                    }
                } else {
                    bytes.push(c);
                }
                i += 1;
            }
            tokens.push(ContentToken::LiteralString(bytes));
            continue;
        }

        if b == b'<' && i + 1 < data.len() && data[i + 1] != b'<' {
            i += 1;
            let mut hex = Vec::new();
            while i < data.len() && data[i] != b'>' {
                if data[i].is_ascii_hexdigit() {
                    hex.push(data[i]);
                }
                i += 1;
            }
            if i < data.len() {
                i += 1;
            }
            let mut bytes = Vec::new();
            let mut j = 0;
            while j + 1 < hex.len() {
                let hi = hex_digit(hex[j]);
                let lo = hex_digit(hex[j + 1]);
                bytes.push((hi << 4) | lo);
                j += 2;
            }
            if j < hex.len() {
                bytes.push(hex_digit(hex[j]) << 4);
            }
            tokens.push(ContentToken::HexString(bytes));
            continue;
        }

        if b == b'[' {
            tokens.push(ContentToken::ArrayStart);
            _in_array = true;
            i += 1;
            continue;
        }
        if b == b']' {
            tokens.push(ContentToken::ArrayEnd);
            _in_array = false;
            i += 1;
            continue;
        }

        if b == b'<' && i + 1 < data.len() && data[i + 1] == b'<' {
            i += 2;
            let mut depth = 1;
            while i + 1 < data.len() && depth > 0 {
                if data[i] == b'<' && data[i + 1] == b'<' {
                    depth += 1;
                    i += 2;
                } else if data[i] == b'>' && data[i + 1] == b'>' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }

        if b == b'/' {
            i += 1;
            let start = i;
            while i < data.len() && !is_pdf_delim_or_ws(data[i]) {
                i += 1;
            }
            let name = String::from_utf8_lossy(&data[start..i]).to_string();
            tokens.push(ContentToken::Name(format!("/{}", name)));
            continue;
        }

        if b == b'-' || b == b'+' || b == b'.' || b.is_ascii_digit() {
            let start = i;
            if b == b'-' || b == b'+' {
                i += 1;
            }
            let mut has_dot = b == b'.';
            while i < data.len() && (data[i].is_ascii_digit() || (data[i] == b'.' && !has_dot)) {
                if data[i] == b'.' {
                    has_dot = true;
                }
                i += 1;
            }
            let s = String::from_utf8_lossy(&data[start..i]);
            if let Ok(n) = s.parse::<f64>() {
                tokens.push(ContentToken::Number(n));
            }
            continue;
        }

        if b.is_ascii_alphabetic() || b == b'*' || b == b'\'' || b == b'"' {
            let start = i;
            while i < data.len()
                && (data[i].is_ascii_alphabetic()
                    || data[i] == b'*'
                    || data[i] == b'\''
                    || data[i] == b'"')
            {
                i += 1;
            }
            let op = String::from_utf8_lossy(&data[start..i]).to_string();
            tokens.push(ContentToken::Operator(op));
            continue;
        }

        i += 1;
    }

    tokens
}

fn hex_digit(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

fn is_pdf_delim_or_ws(b: u8) -> bool {
    matches!(
        b,
        b' ' | b'\n'
            | b'\r'
            | b'\t'
            | 0
            | 12
            | b'('
            | b')'
            | b'<'
            | b'>'
            | b'['
            | b']'
            | b'{'
            | b'}'
            | b'/'
            | b'%'
    )
}

pub fn extract_text_from_page(ctx: &PdfContext, page_dict: &PdfDict) -> String {
    let content = get_page_content_bytes(ctx, page_dict);
    let font_map = build_font_map(ctx, page_dict);
    let mut result = String::new();
    let mut current_font: Option<&FontInfo> = None;

    let tokens = tokenize_content_stream(&content);
    let mut operand_stack: Vec<ContentToken> = Vec::new();

    for token in &tokens {
        match token {
            ContentToken::Operator(op) => {
                match op.as_str() {
                    "Tf" => {
                        if let Some(ContentToken::Name(name)) = operand_stack.first() {
                            current_font = font_map.get(name.as_str());
                        }
                    }
                    "Tj" | "'" | "\"" => {
                        if let Some(
                            ContentToken::LiteralString(bytes) | ContentToken::HexString(bytes),
                        ) = operand_stack.last()
                        {
                            result.push_str(&decode_pdf_string(bytes, current_font));
                        }
                    }
                    "TJ" => {
                        for tok in &operand_stack {
                            match tok {
                                ContentToken::LiteralString(bytes)
                                | ContentToken::HexString(bytes) => {
                                    result.push_str(&decode_pdf_string(bytes, current_font));
                                }
                                ContentToken::Number(n) => {
                                    if *n <= -100.0 {
                                        result.push(' ');
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "Td" | "TD" | "T*" => {
                        if op == "T*" {
                            result.push('\n');
                        } else if let Some(ContentToken::Number(ty)) = operand_stack.get(1) {
                            if *ty != 0.0 {
                                result.push('\n');
                            }
                        }
                    }
                    "Tm" => {
                        if !result.is_empty() && !result.ends_with('\n') {
                            result.push('\n');
                        }
                    }
                    "ET" => {
                        if !result.is_empty() && !result.ends_with('\n') {
                            result.push('\n');
                        }
                    }
                    _ => {}
                }
                operand_stack.clear();
            }
            _ => {
                operand_stack.push(token.clone());
            }
        }
    }

    let mut cleaned = String::new();
    let mut prev_blank = false;
    for line in result.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_blank && !cleaned.is_empty() {
                cleaned.push('\n');
                prev_blank = true;
            }
        } else {
            cleaned.push_str(trimmed);
            cleaned.push('\n');
            prev_blank = false;
        }
    }
    cleaned.trim().to_string()
}

// --- Screenshot helpers ---

#[derive(Debug)]
pub struct ScreenshotArgs {
    pub input: String,
    pub output_prefix: String,
    pub first_page: Option<u32>,
    pub last_page: Option<u32>,
    pub dpi: u32,
    pub format: String,
    pub scale_to: Option<u32>,
    pub gray: bool,
    pub single: bool,
}

pub fn build_pdftoppm_args(args: &ScreenshotArgs) -> Vec<String> {
    let mut cmd_args = Vec::new();

    match args.format.as_str() {
        "png" => cmd_args.push("-png".to_string()),
        "jpeg" | "jpg" => cmd_args.push("-jpeg".to_string()),
        "tiff" | "tif" => cmd_args.push("-tiff".to_string()),
        _ => cmd_args.push("-png".to_string()),
    }

    if let Some(scale) = args.scale_to {
        cmd_args.push("-scale-to".to_string());
        cmd_args.push(scale.to_string());
    } else {
        cmd_args.push("-r".to_string());
        cmd_args.push(args.dpi.to_string());
    }

    if let Some(first) = args.first_page {
        cmd_args.push("-f".to_string());
        cmd_args.push(first.to_string());
    }
    if let Some(last) = args.last_page {
        cmd_args.push("-l".to_string());
        cmd_args.push(last.to_string());
    }

    if args.gray {
        cmd_args.push("-gray".to_string());
    }

    if args.single {
        cmd_args.push("-singlefile".to_string());
    }

    cmd_args.push(args.input.clone());
    cmd_args.push(args.output_prefix.clone());

    cmd_args
}
