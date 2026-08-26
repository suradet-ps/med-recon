//! A4 PDF report engine for the medication history export.
//!
//! Pure-Rust PDF pipeline - no system fonts, no browser, byte-identical
//! output on every OS:
//!
//! - Sarabun Regular/Bold TTFs (the UI's own font family, OFL licensed)
//!   are embedded via `include_bytes!` and shipped inside the PDF itself.
//! - Thai text is shaped with `rustybuzz` (a HarfBuzz port) so vowels and
//!   tone marks compose onto their consonants; glyphs are then placed one
//!   by one in the content stream (no PDF library performs shaping).
//! - Layout is fixed to A4 (595.28 × 841.89 pt) with standard margins;
//!   content flows across pages, each carrying the PHI footer.
//!
//! The pipeline has two stages:
//! 1. [`layout`] - wraps text (cluster-aware, so Thai runs break only
//!    between clusters) and packs the report model into per-page command
//!    lists. Pure and fully unit-testable.
//! 2. [`write_pdf`] - turns command lists into PDF bytes via `pdf-writer`
//!    (Type0/CID fonts, width arrays, ToUnicode cmap for text extraction).

use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};
use rustybuzz::{Direction, Face as HbFace, UnicodeBuffer, shape as hb_shape};
use ttf_parser::Face as TtfFace;

use crate::report::{AllergyEntry, MedItem, MedSection, ReportModel};

// ---------------------------------------------------------------- geometry

/// A4 page width in points (210 mm).
pub const PAGE_W: f32 = 595.28;
/// A4 page height in points (297 mm).
pub const PAGE_H: f32 = 841.89;

/// Side margin - content never starts closer than 45 pt (≈ 16 mm) to the
/// paper edge.
const MARGIN: f32 = 45.0;
/// Top margin for the first content row.
const MARGIN_TOP: f32 = 52.0;
/// Content stops 56 pt above the paper bottom; the footer (PHI notice,
/// version, page number) lives in the strip below.
const CONTENT_BOTTOM: f32 = 56.0;
/// Usable content width between the side margins.
const CONTENT_W: f32 = PAGE_W - 2.0 * MARGIN;

/// Vertical spacing below a section heading.
const SECTION_STEP: f32 = 22.0;
/// Line height factor - Thai script needs more leading than Latin.
fn line_step(size: f32) -> f32 {
    size * 1.45
}

// ---------------------------------------------------------------- colors

// App design tokens (see style.css) as RGB 0-1.
const HOUSE: [f32; 3] = [0.118, 0.224, 0.196]; // #1E3932
const BRAND: [f32; 3] = [0.000, 0.459, 0.290]; // #00754A
const CANVAS: [f32; 3] = [0.949, 0.941, 0.922]; // #F2F0EB
const RED: [f32; 3] = [0.784, 0.125, 0.078]; // #C82014
const RED_BG: [f32; 3] = [0.992, 0.953, 0.953]; // #FDF3F2
const AMBER: [f32; 3] = [0.541, 0.427, 0.000]; // #8A6D00
const AMBER_BG: [f32; 3] = [0.980, 0.965, 0.910]; // #FAF6E8
const TEXT: [f32; 3] = [0.133, 0.133, 0.133]; // #222222
const MUTED: [f32; 3] = [0.420, 0.420, 0.420]; // #6B6B6B
const BORDER: [f32; 3] = [0.906, 0.906, 0.906]; // #E7E7E7
const WHITE: [f32; 3] = [1.0, 1.0, 1.0];
const WHITE_SOFT: [f32; 3] = [0.760, 0.800, 0.780]; // white at ~70% on the house green

// ---------------------------------------------------------------- fonts

/// Embedded Sarabun font assets (the UI's font family - OFL 1.1, see
/// `assets/fonts/OFL.txt`). `include_bytes!` keeps the report byte-identical
/// on every OS and avoids a runtime resource lookup.
const SARABUN_REGULAR: &[u8] = include_bytes!("../assets/fonts/Sarabun-Regular.ttf");
const SARABUN_BOLD: &[u8] = include_bytes!("../assets/fonts/Sarabun-Bold.ttf");

/// Text weight role - selects one of the embedded Sarabun faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FontRole {
    Regular,
    Bold,
}

/// The two embedded Sarabun faces. Each weight holds a `ttf-parser` face
/// (metrics, glyph coverage) and a `rustybuzz` face (shaping).
pub(crate) struct Fonts {
    regular: TtfFace<'static>,
    bold: TtfFace<'static>,
    hb_regular: HbFace<'static>,
    hb_bold: HbFace<'static>,
}

impl Default for Fonts {
    fn default() -> Self {
        Self::new()
    }
}

impl Fonts {
    /// Load the bundled Sarabun Regular/Bold faces.
    ///
    /// # Panics
    ///
    /// Only if an embedded font is corrupt - impossible in practice since
    /// the bytes are compile-time constants covered by
    /// [`font_assets_are_valid`].
    pub(crate) fn new() -> Self {
        Self::load(SARABUN_REGULAR, SARABUN_BOLD)
    }

    fn load(regular: &'static [u8], bold: &'static [u8]) -> Self {
        Self {
            regular: TtfFace::parse(regular, 0)
                .expect("invariant: bundled Sarabun-Regular.ttf parses"),
            bold: TtfFace::parse(bold, 0).expect("invariant: bundled Sarabun-Bold.ttf parses"),
            hb_regular: HbFace::from_slice(regular, 0)
                .expect("invariant: bundled Sarabun-Regular.ttf parses"),
            hb_bold: HbFace::from_slice(bold, 0)
                .expect("invariant: bundled Sarabun-Bold.ttf parses"),
        }
    }

    fn face(&self, role: FontRole) -> &TtfFace<'static> {
        match role {
            FontRole::Regular => &self.regular,
            FontRole::Bold => &self.bold,
        }
    }

    fn hb_face(&self, role: FontRole) -> &HbFace<'static> {
        match role {
            FontRole::Regular => &self.hb_regular,
            FontRole::Bold => &self.hb_bold,
        }
    }

    fn upem(&self, role: FontRole) -> f32 {
        self.face(role).units_per_em() as f32
    }

    /// Whether `role` can render `ch` (used to strip characters the
    /// embedded fonts cannot draw, e.g. emoji in HOSxP free text).
    fn covers(&self, role: FontRole, ch: char) -> bool {
        self.face(role).glyph_index(ch).is_some_and(|g| g.0 != 0)
    }

    fn pdf_name(&self, role: FontRole) -> Name<'static> {
        match role {
            FontRole::Regular => Name(b"Sarabun"),
            FontRole::Bold => Name(b"Sarabun-Bold"),
        }
    }
}

