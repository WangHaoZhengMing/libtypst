use std::io::Cursor;
use std::path::Path;

use image::{DynamicImage, ImageFormat, RgbaImage};
use resvg::tiny_skia::{Pixmap, Transform};

const PLACEHOLDER_WIDTH: u32 = 192;
const PLACEHOLDER_HEIGHT: u32 = 108;
const SVG_CANVAS_WIDTH: u32 = 960;
const SVG_CANVAS_HEIGHT: u32 = 540;
const SVG_HEADER: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="960" height="540" viewBox="0 0 960 540">"##;
const SVG_DEFS: &str = concat!(
    r##"<defs><linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">"##,
    r##"<stop offset="0%" stop-color="#f4f6f8"/><stop offset="100%" stop-color="#dde6ec"/>"##,
    r##"</linearGradient></defs>"##,
);
const SVG_BACKGROUND: &str = r##"<rect width="960" height="540" rx="32" fill="url(#bg)"/>"##;
const SVG_CARD: &str = r##"<rect x="90" y="90" width="780" height="360" rx="26" fill="#ffffff" stroke="#cfd8de" stroke-width="4"/>"##;
const SVG_BADGE: &str = concat!(
    r##"<rect x="130" y="126" width="110" height="42" rx="14" fill="#2f80ed"/>"##,
    r##"<text x="185" y="154" text-anchor="middle" font-family="-apple-system,BlinkMacSystemFont,Segoe UI,sans-serif" font-size="24" font-weight="700" fill="#ffffff">T</text>"##,
);
const SVG_TITLE: &str = r##"<text x="130" y="230" font-family="-apple-system,BlinkMacSystemFont,Segoe UI,sans-serif" font-size="42" font-weight="700" fill="#22303a">Sandbox blocked</text>"##;
const SVG_SUBTITLE: &str = r##"<text x="130" y="282" font-family="-apple-system,BlinkMacSystemFont,Segoe UI,sans-serif" font-size="24" fill="#5a6b77">External asset unavailable in Quick Look</text>"##;
const SVG_LABEL_PREFIX: &str = r##"<text x="130" y="340" font-family="-apple-system,BlinkMacSystemFont,Segoe UI,sans-serif" font-size="22" fill="#7b8b96">"##;
const SVG_FOOTER: &str = r##"</text></svg>"##;

pub fn placeholder_source_comment(path: &Path) -> String {
    format!(
        "// Sandbox blocked access to external Typst file: {}\n",
        path.display()
    )
}

pub fn placeholder_bytes(path: &Path) -> Vec<u8> {
    let svg = placeholder_svg(path);
    let svg_bytes = svg.as_bytes().to_vec();
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());

    match ext.as_deref() {
        Some("svg") => svg_bytes,
        Some("jpg") | Some("jpeg") => {
            placeholder_raster_bytes(&svg, ImageFormat::Jpeg)
                .unwrap_or_else(|| svg_bytes.clone())
        }
        Some("png") => {
            placeholder_raster_bytes(&svg, ImageFormat::Png)
                .unwrap_or_else(|| svg_bytes.clone())
        }
        Some("gif") => {
            placeholder_raster_bytes(&svg, ImageFormat::Gif)
                .unwrap_or_else(|| svg_bytes.clone())
        }
        Some("webp") => {
            placeholder_raster_bytes(&svg, ImageFormat::WebP)
                .unwrap_or_else(|| svg_bytes.clone())
        }
        _ => svg_bytes,
    }
}

pub fn placeholder_svg(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("external asset");
    let label = escape_xml(name);
    format!(
        "{}{}{}{}{}{}{}{}{}",
        SVG_HEADER,
        SVG_DEFS,
        SVG_BACKGROUND,
        SVG_CARD,
        SVG_BADGE,
        SVG_TITLE,
        SVG_SUBTITLE,
        SVG_LABEL_PREFIX,
        label + SVG_FOOTER,
    )
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn placeholder_raster_bytes(svg: &str, format: ImageFormat) -> Option<Vec<u8>> {
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(svg, &options).ok()?;
    let mut pixmap = Pixmap::new(PLACEHOLDER_WIDTH, PLACEHOLDER_HEIGHT)?;

    let source_size = tree.size().to_int_size();
    let scale_x = PLACEHOLDER_WIDTH as f32 / SVG_CANVAS_WIDTH as f32;
    let scale_y = PLACEHOLDER_HEIGHT as f32 / SVG_CANVAS_HEIGHT as f32;
    if source_size.width() == 0 || source_size.height() == 0 {
        return None;
    }
    resvg::render(&tree, Transform::from_scale(scale_x, scale_y), &mut pixmap.as_mut());

    let image = RgbaImage::from_raw(
        PLACEHOLDER_WIDTH,
        PLACEHOLDER_HEIGHT,
        pixmap.data().to_vec(),
    )?;

    let mut bytes = Vec::new();
    let mut cursor = Cursor::new(&mut bytes);
    DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, format)
        .ok()?;
    Some(bytes)
}