//! HarfRust-shaped RTL paragraph painted through GPUI's public `paint_glyph`.
//!
//! GPUI on macOS shapes with CoreText, which only partly applies the chaining
//! contexts some Nastaliq faces depend on. This element shapes with HarfRust
//! (HarfBuzz port) and uses GPUI only to rasterize glyph ids, so every platform
//! gets the same shaping result. CoreText, DirectWrite and cosmic-text are all
//! bypassed for layout.
//!
//! Prototype limits: whole paragraph is one RTL Arabic run (no bidi pass, so
//! embedded Latin or ASCII digits would be misordered), no text selection, no
//! caret mapping, no accessibility text.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::{
    App, AvailableSpace, Bounds, Element, ElementId, Font, GlobalElementId, GlyphId, Hsla,
    InspectorElementId, IntoElement, LayoutId, Pixels, SharedString, Size, Style, Window, point,
    px, relative, size,
};
use harfrust::{Direction, FontRef, Language, ShapeOptions, ShaperData, UnicodeBuffer, script};

#[derive(Clone, Copy)]
struct PlacedGlyph {
    id: u32,
    x_advance: f32,
    x_offset: f32,
    y_offset: f32,
}

struct Run {
    glyphs: Vec<PlacedGlyph>,
    width_units: f32,
}

pub struct ShapingFont {
    bytes: &'static [u8],
    font: Font,
    data: ShaperData,
    upem: f32,
    cache: RefCell<HashMap<String, Rc<Run>>>,
}

impl ShapingFont {
    pub fn new(bytes: &'static [u8], font: Font) -> Rc<Self> {
        let face = FontRef::new(bytes).expect("parse font for HarfRust");
        let data = ShaperData::new(&face);
        let upem = data.shaper(&face).build().units_per_em() as f32;
        Rc::new(Self {
            bytes,
            font,
            data,
            upem,
            cache: RefCell::default(),
        })
    }

    fn shape(&self, text: &str) -> Rc<Run> {
        if let Some(run) = self.cache.borrow().get(text) {
            return Rc::clone(run);
        }
        let face = FontRef::new(self.bytes).expect("parse font for HarfRust");
        let shaper = self.data.shaper(&face).build();
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.set_direction(Direction::RightToLeft);
        buffer.set_script(script::ARABIC);
        buffer.set_language("ar".parse::<Language>().expect("valid language tag"));
        let shaped = shaper.shape(buffer, ShapeOptions::new());
        let glyphs: Vec<PlacedGlyph> = shaped
            .glyph_infos()
            .iter()
            .zip(shaped.glyph_positions())
            .map(|(info, pos)| PlacedGlyph {
                id: info.glyph_id,
                x_advance: pos.x_advance as f32,
                x_offset: pos.x_offset as f32,
                y_offset: pos.y_offset as f32,
            })
            .collect();
        let width_units = glyphs.iter().map(|g| g.x_advance).sum();
        let run = Rc::new(Run {
            glyphs,
            width_units,
        });
        self.cache
            .borrow_mut()
            .insert(text.to_owned(), Rc::clone(&run));
        run
    }

    /// Greedy wrap on spaces, measuring each candidate line as really shaped.
    fn wrap(&self, text: &str, font_size: f32, max_width: f32) -> Vec<Rc<Run>> {
        let limit_units = max_width * self.upem / font_size;
        let mut lines = Vec::new();
        let mut current = String::new();
        for word in text.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_owned()
            } else {
                format!("{current} {word}")
            };
            if current.is_empty() || self.shape(&candidate).width_units <= limit_units {
                current = candidate;
            } else {
                lines.push(self.shape(&current));
                current = word.to_owned();
            }
        }
        if !current.is_empty() {
            lines.push(self.shape(&current));
        }
        lines
    }
}

pub struct ShapedParagraph {
    pub text: SharedString,
    pub font: Rc<ShapingFont>,
    pub font_size: f32,
    pub line_height: f32,
    pub color: Hsla,
}

impl IntoElement for ShapedParagraph {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ShapedParagraph {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let text = self.text.clone();
        let font = Rc::clone(&self.font);
        let font_size = self.font_size;
        let line_height = self.line_height;
        let layout_id = window.request_measured_layout(
            style,
            move |known: Size<Option<Pixels>>, available: Size<AvailableSpace>, _, _| {
                let width = known.width.unwrap_or(match available.width {
                    AvailableSpace::Definite(width) => width,
                    AvailableSpace::MinContent | AvailableSpace::MaxContent => px(f32::MAX),
                });
                let lines = font.wrap(&text, font_size, f32::from(width));
                size(width, px(lines.len() as f32 * line_height))
            },
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let font_id = window.text_system().resolve_font(&self.font.font);
        let font_px = px(self.font_size);
        let ascent = f32::from(window.text_system().ascent(font_id, font_px));
        let descent = f32::from(window.text_system().descent(font_id, font_px)).abs();
        let scale = self.font_size / self.font.upem;
        let baseline = (self.line_height - ascent - descent) / 2. + ascent;
        let mask = window.content_mask().bounds;
        let (mask_top, mask_bottom) = (
            f32::from(mask.origin.y),
            f32::from(mask.origin.y + mask.size.height),
        );
        let left = f32::from(bounds.origin.x);
        let right = left + f32::from(bounds.size.width);
        let top = f32::from(bounds.origin.y);

        let lines = self
            .font
            .wrap(&self.text, self.font_size, f32::from(bounds.size.width));
        for (ix, line) in lines.iter().enumerate() {
            let line_top = top + ix as f32 * self.line_height;
            // Nastaliq ink overflows its line box, so cull with a line of slack.
            if line_top + 2. * self.line_height < mask_top
                || line_top - self.line_height > mask_bottom
            {
                continue;
            }
            let y = line_top + baseline;
            let mut pen = right - line.width_units * scale;
            for glyph in &line.glyphs {
                // HarfBuzz y grows up, GPUI y grows down.
                let origin = point(
                    px(pen + glyph.x_offset * scale),
                    px(y - glyph.y_offset * scale),
                );
                window
                    .paint_glyph(origin, font_id, GlyphId(glyph.id), font_px, self.color)
                    .ok();
                pen += glyph.x_advance * scale;
            }
        }
    }
}