// ---------------------------------------------------------------- shaping

/// One placed glyph of a shaped run - font glyph id plus displacement in
/// points (font units scaled by `size / upem`).
#[derive(Debug, Clone, Copy)]
struct Glyph {
    gid: u16,
    x_adv: f32,
    x_off: f32,
    y_off: f32,
}

/// A shaped text run: glyphs plus the byte offset of each glyph's cluster
/// in the source text (HarfBuzz clusters never split a Thai consonant +
/// vowel + mark group, which is what makes cluster-aware line breaking
/// safe).
#[derive(Debug)]
struct ShapedRun {
    glyphs: Vec<Glyph>,
    clusters: Vec<usize>,
}

impl ShapedRun {
    fn width(&self) -> f32 {
        self.glyphs.iter().map(|g| g.x_adv).sum()
    }
}

/// Shape `text` with HarfBuzz, scaling font units to points. Glyphs the
/// font cannot represent (`gid 0`) are dropped.
fn shape(fonts: &Fonts, role: FontRole, size: f32, text: &str) -> ShapedRun {
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.set_direction(Direction::LeftToRight);
    buffer.set_language(
        "th".parse()
            .expect("invariant: \"th\" is a valid language tag"),
    );
    buffer.guess_segment_properties();
    let out = hb_shape(fonts.hb_face(role), &[], buffer);
    let scale = size / fonts.upem(role);
    let (glyphs, clusters) = out
        .glyph_infos()
        .iter()
        .zip(out.glyph_positions())
        .filter_map(|(info, pos)| {
            if info.glyph_id == 0 {
                return None;
            }
            Some((
                Glyph {
                    gid: info.glyph_id as u16,
                    x_adv: pos.x_advance as f32 * scale,
                    x_off: pos.x_offset as f32 * scale,
                    y_off: pos.y_offset as f32 * scale,
                },
                info.cluster as usize,
            ))
        })
        .unzip();
    ShapedRun { glyphs, clusters }
}

/// Measure a text run in points.
fn measure(fonts: &Fonts, role: FontRole, size: f32, text: &str) -> f32 {
    shape(fonts, role, size, text).width()
}

/// Strip characters the embedded fonts cannot render and normalize
/// whitespace: tabs/CR become spaces (paragraph breaks are handled by the
/// caller via `\n`), control characters are dropped, and unsupported
/// symbols (emoji etc.) are removed rather than drawn as `.notdef` boxes.
fn sanitize(fonts: &Fonts, role: FontRole, text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\t' | '\r' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .filter(|c| fonts.covers(role, *c))
        .collect()
}

// ---------------------------------------------------------------- wrapping

/// Greedy wrap of `text` into lines of at most `max_w` points. Words are
/// split at whitespace; a word that does not fit even alone is broken at
/// the longest cluster boundary that fits, so Thai vowels and marks stay
/// attached to their consonant.
fn wrap(fonts: &Fonts, role: FontRole, size: f32, text: &str, max_w: f32) -> Vec<String> {
    let text = sanitize(fonts, role, text);
    let mut lines: Vec<Vec<String>> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    let mut cur_w = 0.0_f32;
    let space_w = measure(fonts, FontRole::Regular, size, " ");
    let start_line = |word: &str,
                      word_w: f32,
                      lines: &mut Vec<Vec<String>>,
                      cur: &mut Vec<String>,
                      cur_w: &mut f32| {
        if word_w <= max_w {
            cur.push(word.to_string());
            *cur_w = word_w;
        } else {
            for chunk in break_word(fonts, role, size, word, max_w) {
                let chunk_w = measure(fonts, role, size, &chunk);
                if cur.is_empty() {
                    cur.push(chunk);
                    *cur_w = chunk_w;
                } else {
                    lines.push(std::mem::take(cur));
                    cur.push(chunk);
                    *cur_w = chunk_w;
                }
            }
        }
    };
    for word in text.split_whitespace() {
        let word_w = measure(fonts, role, size, word);
        if cur.is_empty() {
            start_line(word, word_w, &mut lines, &mut cur, &mut cur_w);
        } else if cur_w + space_w + word_w <= max_w {
            cur.push(word.to_string());
            cur_w += space_w + word_w;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur_w = 0.0;
            start_line(word, word_w, &mut lines, &mut cur, &mut cur_w);
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines.into_iter().map(|l| l.join(" ")).collect()
}

/// Break `word` into the longest prefixes that fit `max_w`, walking the
/// shaped cluster boundaries of the whole word at once.
fn break_word(fonts: &Fonts, role: FontRole, size: f32, word: &str, max_w: f32) -> Vec<String> {
    let mut rest = word;
    let mut chunks = Vec::new();
    while !rest.is_empty() {
        let shaped = shape(fonts, role, size, rest);
        // `consumed` = byte offset just past the last cluster that fits.
        // HarfBuzz gives every glyph of a Thai grapheme the same cluster
        // (the base consonant's offset), so cutting at the *next* cluster's
        // start never splits a consonant + vowel + mark group.
        let mut w = 0.0_f32;
        let mut consumed = 0usize;
        for (i, g) in shaped.glyphs.iter().enumerate() {
            if w + g.x_adv > max_w && consumed > 0 {
                break;
            }
            w += g.x_adv;
            consumed = shaped.clusters.get(i + 1).copied().unwrap_or(rest.len());
        }
        if consumed == 0 {
            // Degenerate: the first cluster alone exceeds the line - take
            // the first character whole so we always make progress.
            consumed = rest.chars().next().map_or(0, char::len_utf8);
        } else if let Some(punct) = rest[..consumed]
            .char_indices()
            .rev()
            .find_map(|(i, c)| matches!(c, '/' | '-' | '.' | '(' | ')').then_some(i + c.len_utf8()))
        {
            // Prefer breaking right after a separator inside the word
            // (e.g. "drugusage/sp_use" -> "drugusage/" + "sp_use") before
            // falling back to a character break.
            consumed = punct;
        }
        if consumed >= rest.len() {
            chunks.push(rest.to_string());
            break;
        }
        let (head, tail) = rest.split_at(consumed);
        chunks.push(head.to_string());
        rest = tail;
    }
    chunks
}

// ---------------------------------------------------------------- commands

/// One drawing operation - the intermediate representation between layout
/// and the PDF writer. All coordinates are in points from the page's
/// bottom-left origin (PDF convention), with `y` being the baseline for
/// [`Cmd::Text`].
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Cmd {
    /// Filled rounded rectangle; `stroke` draws an optional border.
    RoundedRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        fill: [f32; 3],
        stroke: Option<([f32; 3], f32)>,
    },
    /// A single text run at its baseline position.
    Text {
        x: f32,
        y: f32,
        size: f32,
        role: FontRole,
        color: [f32; 3],
        text: String,
    },
}

/// Layout state: the current page's command list and the vertical cursor.
struct Layout<'m> {
    model: &'m ReportModel,
    fonts: &'m Fonts,
    pages: Vec<Vec<Cmd>>,
    cursor: f32,
}

