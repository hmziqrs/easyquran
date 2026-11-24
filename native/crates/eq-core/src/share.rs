//! Branded verse share card (PARITY C9; web `lib/share/verse-card.ts`).
//!
//! Layout is pure and takes a measure callback so it is testable without
//! fonts; [`render_verse_card`] shapes Arabic (RTL, full HarfBuzz via
//! `rustybuzz`) and Latin (LTR) verbatim from the bundled TTF faces, rasterizes
//! outlines with `tiny-skia`, and encodes a 1080 px wide PNG.

use std::fmt;

use rustybuzz::{Direction, Face, UnicodeBuffer, shape, ttf_parser::OutlineBuilder};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

pub const CARD_WIDTH: u32 = 1080;
const PADDING: f32 = 96.0;
const ARABIC_SIZE: f32 = 58.0;
const ARABIC_LEADING: f32 = 1.9;
const LATIN_SIZE: f32 = 38.0;
const LATIN_LEADING: f32 = 1.5;
const REF_SIZE: f32 = 26.0;
const BRAND_SIZE: f32 = 34.0;
const SMALL_SIZE: f32 = 24.0;
const RULE_GAP: f32 = 40.0;
const SECTION_GAP: f32 = 56.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    fn to_color(self) -> Color {
        Color::from_rgba8(self.0, self.1, self.2, 255)
    }
}

/// Web `PALETTE` (`verse-card.ts`).
const PALETTE_BG: Rgb = Rgb(0xFB, 0xF8, 0xF1);
const PALETTE_INK: Rgb = Rgb(0x1E, 0x2B, 0x25);
const PALETTE_MUTED: Rgb = Rgb(0x5F, 0x6B, 0x64);
const PALETTE_ACCENT: Rgb = Rgb(0x0E, 0x6B, 0x5C);
const PALETTE_RULE: Rgb = Rgb(0xD9, 0xD2, 0xBF);

/// One verse entry on the card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardVerse {
    pub text: String,
    pub reference: String,
}

/// Input to [`layout_card`] and [`render_verse_card`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardInput {
    pub verses: Vec<CardVerse>,
    pub include_reference: bool,
    pub link: Option<String>,
    pub brand: String,
    pub footer: String,
}

/// Which bundled face draws a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardFace {
    Arabic,
    Latin,
}

/// One laid-out element of the card: either a text line or a horizontal rule.
#[derive(Debug, Clone, PartialEq)]
pub enum CardLine {
    Rule {
        y: f32,
        color: Rgb,
    },
    Text {
        text: String,
        face: CardFace,
        size: f32,
        y: f32,
        rtl: bool,
        color: Rgb,
    },
}

/// Laid-out card dimensions and drawing primitives.
#[derive(Debug, Clone, PartialEq)]
pub struct CardLayout {
    pub width: u32,
    pub height: u32,
    pub lines: Vec<CardLine>,
}

/// Why rendering a card failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardError {
    InvalidFont(&'static str),
    InvalidDimensions,
    Encode(String),
}

impl fmt::Display for CardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CardError::InvalidFont(which) => write!(f, "invalid {which} font"),
            CardError::InvalidDimensions => write!(f, "invalid card dimensions"),
            CardError::Encode(msg) => write!(f, "png encode failed: {msg}"),
        }
    }
}

impl std::error::Error for CardError {}

/// Arabic-script detection (`isArabicScript` in `verse-card.ts`).
pub fn is_arabic_script(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(
            c,
            '\u{0600}'..='\u{06FF}'
                | '\u{0750}'..='\u{077F}'
                | '\u{08A0}'..='\u{08FF}'
                | '\u{FB50}'..='\u{FDFF}'
                | '\u{FE70}'..='\u{FEFF}'
        )
    })
}