/// Lay the report model out on A4 pages. Returns one command list per
/// page; footer page numbers still carry `{page}`/`{total}` placeholders
/// until [`finalize_pages`] runs (the count is only known afterwards).
pub(crate) fn layout(model: &ReportModel, fonts: &Fonts) -> Vec<Vec<Cmd>> {
    let mut lo = Layout {
        model,
        fonts,
        pages: Vec::new(),
        cursor: MARGIN_TOP,
    };
    lo.push_page();

    lo.disclaimer();
    lo.patient_card();
    lo.warnings();
    lo.allergies();
    for section in &model.sections {
        lo.med_section(section);
    }
    lo.visits();
    lo.pages
}

/// Replace the `{page}` / `{total}` placeholders in every page footer.
pub(crate) fn finalize_pages(pages: &mut [Vec<Cmd>]) {
    let total = pages.len();
    for (i, page) in pages.iter_mut().enumerate() {
        for cmd in page.iter_mut() {
            if let Cmd::Text { text, .. } = cmd
                && text.contains("{page}")
            {
                *text = text
                    .replace("{page}", &(i + 1).to_string())
                    .replace("{total}", &total.to_string());
            }
        }
    }
}

impl Layout<'_> {
    /// Start a new page: header band (full on page 1, slim on later pages)
    /// plus the shared footer, and reset the cursor.
    fn push_page(&mut self) {
        self.pages.push(Vec::new());
        let first = self.pages.len() == 1;
        let page = self.pages.last_mut().expect("invariant: page just pushed");
        if first {
            header_band(page, self.model, self.fonts);
            self.cursor = MARGIN_TOP + 64.0 + 14.0;
        } else {
            slim_header(page, self.model, self.fonts);
            self.cursor = MARGIN_TOP + 34.0 + 12.0;
        }
        footer(page, self.model, self.fonts);
    }

    /// Open a new page if `height` does not fit below the cursor.
    fn ensure(&mut self, height: f32) {
        if self.cursor + height > PAGE_H - CONTENT_BOTTOM {
            self.push_page();
        }
    }

    /// Disclaimer band: a house-green box under the header.
    fn disclaimer(&mut self) {
        let size = 9.5;
        let lines = wrap(
            self.fonts,
            FontRole::Regular,
            size,
            &self.model.disclaimer,
            CONTENT_W - 20.0,
        );
        let height = 10.0 + lines.len() as f32 * line_step(size) + 10.0;
        self.ensure(height);
        let page = self.pages.last_mut().expect("invariant: page exists");
        page.push(Cmd::RoundedRect {
            x: MARGIN,
            y: self.cursor,
            w: CONTENT_W,
            h: height,
            r: 8.0,
            fill: HOUSE,
            stroke: None,
        });
        for (i, line) in lines.iter().enumerate() {
            page.push(Cmd::Text {
                x: MARGIN + 10.0,
                y: self.cursor + 10.0 + i as f32 * line_step(size) + size,
                size,
                role: FontRole::Regular,
                color: WHITE_SOFT,
                text: line.clone(),
            });
        }
        self.cursor += height + 14.0;
    }

    /// Patient identity card on a light canvas background.
    fn patient_card(&mut self) {
        let name_size = 12.0;
        let meta_size = 9.5;
        let height = 12.0 + line_step(name_size) + line_step(meta_size) + 12.0;
        self.ensure(height);
        let page = self.pages.last_mut().expect("invariant: page exists");
        page.push(Cmd::RoundedRect {
            x: MARGIN,
            y: self.cursor,
            w: CONTENT_W,
            h: height,
            r: 8.0,
            fill: CANVAS,
            stroke: None,
        });
        page.push(Cmd::Text {
            x: MARGIN + 14.0,
            y: self.cursor + 12.0 + name_size,
            size: name_size,
            role: FontRole::Bold,
            color: TEXT,
            text: self.model.patient_name.clone(),
        });
        page.push(Cmd::Text {
            x: MARGIN + 14.0,
            y: self.cursor + 12.0 + line_step(name_size) + meta_size,
            size: meta_size,
            role: FontRole::Regular,
            color: MUTED,
            text: self.model.patient_meta.clone(),
        });
        self.cursor += height + 14.0;
    }

    /// Amber data-completeness warnings box.
    fn warnings(&mut self) {
        if self.model.warnings.is_empty() {
            return;
        }
        let title_size = 10.5;
        let body_size = 9.5;
        let mut height = 10.0;
        height += line_step(title_size);
        let warn_lines: usize = self
            .model
            .warnings
            .iter()
            .map(|w| {
                wrap(
                    self.fonts,
                    FontRole::Regular,
                    body_size,
                    w,
                    CONTENT_W - 20.0,
                )
                .len()
            })
            .sum();
        height += warn_lines as f32 * line_step(body_size) + 10.0;
        self.ensure(height);
        let page = self.pages.last_mut().expect("invariant: page exists");
        page.push(Cmd::RoundedRect {
            x: MARGIN,
            y: self.cursor,
            w: CONTENT_W,
            h: height,
            r: 8.0,
            fill: AMBER_BG,
            stroke: None,
        });
        page.push(Cmd::Text {
            x: MARGIN + 10.0,
            y: self.cursor + 10.0 + title_size,
            size: title_size,
            role: FontRole::Bold,
            color: AMBER,
            text: self.model.warnings_title.clone(),
        });
        let mut y = self.cursor + 10.0 + line_step(title_size) + body_size;
        for w in &self.model.warnings {
            for line in wrap(
                self.fonts,
                FontRole::Regular,
                body_size,
                w,
                CONTENT_W - 20.0,
            ) {
                page.push(Cmd::Text {
                    x: MARGIN + 10.0,
                    y,
                    size: body_size,
                    role: FontRole::Regular,
                    color: AMBER,
                    text: line,
                });
                y += line_step(body_size);
            }
        }
        self.cursor += height + 14.0;
    }

    /// Red allergy boxes.
    fn allergies(&mut self) {
        self.section_heading(&self.model.allergy_title);
        for allergy in &self.model.allergies {
            self.allergy_box(allergy);
        }
        self.cursor += 10.0;
    }

    fn allergy_box(&mut self, allergy: &AllergyEntry) {
        let agent_size = 10.5;
        let detail_size = 9.0;
        let pad = 8.0;
        let mut height = pad;
        height += line_step(agent_size);
        let detail_lines = allergy
            .detail
            .as_deref()
            .map(|d| {
                wrap(
                    self.fonts,
                    FontRole::Regular,
                    detail_size,
                    d,
                    CONTENT_W - 16.0,
                )
                .len()
            })
            .unwrap_or(0) as f32;
        height += detail_lines * line_step(detail_size) + pad;
        self.ensure(height);
        let page = self.pages.last_mut().expect("invariant: page exists");
        page.push(Cmd::RoundedRect {
            x: MARGIN,
            y: self.cursor,
            w: CONTENT_W,
            h: height,
            r: 8.0,
            fill: RED_BG,
            stroke: Some((RED, 1.0)),
        });
        page.push(Cmd::Text {
            x: MARGIN + 8.0,
            y: self.cursor + pad + agent_size,
            size: agent_size,
            role: FontRole::Bold,
            color: RED,
            text: allergy.agent.clone(),
        });
        let mut y = self.cursor + pad + line_step(agent_size) + detail_size;
        if let Some(detail) = &allergy.detail {
            for line in wrap(
                self.fonts,
                FontRole::Regular,
                detail_size,
                detail,
                CONTENT_W - 16.0,
            ) {
                page.push(Cmd::Text {
                    x: MARGIN + 8.0,
                    y,
                    size: detail_size,
                    role: FontRole::Regular,
                    color: RED,
                    text: line,
                });
                y += line_step(detail_size);
            }
        }
        self.cursor += height + 8.0;
    }

    /// Green section heading with the brand accent bar.
    fn section_heading(&mut self, title: &str) {
        self.ensure(SECTION_STEP);
        let page = self.pages.last_mut().expect("invariant: page exists");
        page.push(Cmd::RoundedRect {
            x: MARGIN,
            y: self.cursor + 1.0,
            w: 3.5,
            h: 16.0,
            r: 1.75,
            fill: BRAND,
            stroke: None,
        });
        page.push(Cmd::Text {
            x: MARGIN + 12.0,
            y: self.cursor + 15.0,
            size: 13.0,
            role: FontRole::Bold,
            color: BRAND,
            text: title.to_string(),
        });
        self.cursor += SECTION_STEP;
    }

    /// One medication section: heading + separated item rows.
    fn med_section(&mut self, section: &MedSection) {
        self.section_heading(&section.title);
        for (i, item) in section.items.iter().enumerate() {
            self.med_item(item, i == 0);
        }
        self.cursor += 10.0;
    }

    /// One medication row: title (bold name + muted strength/units), meta
    /// line with the days-supply chip on the right, and the sig in brand
    /// green.
    fn med_item(&mut self, item: &MedItem, first: bool) {
        let title_size = 10.5;
        let meta_size = 9.0;
        let sig_size = 9.0;
        let title_step = line_step(title_size);

        // Title: the bold drug name, then the muted strength/units either
        // on the same line (when it fits) or on its own wrapped lines.
        let name_lines = wrap(
            self.fonts,
            FontRole::Bold,
            title_size,
            &item.title,
            CONTENT_W,
        );
        let mut title_lines: Vec<(String, FontRole)> = name_lines
            .into_iter()
            .map(|l| (l, FontRole::Bold))
            .collect();
        if let Some(sub) = &item.sub {
            let sub_w = measure(self.fonts, FontRole::Regular, title_size, sub);
            let first_w = measure(self.fonts, FontRole::Bold, title_size, &title_lines[0].0);
            if title_lines.len() == 1 && first_w + sub_w + 6.0 <= CONTENT_W {
                let mut merged = title_lines[0].0.clone();
                merged.push(' ');
                merged.push_str(sub);
                title_lines[0] = (merged, FontRole::Bold);
            } else {
                title_lines.extend(
                    wrap(self.fonts, FontRole::Regular, title_size, sub, CONTENT_W)
                        .into_iter()
                        .map(|l| (l, FontRole::Regular)),
                );
            }
        }

        // Meta line, shortened when the chip needs the right edge.
        let chip = item.chip.as_deref().map(|c| {
            let w = measure(self.fonts, FontRole::Bold, 8.0, c) + 14.0;
            (c.to_string(), w)
        });
        let meta_w = match &chip {
            Some((_, w)) => CONTENT_W - w - 10.0,
            None => CONTENT_W,
        };
        let meta_lines: Vec<String> = item
            .meta
            .as_deref()
            .map(|m| wrap(self.fonts, FontRole::Regular, meta_size, m, meta_w))
            .unwrap_or_default();
        let sig_lines: Vec<String> = item
            .sig
            .as_deref()
            .map(|s| wrap(self.fonts, FontRole::Regular, sig_size, s, CONTENT_W))
            .unwrap_or_default();

        let height = 8.0
            + if first { 0.0 } else { 1.0 + 7.0 }
            + title_lines.len() as f32 * title_step
            + 3.0
            + meta_lines.len() as f32 * line_step(meta_size)
            + if sig_lines.is_empty() {
                0.0
            } else {
                3.0 + sig_lines.len() as f32 * line_step(sig_size)
            }
            + 8.0;
        self.ensure(height);
        let page = self.pages.last_mut().expect("invariant: page exists");

        if !first {
            page.push(Cmd::RoundedRect {
                x: MARGIN,
                y: self.cursor,
                w: CONTENT_W,
                h: 1.0,
                r: 0.0,
                fill: BORDER,
                stroke: None,
            });
            self.cursor += 8.0;
        } else {
            self.cursor += 8.0;
        }

        let mut y = self.cursor + title_size;
        for (text, role) in &title_lines {
            page.push(Cmd::Text {
                x: MARGIN,
                y,
                size: title_size,
                role: *role,
                color: TEXT,
                text: text.clone(),
            });
            y += title_step;
        }
        self.cursor = y - title_step + 3.0;
        if !meta_lines.is_empty() {
            let meta_baseline = self.cursor + meta_size;
            for (i, line) in meta_lines.iter().enumerate() {
                page.push(Cmd::Text {
                    x: MARGIN,
                    y: meta_baseline + i as f32 * line_step(meta_size),
                    size: meta_size,
                    role: FontRole::Regular,
                    color: MUTED,
                    text: line.clone(),
                });
            }
            if let Some((chip_text, chip_w)) = &chip {
                let chip_h = 12.0;
                let chip_y = self.cursor + (line_step(meta_size) - chip_h) / 2.0;
                page.push(Cmd::RoundedRect {
                    x: MARGIN + CONTENT_W - chip_w,
                    y: chip_y,
                    w: *chip_w,
                    h: chip_h,
                    r: chip_h / 2.0,
                    fill: WHITE,
                    stroke: Some((BRAND, 1.0)),
                });
                let text_w = measure(self.fonts, FontRole::Bold, 8.0, chip_text);
                page.push(Cmd::Text {
                    x: MARGIN + CONTENT_W - chip_w + (chip_w - text_w) / 2.0,
                    y: chip_y + chip_h / 2.0 + 4.0,
                    size: 8.0,
                    role: FontRole::Bold,
                    color: BRAND,
                    text: chip_text.clone(),
                });
            }
            self.cursor += meta_lines.len() as f32 * line_step(meta_size);
        }
        if !sig_lines.is_empty() {
            self.cursor += 3.0;
            for line in &sig_lines {
                page.push(Cmd::Text {
                    x: MARGIN,
                    y: self.cursor + sig_size,
                    size: sig_size,
                    role: FontRole::Regular,
                    color: BRAND,
                    text: line.clone(),
                });
                self.cursor += line_step(sig_size);
            }
        }
        self.cursor += 8.0;
    }

    /// Visit history table with a repeated header row on page breaks.
    fn visits(&mut self) {
        self.section_heading(&self.model.visits_title);
        let widths = [78.0, 42.0, CONTENT_W - 78.0 - 42.0 - 92.0, 92.0];
        let row_top = |y: f32, h: f32| y + h - 4.5;
        let header_h = 22.0;
        let mut first = true;
        for row in &self.model.visits {
            let dept_lines = wrap(self.fonts, FontRole::Regular, 9.0, &row[2], widths[2]);
            let row_h = (dept_lines.len() as f32 * line_step(9.0) + 6.0).max(18.0);
            if self.cursor + row_h > PAGE_H - CONTENT_BOTTOM {
                self.push_page();
                self.draw_visit_header(&widths, header_h);
                first = false;
            }
            let page = self.pages.last_mut().expect("invariant: page exists");
            if !first {
                page.push(Cmd::RoundedRect {
                    x: MARGIN,
                    y: self.cursor,
                    w: CONTENT_W,
                    h: 1.0,
                    r: 0.0,
                    fill: BORDER,
                    stroke: None,
                });
            }
            let mut x = MARGIN;
            for (i, cell) in row.iter().enumerate() {
                if i == 2 {
                    for (l, line) in dept_lines.iter().enumerate() {
                        page.push(Cmd::Text {
                            x,
                            y: self.cursor + row_h - 6.0 + l as f32 * line_step(9.0),
                            size: 9.0,
                            role: FontRole::Regular,
                            color: TEXT,
                            text: line.clone(),
                        });
                    }
                } else {
                    page.push(Cmd::Text {
                        x,
                        y: self.cursor + row_h - 6.0,
                        size: 9.0,
                        role: FontRole::Regular,
                        color: TEXT,
                        text: cell.clone(),
                    });
                }
                x += widths[i];
            }
            self.cursor += row_h;
            first = false;
            let _ = row_top;
        }
        self.cursor += 10.0;
    }

    fn draw_visit_header(&mut self, widths: &[f32; 4], header_h: f32) {
        let page = self.pages.last_mut().expect("invariant: page exists");
        page.push(Cmd::RoundedRect {
            x: MARGIN,
            y: self.cursor,
            w: CONTENT_W,
            h: header_h,
            r: 4.0,
            fill: CANVAS,
            stroke: None,
        });
        let mut x = MARGIN;
        for (i, header) in self.model.visit_headers.iter().enumerate() {
            page.push(Cmd::Text {
                x,
                y: self.cursor + header_h - 6.5,
                size: 9.0,
                role: FontRole::Bold,
                color: HOUSE,
                text: header.clone(),
            });
            x += widths[i];
        }
        self.cursor += header_h;
    }
}

/// Page-1 header band: dark house-green box with the report heading and
/// the `{site} · {generated}` sub-line.
fn header_band(page: &mut Vec<Cmd>, model: &ReportModel, fonts: &Fonts) {
    page.push(Cmd::RoundedRect {
        x: MARGIN,
        y: MARGIN_TOP,
        w: CONTENT_W,
        h: 64.0,
        r: 10.0,
        fill: HOUSE,
        stroke: None,
    });
    page.push(Cmd::Text {
        x: MARGIN + 22.0,
        y: MARGIN_TOP + 34.0,
        size: 17.0,
        role: FontRole::Bold,
        color: WHITE,
        text: model.heading.clone(),
    });
    page.push(Cmd::Text {
        x: MARGIN + 22.0,
        y: MARGIN_TOP + 54.0,
        size: 9.5,
        role: FontRole::Regular,
        color: WHITE_SOFT,
        text: model.sub_line.clone(),
    });
    let _ = fonts;
}

/// Slim header for continuation pages: heading left, patient name right.
fn slim_header(page: &mut Vec<Cmd>, model: &ReportModel, fonts: &Fonts) {
    page.push(Cmd::RoundedRect {
        x: MARGIN,
        y: MARGIN_TOP,
        w: CONTENT_W,
        h: 34.0,
        r: 8.0,
        fill: HOUSE,
        stroke: None,
    });
    page.push(Cmd::Text {
        x: MARGIN + 16.0,
        y: MARGIN_TOP + 20.0,
        size: 11.0,
        role: FontRole::Bold,
        color: WHITE,
        text: model.heading.clone(),
    });
    let name_w = measure(fonts, FontRole::Regular, 9.0, &model.patient_name);
    page.push(Cmd::Text {
        x: MARGIN + CONTENT_W - 16.0 - name_w,
        y: MARGIN_TOP + 20.0,
        size: 9.0,
        role: FontRole::Regular,
        color: WHITE_SOFT,
        text: model.patient_name.clone(),
    });
}