/// Greedy word wrap (`wrapWords`). A single word wider than `max_width` keeps
/// its own line.
pub fn wrap_words<F>(words: &[&str], max_width: f32, mut measure: F) -> Vec<String>
where
    F: FnMut(&str) -> f32,
{
    let mut lines = Vec::new();
    let mut current = String::new();
    for &word in words {
        if current.is_empty() {
            current.push_str(word);
            continue;
        }
        let candidate = format!("{current} {word}");
        if measure(&candidate) > max_width {
            lines.push(current);
            current = word.to_owned();
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn body_spec(text: &str) -> (CardFace, f32, f32, bool) {
    if is_arabic_script(text) {
        (
            CardFace::Arabic,
            ARABIC_SIZE,
            ARABIC_SIZE * ARABIC_LEADING,
            true,
        )
    } else {
        (
            CardFace::Latin,
            LATIN_SIZE,
            LATIN_SIZE * LATIN_LEADING,
            false,
        )
    }
}

/// Lays out the card top to bottom (`layoutCard`).
pub fn layout_card<F>(input: &CardInput, mut measure: F) -> CardLayout
where
    F: FnMut(CardFace, f32, bool, &str) -> f32,
{
    let max_width = (CARD_WIDTH as f32) - PADDING * 2.0;
    let mut lines = Vec::new();
    let mut y = PADDING;

    let brand_rtl = is_arabic_script(&input.brand);
    let brand_face = if brand_rtl {
        CardFace::Arabic
    } else {
        CardFace::Latin
    };
    lines.push(CardLine::Text {
        text: input.brand.clone(),
        face: brand_face,
        size: BRAND_SIZE,
        y,
        rtl: brand_rtl,
        color: PALETTE_ACCENT,
    });
    y += BRAND_SIZE + RULE_GAP;
    lines.push(CardLine::Rule {
        y,
        color: PALETTE_RULE,
    });
    y += SECTION_GAP;

    for verse in &input.verses {
        let (face, size, leading, rtl) = body_spec(&verse.text);
        let words: Vec<&str> = verse.text.split_whitespace().collect();
        let wrapped = wrap_words(&words, max_width, |line| measure(face, size, rtl, line));
        for line in wrapped {
            lines.push(CardLine::Text {
                text: line,
                face,
                size,
                y,
                rtl,
                color: PALETTE_INK,
            });
            y += leading;
        }
        if input.include_reference {
            y += 12.0;
            let ref_rtl = is_arabic_script(&verse.reference);
            let ref_face = if ref_rtl {
                CardFace::Arabic
            } else {
                CardFace::Latin
            };
            lines.push(CardLine::Text {
                text: verse.reference.clone(),
                face: ref_face,
                size: REF_SIZE,
                y,
                rtl: ref_rtl,
                color: PALETTE_MUTED,
            });
            y += REF_SIZE * 1.5;
        }
        y += SECTION_GAP / 2.0;
    }

    y += RULE_GAP / 2.0;
    lines.push(CardLine::Rule {
        y,
        color: PALETTE_RULE,
    });
    y += SECTION_GAP;

    if let Some(ref link) = input.link {
        lines.push(CardLine::Text {
            text: link.clone(),
            face: CardFace::Latin,
            size: SMALL_SIZE,
            y,
            rtl: false,
            color: PALETTE_ACCENT,
        });
        y += SMALL_SIZE * 1.6;
    }

    let footer_rtl = is_arabic_script(&input.footer);
    let footer_face = if footer_rtl {
        CardFace::Arabic
    } else {
        CardFace::Latin
    };
    lines.push(CardLine::Text {
        text: input.footer.clone(),
        face: footer_face,
        size: SMALL_SIZE,
        y,
        rtl: footer_rtl,
        color: PALETTE_MUTED,
    });
    y += SMALL_SIZE + PADDING;

    CardLayout {
        width: CARD_WIDTH,
        height: y.ceil().max(1.0) as u32,
        lines,
    }
}

fn shape_line(face: &Face<'_>, text: &str, rtl: bool) -> rustybuzz::GlyphBuffer {
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.set_direction(if rtl {
        Direction::RightToLeft
    } else {
        Direction::LeftToRight
    });
    shape(face, &[], buffer)
}

fn measure_line(face: &Face<'_>, size: f32, rtl: bool, text: &str) -> f32 {
    let upem = face.units_per_em().max(1) as f32;
    let scale = size / upem;
    let shaped = shape_line(face, text, rtl);
    let advance: i32 = shaped.glyph_positions().iter().map(|p| p.x_advance).sum();
    (advance as f32) * scale
}

struct SkiaOutline(PathBuilder);

impl OutlineBuilder for SkiaOutline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.quad_to(x1, y1, x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.cubic_to(x1, y1, x2, y2, x, y);
    }

    fn close(&mut self) {
        self.0.close();
    }
}

fn draw_text_line(
    pixmap: &mut Pixmap,
    face: &Face<'_>,
    text: &str,
    size: f32,
    top_y: f32,
    rtl: bool,
    color: Rgb,
) {
    let upem = face.units_per_em().max(1) as f32;
    let scale = size / upem;
    let ascender = f32::from(face.ascender()) * scale;
    let shaped = shape_line(face, text, rtl);
    let infos = shaped.glyph_infos();
    let positions = shaped.glyph_positions();
    let total_width: f32 = positions.iter().map(|p| (p.x_advance as f32) * scale).sum();
    let mut pen_x = ((pixmap.width() as f32) - total_width) * 0.5;
    let baseline_y = top_y + ascender;

    let mut paint = Paint::default();
    paint.set_color(color.to_color());
    paint.anti_alias = true;

    for (info, pos) in infos.iter().zip(positions.iter()) {
        let glyph_id = rustybuzz::ttf_parser::GlyphId(info.glyph_id as u16);
        let mut outline = SkiaOutline(PathBuilder::new());
        if face.outline_glyph(glyph_id, &mut outline).is_some()
            && let Some(path) = outline.0.finish()
        {
            let gx = pen_x + (pos.x_offset as f32) * scale;
            let gy = baseline_y - (pos.y_offset as f32) * scale;
            // Font coordinates are Y-up; pixmap coordinates are Y-down.
            let transform = Transform::from_row(scale, 0.0, 0.0, -scale, gx, gy);
            pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
        }
        pen_x += (pos.x_advance as f32) * scale;
    }
}

/// Renders `input` to a PNG byte buffer using the supplied TTF font bytes.
pub fn render_verse_card(
    input: &CardInput,
    arabic_font: &[u8],
    latin_font: &[u8],
) -> Result<Vec<u8>, CardError> {
    let arabic = Face::from_slice(arabic_font, 0).ok_or(CardError::InvalidFont("arabic"))?;
    let latin = Face::from_slice(latin_font, 0).ok_or(CardError::InvalidFont("latin"))?;

    let layout = layout_card(input, |face, size, rtl, text| {
        let f = match face {
            CardFace::Arabic => &arabic,
            CardFace::Latin => &latin,
        };
        measure_line(f, size, rtl, text)
    });

    // Guard against absurdly tall cards (e.g. 50 very long ayahs would exceed
    // reasonable PNG memory limits if unbounded; cap matches browser canvas).
    if layout.width == 0 || layout.height == 0 || layout.height > 16_384 {
        return Err(CardError::InvalidDimensions);
    }

    let mut pixmap =
        Pixmap::new(layout.width, layout.height).ok_or(CardError::InvalidDimensions)?;
    pixmap.fill(PALETTE_BG.to_color());

    for line in &layout.lines {
        match line {
            CardLine::Rule { y, color } => {
                if let Some(rect) =
                    Rect::from_xywh(PADDING, *y, (layout.width as f32) - PADDING * 2.0, 2.0)
                {
                    let mut paint = Paint::default();
                    paint.set_color(color.to_color());
                    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
                }
            }
            CardLine::Text {
                text,
                face,
                size,
                y,
                rtl,
                color,
            } => {
                let f = match face {
                    CardFace::Arabic => &arabic,
                    CardFace::Latin => &latin,
                };
                draw_text_line(&mut pixmap, f, text, *size, *y, *rtl, *color);
            }
        }
    }

    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, layout.width, layout.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| CardError::Encode(e.to_string()))?;
        writer
            .write_image_data(pixmap.data())
            .map_err(|e| CardError::Encode(e.to_string()))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arabic_script_detection_matches_web() {
        assert!(is_arabic_script("بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ"));
        assert!(!is_arabic_script("In the name of Allah"));
        assert!(is_arabic_script("EasyQuran إيزي قرآن"));
    }

    #[test]
    fn wrap_words_is_greedy_and_keeps_oversized_words() {
        let words = ["one", "two", "superlongword", "three", "four"];
        let lines = wrap_words(&words, 8.0, |s| s.len() as f32);
        assert_eq!(lines, vec!["one two", "superlongword", "three", "four"]);
    }

    #[test]
    fn render_verse_card_emits_valid_png_with_painted_glyphs() {
        let fonts_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
        let arabic = std::fs::read(fonts_dir.join("uthmanic-hafs-v22.ttf")).expect("hafs font");
        let latin = std::fs::read(fonts_dir.join("Onest.ttf")).expect("onest font");

        let input = CardInput {
            verses: vec![CardVerse {
                text: "بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ".into(),
                reference: "Al-Fatihah 1:1".into(),
            }],
            include_reference: true,
            link: Some("https://easyquran.fyi/al-fatihah?ayah=1:1".into()),
            brand: "EasyQuran".into(),
            footer: "Read and listen at easyquran.fyi".into(),
        };

        let bytes = render_verse_card(&input, &arabic, &latin).expect("render png");
        // PNG signature
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");

        // Decode back and verify dimensions and non-background ink pixels.
        let decoder = png::Decoder::new(std::io::Cursor::new(&bytes));
        let mut reader = decoder.read_info().expect("png header");
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("png frame");
        assert_eq!(info.width, CARD_WIDTH);
        assert!(info.height > 400);

        let non_bg = buf[..info.buffer_size()]
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[0] != PALETTE_BG.0 || px[1] != PALETTE_BG.1 || px[2] != PALETTE_BG.2)
            .count();
        assert!(non_bg > 5_000, "expected painted glyphs, got {non_bg} px");
    }
}