/// Shared footer: thin rule, PHI notice, version, and `หน้า {page}/{total}`.
fn footer(page: &mut Vec<Cmd>, model: &ReportModel, fonts: &Fonts) {
    page.push(Cmd::RoundedRect {
        x: MARGIN,
        y: 26.0,
        w: CONTENT_W,
        h: 0.75,
        r: 0.0,
        fill: BORDER,
        stroke: None,
    });
    page.push(Cmd::Text {
        x: MARGIN,
        y: 38.0,
        size: 8.0,
        role: FontRole::Regular,
        color: MUTED,
        text: model.version_line.clone(),
    });
    let phi_w = measure(fonts, FontRole::Regular, 7.5, &model.footer_phi);
    page.push(Cmd::Text {
        x: MARGIN + (CONTENT_W - phi_w) / 2.0,
        y: 50.0,
        size: 7.5,
        role: FontRole::Regular,
        color: MUTED,
        text: model.footer_phi.clone(),
    });
    let page_w = measure(fonts, FontRole::Regular, 8.0, &model.page_of);
    page.push(Cmd::Text {
        x: MARGIN + CONTENT_W - page_w,
        y: 38.0,
        size: 8.0,
        role: FontRole::Regular,
        color: MUTED,
        text: model.page_of.clone(),
    });
}

// ---------------------------------------------------------------- rendering

/// Serialize the laid-out pages into a PDF document.
pub(crate) fn write_pdf(pages: &[Vec<Cmd>], fonts: &Fonts) -> Vec<u8> {
    let mut pdf = Pdf::new();
    let catalog_id = Ref::new(1);
    let pages_id = Ref::new(2);
    let n = pages.len();
    let page_ids: Vec<Ref> = (0..n).map(|i| Ref::new(3 + 2 * i as i32)).collect();
    let content_ids: Vec<Ref> = (0..n).map(|i| Ref::new(4 + 2 * i as i32)).collect();
    let font_base = (3 + 2 * n) as i32;

    pdf.catalog(catalog_id).pages(pages_id);
    pdf.pages(pages_id).kids(page_ids.clone()).count(n as i32);
    for (i, page_id) in page_ids.iter().enumerate() {
        let mut page = pdf.page(*page_id);
        page.media_box(Rect::new(0.0, 0.0, PAGE_W, PAGE_H));
        page.parent(pages_id);
        page.contents(content_ids[i]);
        page.resources()
            .fonts()
            .pair(Name(b"F1"), Ref::new(font_base))
            .pair(Name(b"F2"), Ref::new(font_base + 5));
    }
    for (i, cmds) in pages.iter().enumerate() {
        let content = render_content(cmds, fonts);
        pdf.stream(content_ids[i], &content.finish());
    }
    embed_font(
        &mut pdf,
        FontIds {
            type0: Ref::new(font_base),
            cid: Ref::new(font_base + 1),
            descriptor: Ref::new(font_base + 2),
            data: Ref::new(font_base + 3),
            to_unicode: Ref::new(font_base + 4),
        },
        fonts,
        FontRole::Regular,
    );
    embed_font(
        &mut pdf,
        FontIds {
            type0: Ref::new(font_base + 5),
            cid: Ref::new(font_base + 6),
            descriptor: Ref::new(font_base + 7),
            data: Ref::new(font_base + 8),
            to_unicode: Ref::new(font_base + 9),
        },
        fonts,
        FontRole::Bold,
    );
    pdf.finish()
}

/// Indirect references for one embedded font.
#[derive(Debug, Clone, Copy)]
struct FontIds {
    type0: Ref,
    cid: Ref,
    descriptor: Ref,
    data: Ref,
    to_unicode: Ref,
}

/// Write one embedded font: Type0 font, CID font (TrueType, Identity-H),
/// descriptor, raw TTF stream, and the ToUnicode cmap so text can be
/// copied out of the PDF.
fn embed_font(pdf: &mut Pdf, ids: FontIds, fonts: &Fonts, role: FontRole) {
    let name = fonts.pdf_name(role);
    let mut type0 = pdf.type0_font(ids.type0);
    type0.base_font(name);
    type0.encoding_predefined(Name(b"Identity-H"));
    type0.descendant_font(ids.cid);
    type0.to_unicode(ids.to_unicode);
    type0.finish();

    let face = fonts.face(role);
    let upem = fonts.upem(role);
    let mut cid = pdf.cid_font(ids.cid);
    cid.subtype(CidFontType::Type2);
    cid.base_font(name);
    cid.system_info(SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    });
    cid.font_descriptor(ids.descriptor);
    cid.default_width(0.0);
    cid.cid_to_gid_map_predefined(Name(b"Identity"));
    let glyph_count = face.number_of_glyphs();
    {
        let mut widths = cid.widths();
        let iter = (0..glyph_count).map(|gid| {
            face.glyph_hor_advance(ttf_parser::GlyphId(gid))
                .unwrap_or(500) as f32
                * 1000.0
                / upem
        });
        widths.consecutive(0, iter);
    }
    cid.finish();

    let bbox = face.global_bounding_box();
    let mut desc = pdf.font_descriptor(ids.descriptor);
    desc.name(name);
    desc.flags(FontFlags::NON_SYMBOLIC);
    desc.bbox(Rect::new(
        bbox.x_min as f32 * 1000.0 / upem,
        bbox.y_min as f32 * 1000.0 / upem,
        bbox.x_max as f32 * 1000.0 / upem,
        bbox.y_max as f32 * 1000.0 / upem,
    ));
    desc.italic_angle(0.0);
    desc.ascent(face.ascender() as f32 * 1000.0 / upem);
    desc.descent(face.descender() as f32 * 1000.0 / upem);
    desc.leading(face.line_gap() as f32 * 1000.0 / upem);
    desc.stem_v(80.0);
    desc.font_file2(ids.data);
    desc.finish();

    let raw = match role {
        FontRole::Regular => SARABUN_REGULAR,
        FontRole::Bold => SARABUN_BOLD,
    };
    pdf.stream(ids.data, raw).finish();

    // ToUnicode cmap: gid -> unicode from the font's own cmap (Identity-H
    // encoding carries glyph ids, so without this map copy/paste would
    // yield garbage).
    let mut cmap = UnicodeCmap::new(
        Name(b"MedReconToUnicode"),
        SystemInfo {
            registry: Str(b"Adobe"),
            ordering: Str(b"Identity"),
            supplement: 0,
        },
    );
    if let Some(table) = face.tables().cmap {
        for subtable in table.subtables {
            if !subtable.is_unicode() {
                continue;
            }
            let mut seen = std::collections::HashSet::new();
            subtable.codepoints(|cp| {
                if let Some(gid) = subtable.glyph_index(cp)
                    && gid.0 != 0
                    && seen.insert(gid.0)
                    && let Some(ch) = char::from_u32(cp)
                {
                    cmap.pair(gid.0, ch);
                }
            });
            break;
        }
    }
    pdf.stream(ids.to_unicode, &cmap.finish());
}

/// Turn one page's commands into a PDF content stream.
fn render_content(cmds: &[Cmd], fonts: &Fonts) -> Content {
    let mut c = Content::new();
    for cmd in cmds {
        match cmd {
            Cmd::RoundedRect {
                x,
                y,
                w,
                h,
                r,
                fill,
                stroke,
            } => {
                rounded_rect(&mut c, *x, *y, *w, *h, *r);
                c.set_fill_color(*fill);
                match stroke {
                    Some((color, width)) => {
                        c.set_stroke_color(*color);
                        c.set_line_width(*width);
                        c.fill_nonzero_and_stroke();
                    }
                    None => {
                        c.fill_nonzero();
                    }
                }
            }
            Cmd::Text {
                x,
                y,
                size,
                role,
                color,
                text,
            } => {
                c.set_fill_color(*color);
                c.begin_text();
                // `Tf` references the page-resources font name (F1/F2), not
                // the font's own BaseFont name.
                let resource = match role {
                    FontRole::Regular => Name(b"F1"),
                    FontRole::Bold => Name(b"F2"),
                };
                c.set_font(resource, *size);
                let shaped = shape(fonts, *role, *size, text);
                let mut px = *x;
                for g in &shaped.glyphs {
                    // Absolute text matrix per glyph: HarfBuzz offsets (Thai
                    // marks sit above the base) apply per glyph.
                    c.set_text_matrix([1.0, 0.0, 0.0, 1.0, px + g.x_off, y + g.y_off]);
                    c.show(Str(&g.gid.to_be_bytes()));
                    px += g.x_adv;
                }
                c.end_text();
            }
        }
    }
    c
}

/// Append a rounded-rectangle path (quarter-circle corners) to `c`.
fn rounded_rect(c: &mut Content, x: f32, y: f32, w: f32, h: f32, r: f32) {
    let r = r.min(w / 2.0).min(h / 2.0);
    c.move_to(x + r, y);
    c.line_to(x + w - r, y);
    c.cubic_to(x + w, y, x + w, y, x + w, y + r);
    c.line_to(x + w, y + h - r);
    c.cubic_to(x + w, y + h, x + w, y + h, x + w - r, y + h);
    c.line_to(x + r, y + h);
    c.cubic_to(x, y + h, x, y + h, x, y + h - r);
    c.line_to(x, y + r);
    c.cubic_to(x, y, x, y, x + r, y);
    c.close_path();
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{AllergyEntry, MedItem, MedSection, ReportModel};

    fn fonts() -> Fonts {
        Fonts::new()
    }

    fn model() -> ReportModel {
        ReportModel {
            heading: "ประวัติยาและการใช้ยา - Med Recon".into(),
            sub_line: "รพ.ทดสอบ · สร้างเมื่อ 26/08/2026".into(),
            disclaimer: "เอกสารนี้สร้างจากข้อมูลการจ่ายยา".into(),
            warnings_title: "คำเตือนความครบถ้วนของข้อมูล".into(),
            warnings: vec![],
            patient_name: "นายสมชาย ใจดี".into(),
            patient_meta: "HN 0012345 · CID 1103700123456".into(),
            allergy_title: "แพ้ยา / อาการไม่พึงประสงค์ (1)".into(),
            allergies: vec![AllergyEntry {
                agent: "Penicillin".into(),
                detail: Some("ผื่น · รายงานเมื่อ 01/07/2026 · โดย นส. nurse".into()),
            }],
            sections: vec![
                MedSection {
                    title: "ยาที่ผู้ป่วยเคยได้รับ (1)".into(),
                    items: vec![MedItem {
                        title: "Paracetamol".into(),
                        sub: Some(" · 500 mg · เม็ด".into()),
                        meta: Some("ครั้งล่าสุด 01/07/2026 · dispense 2 ครั้ง · รวม 60 · OPD".into()),
                        chip: Some("supply ≈ 30 วัน".into()),
                        sig: Some("1 × 3 /วัน".into()),
                    }],
                },
                MedSection {
                    title: "ยาที่ผู้ป่วยเคยได้รับ (ยาตามอาการ) (1)".into(),
                    items: vec![MedItem {
                        title: "Metformin".into(),
                        sub: None,
                        meta: Some(
                            "ครั้งล่าสุด 01/01/2025 · dispense 1 ครั้ง · รวม 90 · OPD / IPD".into(),
                        ),
                        chip: None,
                        sig: None,
                    }],
                },
            ],
            visits_title: "ประวัติการเข้ารับบริการ (1)".into(),
            visit_headers: vec![
                "วันที่".into(),
                "ประเภท".into(),
                "แผนก / หอผู้ป่วย".into(),
                "รหัส visit".into(),
            ],
            visits: vec![[
                "01/07/2026".into(),
                "OPD".into(),
                "OPD".into(),
                "vn1".into(),
            ]],
            footer_phi: "ข้อมูลนี้เป็นข้อมูลสุขภาพส่วนบุคคล (PHI)".into(),
            version_line: "Med Recon v0.2.0".into(),
            page_of: "หน้า {page} / {total}".into(),
        }
    }

    fn texts(pages: &[Vec<Cmd>]) -> Vec<Vec<String>> {
        pages
            .iter()
            .map(|page| {
                page.iter()
                    .filter_map(|cmd| match cmd {
                        Cmd::Text { text, .. } => Some(text.clone()),
                        Cmd::RoundedRect { .. } => None,
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn font_assets_are_valid() {
        let f = fonts();
        assert!(f.regular.number_of_glyphs() > 0);
        assert!(f.bold.number_of_glyphs() > 0);
        assert_eq!(f.regular.units_per_em(), f.bold.units_per_em());
    }

    #[test]
    fn font_covers_report_characters() {
        let f = fonts();
        for ch in "กขคง ABC 0123 ×·≈²°–—%/:().+-'\"".chars() {
            assert!(
                f.covers(FontRole::Regular, ch),
                "Sarabun-Regular must cover {ch:?} (U+{:04X})",
                ch as u32
            );
        }
        for ch in "กขคง ABC 0123 ×·≈²°–—%/:().+-'\"".chars() {
            assert!(
                f.covers(FontRole::Bold, ch),
                "Sarabun-Bold must cover {ch:?} (U+{:04X})",
                ch as u32
            );
        }
    }

    #[test]
    fn font_metrics_are_sane() {
        let f = fonts();
        let upem = f.regular.units_per_em() as f32;
        assert!((0.85..=1.1).contains(&(f.regular.ascender() as f32 / upem)));
        assert!(f.regular.descender() < 0);
        assert!(f.regular.line_gap() >= 0);
    }

    #[test]
    fn shape_places_thai_marks_with_zero_advance() {
        let f = fonts();
        // "ก่" - the tone mark must compose onto the consonant: zero pen
        // advance plus a positioning offset (GPOS), instead of consuming a
        // horizontal slot like a base character.
        let run = shape(&f, FontRole::Regular, 12.0, "ก่");
        assert!(run.glyphs.len() >= 2, "consonant + tone mark");
        let marks = &run.glyphs[1..];
        assert!(
            marks.iter().all(|g| g.x_adv == 0.0),
            "marks must not advance the pen, got {marks:?}"
        );
        assert!(
            marks.iter().any(|g| g.x_off != 0.0 || g.y_off < 0.0),
            "marks must be repositioned onto the consonant, got {marks:?}"
        );
    }

    #[test]
    fn sanitize_drops_unsupported_symbols() {
        let f = fonts();
        assert_eq!(
            sanitize(&f, FontRole::Regular, "ยาพารา ⚠️ 500 mg"),
            "ยาพารา  500 mg"
        );
        assert_eq!(sanitize(&f, FontRole::Regular, "a\tb\rc"), "a b c");
    }

    #[test]
    fn wrap_breaks_long_thai_run_without_splitting_clusters() {
        let f = fonts();
        let text = "รับประทานครั้งละหนึ่งเม็ดวันละสามครั้งหลังอาหารเช้ากลางวันเย็น";
        let lines = wrap(&f, FontRole::Regular, 9.0, text, 200.0);
        assert!(lines.len() >= 2);
        let joined: String = lines.concat();
        assert_eq!(joined, text, "wrapping must not drop or reorder text");
        for line in &lines {
            let w = measure(&f, FontRole::Regular, 9.0, line);
            assert!(w <= 200.0 + 1.0, "line {line:?} too wide: {w}");
        }
    }

    #[test]
    fn wrap_breaks_latin_at_spaces_and_keeps_short_words_whole() {
        let f = fonts();
        let lines = wrap(&f, FontRole::Regular, 9.0, "Paracetamol 500 mg", 60.0);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1], "500 mg");
    }

    #[test]
    fn break_word_prefers_breaking_after_separator() {
        let f = fonts();
        // "drugusage/sp_use" is one whitespace-free token wider than the
        // line; the break should land after "/" instead of mid-cluster.
        let chunks = break_word(&f, FontRole::Regular, 9.0, "drugusage/sp_use", 70.0);
        assert_eq!(chunks[0], "drugusage/");
        assert_eq!(chunks[1], "sp_use");
        let joined: String = chunks.concat();
        assert_eq!(joined, "drugusage/sp_use");
    }

    #[test]
    fn break_word_finds_longest_fitting_prefix() {
        let f = fonts();
        let chunks = break_word(&f, FontRole::Regular, 9.0, "aaaaaaaaaaaaaaaaaaaaaaaa", 80.0);
        assert!(chunks.len() >= 2);
        let joined: String = chunks.concat();
        assert_eq!(joined, "aaaaaaaaaaaaaaaaaaaaaaaa");
        for c in &chunks {
            assert!(measure(&f, FontRole::Regular, 9.0, c) <= 80.0 + 1.0);
        }
    }

    #[test]
    fn layout_single_page_for_small_model() {
        let pages = layout(&model(), &fonts());
        assert_eq!(pages.len(), 1);
        let texts = texts(&pages);
        assert!(texts[0].iter().any(|t| t.contains("ประวัติยาและการใช้ยา")));
        assert!(texts[0].iter().any(|t| t.contains("นายสมชาย ใจดี")));
        assert!(texts[0].iter().any(|t| t.contains("Penicillin")));
        assert!(texts[0].iter().any(|t| t.contains("Paracetamol")));
        assert!(texts[0].iter().any(|t| t.contains("supply ≈ 30 วัน")));
        assert!(texts[0].iter().any(|t| t == "01/07/2026"));
    }

    #[test]
    fn layout_multi_page_repeats_header_and_footer() {
        let mut m = model();
        let many: Vec<MedItem> = (0..60)
            .map(|i| MedItem {
                title: format!("ยาทดลองรายการที่ {i}"),
                sub: Some(" · 500 mg · เม็ด".into()),
                meta: Some("ครั้งล่าสุด 01/07/2026 · dispense 1 ครั้ง · รวม 30 · OPD".into()),
                chip: Some("supply ≈ 30 วัน".into()),
                sig: Some("1 × 3 /วัน".into()),
            })
            .collect();
        m.sections = vec![MedSection {
            title: "ยาที่ผู้ป่วยเคยได้รับ (60)".into(),
            items: many,
        }];
        let pages = layout(&m, &fonts());
        assert!(
            pages.len() >= 2,
            "60 items must span pages, got {}",
            pages.len()
        );
        let texts = texts(&pages);
        for (i, page) in texts.iter().enumerate() {
            assert!(
                page.iter().any(|t| t == "หน้า {page} / {total}"),
                "page {} must carry the footer placeholder",
                i + 1
            );
        }
        assert!(
            texts[1].iter().any(|t| t.contains("ประวัติยาและการใช้ยา")),
            "continuation pages repeat the heading"
        );
        assert!(
            texts[1].iter().any(|t| t.contains("นายสมชาย ใจดี")),
            "continuation pages carry the patient name"
        );
    }

    #[test]
    fn layout_visits_table_repeats_header_on_new_page() {
        let mut m = model();
        m.visits = (0..50)
            .map(|i| {
                [
                    "01/07/2026".to_string(),
                    "OPD".into(),
                    "แผนกอายุรกรรม".into(),
                    format!("vn{i}"),
                ]
            })
            .collect();
        m.visits_title = "ประวัติการเข้ารับบริการ (50)".into();
        let pages = layout(&m, &fonts());
        assert!(pages.len() >= 2);
        let texts = texts(&pages);
        assert!(
            texts[1].iter().any(|t| t == "วันที่"),
            "table header repeats on page 2"
        );
    }

    #[test]
    fn finalize_pages_replaces_page_numbers() {
        let mut pages = layout(&model(), &fonts());
        finalize_pages(&mut pages);
        let texts = texts(&pages);
        assert!(texts[0].iter().any(|t| t == "หน้า 1 / 1"));
        assert!(texts.iter().flatten().all(|t| !t.contains("{page}")));
    }

    #[test]
    fn write_pdf_produces_a4_document() {
        let m = model();
        let fonts = fonts();
        let mut pages = layout(&m, &fonts);
        finalize_pages(&mut pages);
        let bytes = write_pdf(&pages, &fonts);
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.ends_with(b"%%EOF"));
        let head = String::from_utf8_lossy(&bytes);
        assert!(
            head.contains("/MediaBox [0 0 595.28 841.89]"),
            "A4 media box"
        );
        assert!(head.contains("/Sarabun"), "regular font embedded");
        assert!(head.contains("/Sarabun-Bold"), "bold font embedded");
        assert!(head.contains("beginbfchar"), "ToUnicode cmap present");
    }
}
