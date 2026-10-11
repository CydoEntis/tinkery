//! The drawing kit every screen's chrome is built from: pills, chips, strips,
//! keycaps and hint lines, buttons, cards, panels, the dimmed backdrop, and
//! the header and rows of a list in a panel (tinkery-forge spec F4 to F8).
//!
//! A pill is a filled run of cells whose two ends are rounded by a Nerd Font
//! half-circle drawn in the fill colour on the ground around the pill. A font
//! without those glyphs shows boxes, so `[ui] pill_caps = false` squares the
//! ends off, and `mono`, which must work in any terminal, never rounds them:
//! the end is then a blank of the fill colour, so a pill is the same width
//! either way and nothing around it moves.
//!
//! A card is a filled box with an edge around it and its title set into the
//! top edge; a panel is a card centred over a screen `dim` has darkened.
//! `[ui] corners` picks the edge: flush, drawn with eighth blocks that meet
//! the fill exactly, or rounded box-drawing lines.
//!
//! A list in a panel is `dim`, `panel_at` (or `panel`), `header` for its
//! title or query, `sel_row` before each row's text, and `hints` on the row
//! above the bottom edge.
//!
//! Nothing here reads a global. Each function takes the `Look` and the `Theme`
//! it draws with, and the `ground` (the colour behind the pill) where an end
//! needs it, so drawing stays a pure function of `App`.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use crate::config::{Corners, UiConfig};
use crate::theme::{Theme, mix};

/// The half-circle that rounds a pill's left end (Nerd Font, U+E0B6).
pub const CAP_L: &str = "\u{e0b6}";
/// The half-circle that rounds a pill's right end (Nerd Font, U+E0B4).
pub const CAP_R: &str = "\u{e0b4}";

/// How the chrome is drawn, from `[ui]` in `config.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Look {
    /// Round pill ends with `CAP_L` and `CAP_R`; off squares them.
    pub pill_caps: bool,
    /// The edge of a card: flush eighth blocks or rounded lines.
    pub corners: Corners,
}

impl Default for Look {
    fn default() -> Self {
        Look::of(&UiConfig::default())
    }
}

impl Look {
    /// The look `[ui]` asks for.
    pub fn of(ui: &UiConfig) -> Look {
        Look {
            pill_caps: ui.pill_caps,
            corners: ui.corners,
        }
    }

    /// Whether pill ends are the cap glyphs in `theme`: the setting is on and
    /// the theme isn't `mono`, which never draws a Nerd Font glyph.
    pub fn round(self, theme: &Theme) -> bool {
        self.pill_caps && !theme.flat()
    }
}

/// `color` for text on `fill`, or `text` when the two are the same colour and
/// the words would vanish. Only terminal colours collide: `mono`'s `muted`,
/// `guide`, `btn` and `raised2` are all dark grey.
pub fn ink(theme: &Theme, color: Color, fill: Color) -> Color {
    if color == fill { theme.text } else { color }
}

/// The display width of `spans` in cells.
pub fn width(spans: &[Span]) -> u16 {
    let cells: usize = spans.iter().map(Span::width).sum();
    u16::try_from(cells).unwrap_or(u16::MAX)
}

/// Draws `spans` on row `y` from `x`, stopping before `max_x`, and returns the
/// column after the last cell drawn.
pub fn put(buf: &mut Buffer, x: u16, y: u16, spans: &[Span], max_x: u16) -> u16 {
    let mut x = x;
    for span in spans {
        if x >= max_x {
            break;
        }
        x = buf.set_span(x, y, span, max_x - x).0;
    }
    x
}

/// The two ends of a pill of `fill` sitting on `ground`: `CAP_L` and `CAP_R`
/// in `fill` on `ground`, or, squared off, a blank of `fill` each. One cell
/// wide either way.
pub fn ends(
    look: Look,
    theme: &Theme,
    fill: Color,
    ground: Color,
) -> (Span<'static>, Span<'static>) {
    if look.round(theme) {
        let style = Style::new().fg(fill).bg(ground);
        (Span::styled(CAP_L, style), Span::styled(CAP_R, style))
    } else {
        let style = Style::new().bg(fill);
        (Span::styled(" ", style), Span::styled(" ", style))
    }
}

/// `spans` on a pill of `fill` on `ground`: an end, the spans with their
/// background set to `fill`, an end. Two cells wider than the spans.
pub fn pill(
    look: Look,
    theme: &Theme,
    spans: Vec<Span<'static>>,
    fill: Color,
    ground: Color,
) -> Vec<Span<'static>> {
    let (left, right) = ends(look, theme, fill, ground);
    let mut out = Vec::with_capacity(spans.len() + 2);
    out.push(left);
    out.extend(
        spans
            .into_iter()
            .map(|span| span.patch_style(Style::new().bg(fill))),
    );
    out.push(right);
    out
}

/// `text` on a pill filled with `style`'s background, on `ground`: as wide as
/// `" text "`. A toggle, a tag, a key.
pub fn chip(
    look: Look,
    theme: &Theme,
    text: &str,
    style: Style,
    ground: Color,
) -> Vec<Span<'static>> {
    let fill = style.bg.unwrap_or(Color::Reset);
    let label = Span::styled(text.to_string(), style);
    pill(look, theme, vec![label], fill, ground)
}

/// A toggle on `ground`, a chip as wide as `" text "`: `acc_ink` on `accent`,
/// bold, while `on`; `muted` on `btn` while off. `hovered` turns an off
/// toggle's fill to `hov`, with its text in `text`, as `muted` is too close to
/// that fill to read; one that is on doesn't change, and nor does any in
/// `mono`, which has no fills.
pub fn toggle(
    look: Look,
    theme: &Theme,
    text: &str,
    on: bool,
    hovered: bool,
    ground: Color,
) -> Vec<Span<'static>> {
    let style = if on {
        Style::new()
            .bg(theme.accent)
            .fg(theme.acc_ink)
            .add_modifier(Modifier::BOLD)
    } else if hovered && !theme.flat() {
        Style::new().bg(theme.hov).fg(theme.text)
    } else {
        Style::new()
            .bg(theme.btn)
            .fg(ink(theme, theme.muted, theme.btn))
    };
    chip(look, theme, text, style, ground)
}

/// An empty pill of `fill` on `ground`, `width` cells from `(x, y)`, over
/// whatever was drawn there: the ground of a selected row or an input. Draw
/// its text afterwards, from `x + 1`. Narrower than its two ends, it draws
/// nothing.
pub fn row_pill(
    look: Look,
    theme: &Theme,
    buf: &mut Buffer,
    (x, y): (u16, u16),
    width: u16,
    fill: Color,
    ground: Color,
) {
    if width < 2 {
        return;
    }
    let row = Rect::new(x, y, width, 1).intersection(buf.area);
    // What was there may be bold or underlined, which a blank would keep.
    for cx in row.left()..row.right() {
        buf[(cx, y)].reset();
    }
    if row.is_empty() {
        return;
    }
    let blank = Span::raw(" ".repeat(usize::from(width - 2)));
    put(
        buf,
        x,
        y,
        &pill(look, theme, vec![blank], fill, ground),
        row.right(),
    );
}

/// The cells `strip` returns for a bar across the top row of `area`, without
/// drawing it: where the pointer is looked for is where the bar's content was
/// drawn.
pub fn strip_inside(area: Rect) -> Rect {
    let row = Rect {
        height: area.height.min(1),
        ..area
    };
    if row.width < 3 {
        return row;
    }
    Rect {
        x: row.x + 1,
        width: row.width - 2,
        ..row
    }
}

/// A one-row bar of `fill` across the top row of `area`, on `ground`: a row
/// pill, or just the fill where there's no room for two ends and something
/// between them. Returns the cells between the ends, where the bar's content
/// goes; they are the same cells whether the ends are round or square.
pub fn strip(
    look: Look,
    theme: &Theme,
    buf: &mut Buffer,
    area: Rect,
    fill: Color,
    ground: Color,
) -> Rect {
    let row = Rect {
        height: area.height.min(1),
        ..area
    };
    if row.width < 3 {
        let shown = row.intersection(buf.area);
        for y in shown.top()..shown.bottom() {
            for x in shown.left()..shown.right() {
                buf[(x, y)].reset();
            }
        }
        buf.set_style(shown, Style::new().bg(fill));
        return row;
    }
    row_pill(look, theme, buf, (row.x, row.y), row.width, fill, ground);
    strip_inside(area)
}

/// A key as a keycap: a chip on `btn` with the key in `strong`, bold.
pub fn keycap(look: Look, theme: &Theme, key: &str, ground: Color) -> Vec<Span<'static>> {
    let style = Style::new()
        .bg(theme.btn)
        .fg(theme.strong)
        .add_modifier(Modifier::BOLD);
    chip(look, theme, key, style, ground)
}

/// `keys` as keycaps, one blank of `ground` apart. Keys are separated by a
/// space and sets of keys by two; a set of four or more one-letter keys shares
/// one cap (`h j k l`), every other key gets its own.
pub fn keycaps(look: Look, theme: &Theme, keys: &str, ground: Color) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    for set in keys.split("  ").filter(|set| !set.is_empty()) {
        let each: Vec<&str> = set.split(' ').filter(|key| !key.is_empty()).collect();
        let caps = if each.len() >= 4 && each.iter().all(|key| key.chars().count() == 1) {
            vec![each.join(" ")]
        } else {
            each.iter().map(|key| (*key).to_string()).collect()
        };
        for cap in caps {
            if !out.is_empty() {
                out.push(Span::styled(" ", Style::new().bg(ground)));
            }
            out.extend(keycap(look, theme, &cap, ground));
        }
    }
    out
}

/// A hint line on `ground`: for each `(keys, word)`, the keys as keycaps, a
/// blank and the word in `muted`; three blanks between one hint and the next.
pub fn hints(
    look: Look,
    theme: &Theme,
    pairs: &[(&str, &str)],
    ground: Color,
) -> Vec<Span<'static>> {
    let plain = Style::new().bg(ground);
    let word = plain.fg(ink(theme, theme.muted, ground));
    let mut out = Vec::new();
    for (index, (keys, what)) in pairs.iter().enumerate() {
        if index > 0 {
            out.push(Span::styled("   ", plain));
        }
        out.extend(keycaps(look, theme, keys, ground));
        out.push(Span::styled(format!(" {what}"), word));
    }
    out
}

/// What a button is for, which decides how loud it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// The one Enter presses: `acc_ink` on `accent`.
    Primary,
    /// The one that loses or removes something: on `err`, deepened until its
    /// text reads.
    Danger,
    /// Any other: `strong` on `btn`, its key in `accent`.
    Normal,
    /// One that should recede: `text` on `surface`, its key in `accent`.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "no confirm card has one; the splash's buttons (F20) are its first use"
        )
    )]
    Ghost,
}

/// A button on `ground`: a pill of `label`, a blank and `key`, both bold, as
/// wide as `" label key "`; without a `key`, as wide as `" label "`. `hovered`
/// turns a normal or ghost button's fill to `hov`. `mono` has no fills to tell
/// the kinds apart, so there primary and danger are reverse video and the
/// others only bold.
pub fn button(
    look: Look,
    theme: &Theme,
    label: &str,
    key: &str,
    kind: ButtonKind,
    hovered: bool,
    ground: Color,
) -> Vec<Span<'static>> {
    let loud = matches!(kind, ButtonKind::Primary | ButtonKind::Danger);
    let bold = Style::new().add_modifier(Modifier::BOLD);
    if theme.flat() {
        let style = bold.fg(theme.strong).bg(ground);
        let style = if loud {
            style.add_modifier(Modifier::REVERSED)
        } else {
            style
        };
        let text = if key.is_empty() {
            format!(" {label} ")
        } else {
            format!(" {label} {key} ")
        };
        return vec![Span::styled(text, style)];
    }
    let (fill, fg, key_fg) = match kind {
        ButtonKind::Primary => (theme.accent, theme.acc_ink, theme.acc_ink),
        ButtonKind::Danger => {
            let (fill, ink) = danger(theme);
            (fill, ink, ink)
        }
        ButtonKind::Normal => (theme.btn, theme.strong, theme.accent),
        ButtonKind::Ghost => (theme.surface, theme.text, theme.accent),
    };
    let fill = if hovered && !loud { theme.hov } else { fill };
    let spans = if key.is_empty() {
        vec![Span::styled(label.to_string(), bold.fg(fg))]
    } else {
        vec![
            Span::styled(format!("{label} "), bold.fg(fg)),
            Span::styled(key.to_string(), bold.fg(key_fg)),
        ]
    };
    pill(look, theme, spans, fill, ground)
}

/// The least contrast a danger button's text may have on its fill (WCAG AA).
const DANGER_CONTRAST: f64 = 4.5;
/// How far one step deepens `err` towards black, and how many steps at most.
const DANGER_STEP: f64 = 0.08;
const DANGER_STEPS: usize = 12;

/// A danger button's fill and the text colour on it: `err`, deepened a step at
/// a time until `bg` or `strong`, whichever reads better on it, reaches
/// `DANGER_CONTRAST`.
fn danger(theme: &Theme) -> (Color, Color) {
    let ink = |fill: Color| {
        if contrast(theme.bg, fill) >= contrast(theme.strong, fill) {
            theme.bg
        } else {
            theme.strong
        }
    };
    let mut fill = theme.err;
    for _ in 0..DANGER_STEPS {
        if contrast(ink(fill), fill) >= DANGER_CONTRAST {
            break;
        }
        fill = mix(fill, Color::Rgb(0, 0, 0), DANGER_STEP);
    }
    (fill, ink(fill))
}

/// The WCAG 2 contrast ratio of two colours, 1..=21. A terminal colour has no
/// known brightness and counts as mid grey.
fn contrast(a: Color, b: Color) -> f64 {
    let luminance = |c: Color| match c {
        Color::Rgb(r, g, b) => {
            let linear = |v: u8| {
                let v = f64::from(v) / 255.0;
                if v <= 0.03928 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
        }
        _ => 0.2,
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The glyphs a card's edge is drawn with.
struct Edges {
    top: &'static str,
    bottom: &'static str,
    left: &'static str,
    right: &'static str,
    /// Top left, top right, bottom left, bottom right.
    corners: [&'static str; 4],
}

impl Edges {
    /// A cell has one background, and a box-drawing line runs through its
    /// middle, so a filled card inside such a line shows a strip of the ground
    /// between the line and the fill. The flush glyphs are eighth blocks on
    /// the side of the cell that faces the card, so the edge meets the fill;
    /// its corners are blank, where the two lines would meet at a point.
    fn of(look: Look) -> Edges {
        match look.corners {
            Corners::Flush => Edges {
                top: "▁",
                bottom: "▔",
                left: "▕",
                right: "▏",
                corners: [" "; 4],
            },
            Corners::Rounded => Edges {
                top: "─",
                bottom: "─",
                left: "│",
                right: "│",
                corners: ["╭", "╮", "╰", "╯"],
            },
        }
    }
}

/// What a card looks like: its title, edge and fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Card<'a> {
    /// Set into the top edge, bold; empty leaves the edge whole.
    pub title: &'a str,
    /// The edge's colour.
    pub edge: Color,
    /// The inside's colour.
    pub fill: Color,
    /// The card has the focus: the edge is bold and the title takes its colour.
    pub lit: bool,
}

impl<'a> Card<'a> {
    /// A resting card: a `line2` edge around `raised`, its title in `text`.
    pub fn new(theme: &Theme, title: &'a str) -> Card<'a> {
        Card {
            title,
            edge: theme.line2,
            fill: theme.raised,
            lit: false,
        }
    }

    /// The same card with the focus: edge and title in `color`, bold.
    pub fn lit(self, color: Color) -> Card<'a> {
        Card {
            edge: color,
            lit: true,
            ..self
        }
    }
}

/// Empties the cell at `(x, y)` of its glyph, colour and weight but keeps its
/// background: what a card's edge is drawn on.
fn bare(buf: &mut Buffer, x: u16, y: u16) -> &mut ratatui::buffer::Cell {
    let cell = &mut buf[(x, y)];
    let ground = cell.bg;
    cell.reset();
    cell.set_bg(ground)
}

/// Draws `card` over `area` and returns the inside, a cell in from each side,
/// filled and emptied of what was there. The edge cells keep the background
/// already drawn in them, so the card sits on whatever is behind it; draw
/// that first. The title starts three cells in and is cut four short of the
/// right edge. Smaller than 4 by 2, a card draws nothing and has no inside.
pub fn card(look: Look, theme: &Theme, buf: &mut Buffer, area: Rect, card: &Card) -> Rect {
    let area = area.intersection(buf.area);
    if area.width < 4 || area.height < 2 {
        return Rect {
            width: 0,
            height: 0,
            ..area
        };
    }
    let inside = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width - 2,
        height: area.height - 2,
    };
    for y in inside.top()..inside.bottom() {
        for x in inside.left()..inside.right() {
            buf[(x, y)].reset();
        }
    }
    buf.set_style(inside, Style::new().bg(card.fill).fg(theme.text));

    let line = Style::new().fg(card.edge);
    let line = if card.lit {
        line.add_modifier(Modifier::BOLD)
    } else {
        line
    };
    let edges = Edges::of(look);
    let (left, top, right, bottom) = (area.x, area.y, area.right() - 1, area.bottom() - 1);
    let [top_left, top_right, bottom_left, bottom_right] = edges.corners;
    for x in left..=right {
        let (above, below) = match x {
            x if x == left => (top_left, bottom_left),
            x if x == right => (top_right, bottom_right),
            _ => (edges.top, edges.bottom),
        };
        bare(buf, x, top).set_symbol(above).set_style(line);
        bare(buf, x, bottom).set_symbol(below).set_style(line);
    }
    for y in top + 1..bottom {
        bare(buf, left, y).set_symbol(edges.left).set_style(line);
        bare(buf, right, y).set_symbol(edges.right).set_style(line);
    }

    let room = area.width.saturating_sub(6);
    if !card.title.is_empty() && room > 2 {
        let color = if card.lit { card.edge } else { theme.text };
        let title = Style::new().fg(color).add_modifier(Modifier::BOLD);
        buf.set_stringn(
            left + 2,
            top,
            format!(" {} ", card.title),
            usize::from(room),
            title,
        );
    }
    inside
}

/// Where a panel `width` by `height` goes in `area`: centred, and kept a cell
/// inside it on every side.
pub fn centred(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

/// A panel, the box a dialog is drawn in: a card `width` by `height` at
/// `centred` in `area`, with an `accent` edge and title around `raised`.
/// Returns the inside. `dim` the screen first.
pub fn panel(
    look: Look,
    theme: &Theme,
    buf: &mut Buffer,
    area: Rect,
    (width, height): (u16, u16),
    title: &str,
) -> Rect {
    panel_at(look, theme, buf, centred(area, width, height), title)
}

/// A panel over `rect` exactly, for a dialog that says where it goes itself
/// (a list that drops from the top rather than sitting in the middle): a card
/// with an `accent` edge and title around `raised`. Returns the inside. `dim`
/// the screen first.
pub fn panel_at(look: Look, theme: &Theme, buf: &mut Buffer, rect: Rect, title: &str) -> Rect {
    let lit = Card::new(theme, title).lit(theme.accent);
    card(look, theme, buf, rect, &lit)
}

/// The mark a list panel's header starts with.
pub const HEADER_MARK: &str = "✦";
/// The mark a selected row starts with.
pub const SEL_MARK: &str = "›";

/// The header of a list in a panel drawn over `card`: a strip of `raised2`
/// across the card's inside on its third row, with `HEADER_MARK` in `accent2`
/// bold four cells in from the card's edge and `right` (what the list holds,
/// a count) ending four cells short of the other, on the strip's fill. Returns
/// the cells between them, where the caller draws the title or the query on
/// `raised2`: from two cells after the mark to two short of `right`. The row
/// under the strip stays blank and the list starts on the card's fifth row.
/// A card with no room for the strip gets none and an empty field.
pub fn header(
    look: Look,
    theme: &Theme,
    buf: &mut Buffer,
    card: Rect,
    right: Vec<Span<'static>>,
) -> Rect {
    let fill = theme.raised2;
    let y = card.y + 2;
    if card.width < 12 || card.height < 4 || y >= buf.area.bottom() {
        return Rect::new(card.x, y, 0, 0);
    }
    let row = Rect::new(card.x + 1, y, card.width - 2, 1);
    strip(look, theme, buf, row, fill, theme.raised);
    let mark = Style::new()
        .fg(theme.accent2)
        .bg(fill)
        .add_modifier(Modifier::BOLD);
    let (left, end) = (card.x + 4, card.right() - 4);
    put(buf, left, y, &[Span::styled(HEADER_MARK, mark)], end);
    let right: Vec<Span> = right
        .into_iter()
        .map(|span| span.patch_style(Style::new().bg(fill)))
        .collect();
    let field_x = left + 2;
    let right_x = end.saturating_sub(width(&right)).max(field_x);
    put(buf, right_x, y, &right, end);
    let field_end = if right.is_empty() {
        end
    } else {
        right_x.saturating_sub(2)
    };
    Rect::new(field_x, y, field_end.saturating_sub(field_x), 1)
}

/// The styles one row of a list draws its text in, from `sel_row`. They carry
/// the row's fill, so text drawn in them sits on the row whatever state it is
/// in; on the selected row in `mono`, which is reverse video, they carry no
/// colours at all, as a colour would break the reverse up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowInk {
    /// The row's own words: `strong` bold when selected, else `fg`.
    pub label: Style,
    /// The letters a query matched: `accent` bold.
    pub hit: Style,
    /// What says less than the label (a folder, a key, a source): `muted`, or
    /// `text` on a lit row, where `muted` is too close to the fill to read.
    pub faint: Style,
    fill: Color,
    plain: bool,
}

impl RowInk {
    /// `style` as this row draws it: on the row's fill, or with its weight
    /// alone where the row carries no colours.
    pub fn of(self, style: Style) -> Style {
        if self.plain {
            Style::new()
                .add_modifier(style.add_modifier)
                .remove_modifier(style.sub_modifier)
        } else {
            style.bg(self.fill)
        }
    }
}

/// Draws the state of one row of a list on `ground` and returns the styles
/// its text goes in; call it for every row, before drawing the row's text.
/// `row` is the whole row, edge to edge of the card's inside; the text starts
/// three cells into it, after the mark.
///
/// A `selected` row is a row pill of `hov` with `SEL_MARK` in `accent` bold a
/// cell in; a `lit` one gets the pill without the mark; any other row is left
/// as it is. A row is lit while the pointer is on it (see `row_under`), and
/// for whatever else a list marks without selecting, as the tree does its
/// active file. Accent is never a row's fill. `mono` has no fills: there the
/// selected row is reverse video from edge to edge, with the mark and nothing
/// coloured on it, and a lit row shows nothing.
pub fn sel_row(
    look: Look,
    theme: &Theme,
    buf: &mut Buffer,
    row: Rect,
    selected: bool,
    lit: bool,
    ground: Color,
) -> RowInk {
    let bold = Modifier::BOLD;
    let row = Rect { height: 1, ..row }.intersection(buf.area);
    let resting = RowInk {
        label: Style::new().fg(theme.fg).bg(ground),
        hit: Style::new().fg(theme.accent).bg(ground).add_modifier(bold),
        faint: Style::new().fg(ink(theme, theme.muted, ground)).bg(ground),
        fill: ground,
        plain: false,
    };
    if row.is_empty() {
        return resting;
    }
    let mark_x = row.x.saturating_add(1);
    if theme.flat() {
        if !selected {
            return resting;
        }
        for x in row.left()..row.right() {
            buf[(x, row.y)].reset();
        }
        buf.set_style(row, Theme::highlight(theme.hov, theme.strong));
        let weight = Style::new().add_modifier(bold);
        let mark = [Span::styled(SEL_MARK, weight)];
        put(buf, mark_x, row.y, &mark, row.right());
        return RowInk {
            label: weight,
            hit: weight,
            faint: Style::new(),
            fill: ground,
            plain: true,
        };
    }
    if !selected && !lit {
        return resting;
    }
    let fill = theme.hov;
    row_pill(look, theme, buf, (row.x, row.y), row.width, fill, ground);
    let on = Style::new().bg(fill);
    if selected {
        let mark = [Span::styled(
            SEL_MARK,
            on.fg(theme.accent).add_modifier(bold),
        )];
        // Never over the pill's right end, on a row too narrow to hold it.
        put(buf, mark_x, row.y, &mark, row.right().saturating_sub(1));
    }
    RowInk {
        label: if selected {
            on.fg(theme.strong).add_modifier(bold)
        } else {
            on.fg(theme.fg)
        },
        hit: on.fg(theme.accent).add_modifier(bold),
        faint: on.fg(theme.text),
        fill,
        plain: false,
    }
}

/// The screen row of a list in `card` that the pointer at `at` is on: any row
/// inside the card's edges, as a list's rows run from one edge to the other.
/// Pass it to the list's drawing, which lights the row it draws there; a
/// heading or a blank on that row isn't a `sel_row` and stays as it is.
pub fn row_under(card: Rect, at: Position) -> Option<u16> {
    let inside = Rect {
        x: card.x.saturating_add(1),
        y: card.y.saturating_add(1),
        width: card.width.saturating_sub(2),
        height: card.height.saturating_sub(2),
    };
    inside.contains(at).then_some(at.y)
}

/// How far the text behind a panel moves towards its own ground, and then how
/// far everything moves towards black.
const BEHIND_TEXT: f64 = 0.6;
const BEHIND_DARK: f64 = 0.45;

/// Darkens everything already drawn in `area`, so a panel drawn after it
/// stands out: text fades into its ground and then all of it goes towards
/// black. A pill's cap is its fill, not text, so it only darkens and the pill
/// keeps its shape. Where the theme's colours can't be blended (`mono`) the
/// cells get the DIM modifier instead.
pub fn dim(theme: &Theme, buf: &mut Buffer, area: Rect) {
    const BLACK: Color = Color::Rgb(0, 0, 0);
    let area = area.intersection(buf.area);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            if !theme.ramps() {
                cell.modifier.insert(Modifier::DIM);
                continue;
            }
            let bg = if cell.bg == Color::Reset {
                theme.bg
            } else {
                cell.bg
            };
            let fg = if cell.fg == Color::Reset {
                theme.fg
            } else {
                cell.fg
            };
            // Reverse video draws the text in `bg` on a ground of `fg`.
            let reversed = cell.modifier.contains(Modifier::REVERSED);
            let (ink, ground) = if reversed { (bg, fg) } else { (fg, bg) };
            let cap = matches!(cell.symbol(), CAP_L | CAP_R);
            let faded = if cap {
                ink
            } else {
                mix(ink, ground, BEHIND_TEXT)
            };
            // A diagnostic's colour is its curly underline; left alone it
            // would draw at full strength through the dim. `Reset` means no
            // underline colour, not a colour.
            if cell.underline_color != Color::Reset {
                let faded = mix(cell.underline_color, ground, BEHIND_TEXT);
                cell.underline_color = mix(faded, BLACK, BEHIND_DARK);
            }
            let ink = mix(faded, BLACK, BEHIND_DARK);
            let ground = mix(ground, BLACK, BEHIND_DARK);
            (cell.fg, cell.bg) = if reversed {
                (ground, ink)
            } else {
                (ink, ground)
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUND: Look = Look {
        pill_caps: true,
        corners: Corners::Flush,
    };
    const SQUARE: Look = Look {
        pill_caps: false,
        corners: Corners::Flush,
    };
    const FILL: Color = Color::Rgb(1, 2, 3);
    const GROUND: Color = Color::Rgb(9, 9, 9);

    fn mono() -> Theme {
        Theme::named("mono").expect("mono exists")
    }

    /// Every way an end is drawn: round, squared by the setting, squared by
    /// `mono` whatever the setting says.
    fn looks() -> [(Look, Theme, bool); 3] {
        [
            (ROUND, Theme::default(), true),
            (SQUARE, Theme::default(), false),
            (ROUND, mono(), false),
        ]
    }

    fn text(spans: &[Span]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    fn drawn(spans: &[Span], cells: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, cells, 1));
        put(&mut buf, 0, 0, spans, cells);
        buf
    }

    fn row(buf: &Buffer) -> String {
        (0..buf.area.width).map(|x| buf[(x, 0)].symbol()).collect()
    }

    /// Asserts the cell at `x` is a pill end of `fill` on `ground`: the cap
    /// glyph when `round`, else a blank of `fill`.
    fn assert_end(buf: &Buffer, x: u16, cap: &str, round: bool, fill: Color, ground: Color) {
        let cell = &buf[(x, 0)];
        if round {
            assert_eq!(cell.symbol(), cap, "{x}");
            assert_eq!((cell.fg, cell.bg), (fill, ground), "{x}");
        } else {
            assert_eq!(cell.symbol(), " ", "{x}");
            assert_eq!(cell.bg, fill, "{x}");
        }
    }

    fn assert_no_caps(buf: &Buffer) {
        let row = row(buf);
        assert!(!row.contains(CAP_L) && !row.contains(CAP_R), "{row:?}");
    }

    #[test]
    fn the_look_follows_the_setting_and_mono_never_rounds() {
        assert!(Look::default().pill_caps);
        assert_eq!(Look::default().corners, Corners::Flush);
        let asked = Look::of(&UiConfig {
            pill_caps: false,
            corners: Corners::Rounded,
        });
        assert!(!asked.pill_caps);
        assert_eq!(asked.corners, Corners::Rounded);
        assert!(ROUND.round(&Theme::default()));
        assert!(!SQUARE.round(&Theme::default()));
        assert!(!ROUND.round(&mono()));
        for name in crate::theme::NAMES.iter().filter(|name| **name != "mono") {
            let theme = Theme::named(name).expect("a listed theme exists");
            assert!(ROUND.round(&theme), "{name}");
        }
    }

    #[test]
    fn ends_are_caps_in_the_fill_on_the_ground_or_blanks_of_the_fill() {
        for (look, theme, round) in looks() {
            let (left, right) = ends(look, &theme, FILL, GROUND);
            assert_eq!((left.width(), right.width()), (1, 1));
            let buf = drawn(&[left, right], 2);
            assert_end(&buf, 0, CAP_L, round, FILL, GROUND);
            assert_end(&buf, 1, CAP_R, round, FILL, GROUND);
            if !round {
                assert_no_caps(&buf);
            }
        }
    }

    #[test]
    fn a_pill_fills_its_spans_and_is_two_cells_wider() {
        for (look, theme, round) in looks() {
            let spans = vec![
                Span::styled("ab", Style::new().fg(Color::Red)),
                Span::styled("c", Style::new().bg(Color::Blue)),
            ];
            let pill = pill(look, &theme, spans, FILL, GROUND);
            assert_eq!(width(&pill), 5);
            let buf = drawn(&pill, 5);
            assert_end(&buf, 0, CAP_L, round, FILL, GROUND);
            assert_end(&buf, 4, CAP_R, round, FILL, GROUND);
            assert_eq!(&row(&buf)[if round { 3 } else { 1 }..][..3], "abc");
            for x in 1..4 {
                assert_eq!(buf[(x, 0)].bg, FILL, "{x}");
            }
            // A span keeps its own colour for the text.
            assert_eq!(buf[(1, 0)].fg, Color::Red);
        }
    }

    #[test]
    fn a_chip_is_its_text_between_ends_of_its_own_background() {
        for (look, theme, round) in looks() {
            let style = Style::new()
                .fg(Color::Red)
                .bg(FILL)
                .add_modifier(Modifier::BOLD);
            let chip = chip(look, &theme, "Aa", style, GROUND);
            // As wide as ` Aa `.
            assert_eq!(width(&chip), 4);
            let buf = drawn(&chip, 4);
            assert_end(&buf, 0, CAP_L, round, FILL, GROUND);
            assert_end(&buf, 3, CAP_R, round, FILL, GROUND);
            assert_eq!(text(&chip[1..2]), "Aa");
            for x in 1..3 {
                let cell = &buf[(x, 0)];
                assert_eq!((cell.fg, cell.bg), (Color::Red, FILL), "{x}");
                assert!(cell.modifier.contains(Modifier::BOLD), "{x}");
            }
        }
    }

    #[test]
    fn a_row_pill_is_as_wide_as_asked_and_clears_what_was_there() {
        for (look, theme, round) in looks() {
            let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
            let underlined = Style::new().add_modifier(Modifier::UNDERLINED);
            buf.set_string(0, 0, "xxxxxxxxxx", underlined);
            row_pill(look, &theme, &mut buf, (2, 0), 6, FILL, GROUND);
            assert_end(&buf, 2, CAP_L, round, FILL, GROUND);
            assert_end(&buf, 7, CAP_R, round, FILL, GROUND);
            for x in 3..7 {
                let cell = &buf[(x, 0)];
                assert_eq!((cell.symbol(), cell.bg), (" ", FILL), "{x}");
            }
            for x in 2..8 {
                assert!(buf[(x, 0)].modifier.is_empty(), "{x}");
            }
            // Outside the pill nothing changed.
            assert_eq!(buf[(1, 0)].symbol(), "x");
            assert_eq!(buf[(8, 0)].symbol(), "x");
        }
        // No room for two ends: nothing is drawn. Past the buffer: clipped.
        let theme = Theme::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 1));
        row_pill(ROUND, &theme, &mut buf, (0, 0), 1, FILL, GROUND);
        assert_eq!(buf, Buffer::empty(Rect::new(0, 0, 4, 1)));
        row_pill(ROUND, &theme, &mut buf, (2, 0), 9, FILL, GROUND);
        assert_eq!(buf[(2, 0)].symbol(), CAP_L);
        assert_eq!(buf[(3, 0)].bg, FILL);
    }

    #[test]
    fn a_strip_fills_its_row_between_two_ends_and_returns_the_inside() {
        for (look, theme, round) in looks() {
            let mut buf = Buffer::empty(Rect::new(0, 0, 12, 2));
            let inner = strip(look, &theme, &mut buf, Rect::new(1, 0, 10, 2), FILL, GROUND);
            // The inside is the same cells whatever the ends are.
            assert_eq!(inner, Rect::new(2, 0, 8, 1));
            assert_end(&buf, 1, CAP_L, round, FILL, GROUND);
            assert_end(&buf, 10, CAP_R, round, FILL, GROUND);
            for x in inner.left()..inner.right() {
                let cell = &buf[(x, 0)];
                assert_eq!((cell.symbol(), cell.bg), (" ", FILL), "{x}");
            }
            // One row only.
            assert_eq!(buf[(5, 1)].bg, Color::Reset);
            if !round {
                assert_no_caps(&buf);
            }
        }
        // Too narrow for ends and a cell between them: just the fill.
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 1));
        let area = Rect::new(0, 0, 2, 1);
        let inner = strip(ROUND, &Theme::default(), &mut buf, area, FILL, GROUND);
        assert_eq!(inner, area);
        assert_eq!(row(&buf), "    ");
        assert_eq!((buf[(0, 0)].bg, buf[(1, 0)].bg), (FILL, FILL));
        assert_eq!(buf[(2, 0)].bg, Color::Reset);
    }

    #[test]
    fn a_keycap_is_the_key_in_strong_bold_on_btn() {
        for (look, theme, round) in looks() {
            let cap = keycap(look, &theme, "esc", GROUND);
            assert_eq!(width(&cap), 5);
            let buf = drawn(&cap, 5);
            assert_end(&buf, 0, CAP_L, round, theme.btn, GROUND);
            assert_end(&buf, 4, CAP_R, round, theme.btn, GROUND);
            assert_eq!(text(&cap[1..2]), "esc");
            for x in 1..4 {
                let cell = &buf[(x, 0)];
                assert_eq!((cell.fg, cell.bg), (theme.strong, theme.btn), "{x}");
                assert!(cell.modifier.contains(Modifier::BOLD), "{x}");
            }
        }
    }

    #[test]
    fn keycaps_give_each_key_a_cap_and_four_letters_one() {
        let theme = Theme::default();
        let caps = |keys: &str| text(&keycaps(SQUARE, &theme, keys, GROUND));
        assert_eq!(caps("esc"), " esc ");
        assert_eq!(caps("↑ ↓"), " ↑   ↓ ");
        assert_eq!(caps("h j k l"), " h j k l ");
        // Three letters, or four keys with a longer one among them, stay apart.
        assert_eq!(caps("a b c"), " a   b   c ");
        assert_eq!(caps("h j k tab"), " h   j   k   tab ");
        // Two blanks part sets of keys.
        assert_eq!(caps("h j k l  ⏎"), " h j k l   ⏎ ");
        assert_eq!(caps(""), "");

        for (look, theme, round) in looks() {
            let caps = keycaps(look, &theme, "↑ ↓", GROUND);
            assert_eq!(width(&caps), 7);
            let buf = drawn(&caps, 7);
            assert_end(&buf, 0, CAP_L, round, theme.btn, GROUND);
            assert_end(&buf, 2, CAP_R, round, theme.btn, GROUND);
            // The gap between two caps is the ground, not the caps' fill.
            assert_eq!((buf[(3, 0)].symbol(), buf[(3, 0)].bg), (" ", GROUND));
            assert_end(&buf, 4, CAP_L, round, theme.btn, GROUND);
            assert_end(&buf, 6, CAP_R, round, theme.btn, GROUND);
            assert_eq!(buf[(5, 0)].fg, theme.strong);
            if !round {
                assert_no_caps(&buf);
            }
        }
    }

    #[test]
    fn hints_are_keycaps_and_muted_words_three_blanks_apart() {
        for (look, theme, round) in looks() {
            let ground = theme.raised2;
            let line = hints(look, &theme, &[("⏎", "create"), ("esc", "cancel")], ground);
            // ` ⏎  create    esc  cancel`, the blanks round a key being its ends.
            assert_eq!(width(&line), 25);
            let buf = drawn(&line, 25);
            let ends = if round { (CAP_L, CAP_R) } else { (" ", " ") };
            let (l, r) = ends;
            assert_eq!(row(&buf), format!("{l}⏎{r} create   {l}esc{r} cancel"));
            assert_end(&buf, 0, CAP_L, round, theme.btn, ground);
            assert_end(&buf, 2, CAP_R, round, theme.btn, ground);
            assert_eq!((buf[(1, 0)].fg, buf[(1, 0)].bg), (theme.strong, theme.btn));
            // The word, and the gap before the next key, sit on the ground.
            let word = &buf[(4, 0)];
            assert_eq!(word.bg, ground);
            assert_eq!(word.fg, ink(&theme, theme.muted, ground));
            assert!(!word.modifier.contains(Modifier::BOLD));
            for x in 10..13 {
                assert_eq!((buf[(x, 0)].symbol(), buf[(x, 0)].bg), (" ", ground));
            }
            assert_end(&buf, 13, CAP_L, round, theme.btn, ground);
            assert_end(&buf, 17, CAP_R, round, theme.btn, ground);
        }
    }

    #[test]
    fn ink_stays_readable_where_terminal_colours_collide() {
        let theme = Theme::default();
        assert_eq!(ink(&theme, theme.muted, theme.raised2), theme.muted);
        // mono's `muted` and `raised2` are the same dark grey.
        let mono = mono();
        assert_eq!(mono.muted, mono.raised2);
        assert_eq!(ink(&mono, mono.muted, mono.raised2), mono.text);
        assert_ne!(mono.text, mono.raised2);
    }

    #[test]
    fn put_stops_at_the_edge_and_says_where_it_ended() {
        let spans = [Span::raw("abc"), Span::raw("def")];
        assert_eq!(width(&spans), 6);
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 1));
        assert_eq!(put(&mut buf, 1, 0, &spans, 5), 5);
        assert_eq!(row(&buf), " abcd   ");
        assert_eq!(put(&mut buf, 1, 0, &spans, 8), 7);
    }

    const ROUNDED: Look = Look {
        pill_caps: true,
        corners: Corners::Rounded,
    };
    const KINDS: [ButtonKind; 4] = [
        ButtonKind::Primary,
        ButtonKind::Danger,
        ButtonKind::Normal,
        ButtonKind::Ghost,
    ];

    fn is_rgb(color: Color) -> bool {
        matches!(color, Color::Rgb(..))
    }

    /// A buffer `width` by `height` with every cell an underlined `x` on
    /// `GROUND`: a screen with something already drawn on it.
    fn screen(width: u16, height: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        let drawn = Style::new()
            .fg(Color::Red)
            .bg(GROUND)
            .add_modifier(Modifier::UNDERLINED);
        for y in 0..height {
            buf.set_string(0, y, "x".repeat(usize::from(width)), drawn);
        }
        buf
    }

    fn line(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    /// Asserts the cell at `(x, y)` is a card's edge: `glyph` in `edge` on the
    /// ground that was there, with nothing left of what it was drawn over.
    fn assert_edge(buf: &Buffer, (x, y): (u16, u16), glyph: &str, edge: Color, lit: bool) {
        let cell = &buf[(x, y)];
        assert_eq!(cell.symbol(), glyph, "{x},{y}");
        assert_eq!((cell.fg, cell.bg), (edge, GROUND), "{x},{y}");
        let weight = if lit {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };
        assert_eq!(cell.modifier, weight, "{x},{y}");
    }

    #[test]
    fn a_button_is_a_pill_of_its_label_and_key_in_its_kinds_colours() {
        for (look, theme, round) in looks().into_iter().take(2) {
            let (danger_fill, danger_ink) = danger(&theme);
            let colours = [
                (theme.accent, theme.acc_ink, theme.acc_ink),
                (danger_fill, danger_ink, danger_ink),
                (theme.btn, theme.strong, theme.accent),
                (theme.surface, theme.text, theme.accent),
            ];
            for (kind, (fill, fg, key_fg)) in KINDS.into_iter().zip(colours) {
                let button = button(look, &theme, "Save", "S", kind, false, GROUND);
                // As wide as ` Save S `.
                assert_eq!(width(&button), 8, "{kind:?}");
                let buf = drawn(&button, 8);
                assert_end(&buf, 0, CAP_L, round, fill, GROUND);
                assert_end(&buf, 7, CAP_R, round, fill, GROUND);
                let inside: String = (1..7).map(|x| buf[(x, 0)].symbol()).collect();
                assert_eq!(inside, "Save S", "{kind:?}");
                for x in 1..7 {
                    let cell = &buf[(x, 0)];
                    assert_eq!(cell.bg, fill, "{kind:?} {x}");
                    assert!(cell.modifier.contains(Modifier::BOLD), "{kind:?} {x}");
                    assert!(!cell.modifier.contains(Modifier::REVERSED), "{kind:?} {x}");
                }
                assert_eq!(buf[(1, 0)].fg, fg, "{kind:?}");
                assert_eq!(buf[(6, 0)].fg, key_fg, "{kind:?}");
                if !round {
                    assert_no_caps(&buf);
                }
            }
        }
    }

    #[test]
    fn a_button_without_a_key_is_just_its_label() {
        let theme = Theme::default();
        for kind in KINDS {
            let button = button(SQUARE, &theme, "Open", "", kind, false, GROUND);
            assert_eq!(text(&button), " Open ", "{kind:?}");
            assert_eq!(width(&button), 6, "{kind:?}");
        }
        let button = button(
            ROUND,
            &mono(),
            "Open",
            "",
            ButtonKind::Normal,
            false,
            GROUND,
        );
        assert_eq!(text(&button), " Open ");
    }

    #[test]
    fn hover_lights_a_normal_or_ghost_button_and_no_other() {
        let theme = Theme::default();
        let fill = |kind, hovered| {
            let button = button(SQUARE, &theme, "Save", "S", kind, hovered, GROUND);
            drawn(&button, 8)[(3, 0)].bg
        };
        assert_ne!(theme.hov, theme.btn);
        assert_eq!(fill(ButtonKind::Normal, true), theme.hov);
        assert_eq!(fill(ButtonKind::Ghost, true), theme.hov);
        assert_eq!(fill(ButtonKind::Primary, true), theme.accent);
        assert_eq!(fill(ButtonKind::Danger, true), danger(&theme).0);
        // The text keeps its colours on the lit fill.
        let button = button(
            SQUARE,
            &theme,
            "Save",
            "S",
            ButtonKind::Normal,
            true,
            GROUND,
        );
        let buf = drawn(&button, 8);
        assert_eq!(
            (buf[(1, 0)].fg, buf[(6, 0)].fg),
            (theme.strong, theme.accent)
        );
        assert_end(&buf, 0, CAP_L, false, theme.hov, GROUND);
    }

    #[test]
    fn in_mono_a_loud_button_is_reverse_video_and_the_others_bold() {
        let mono = mono();
        let ground = mono.raised;
        for look in [ROUND, SQUARE, ROUNDED] {
            for (kind, loud) in KINDS.into_iter().zip([true, true, false, false]) {
                for hovered in [false, true] {
                    let button = button(look, &mono, "Save", "S", kind, hovered, ground);
                    // The same width as with colours, so nothing moves.
                    assert_eq!(width(&button), 8, "{kind:?}");
                    let buf = drawn(&button, 8);
                    assert_eq!(row(&buf), " Save S ", "{kind:?}");
                    for x in 0..8 {
                        let cell = &buf[(x, 0)];
                        assert!(cell.modifier.contains(Modifier::BOLD), "{kind:?} {x}");
                        assert_eq!(
                            cell.modifier.contains(Modifier::REVERSED),
                            loud,
                            "{kind:?} {x}"
                        );
                        // Terminal colours only: nothing blended.
                        assert_eq!((cell.fg, cell.bg), (mono.strong, ground), "{kind:?} {x}");
                        assert!(!is_rgb(cell.fg) && !is_rgb(cell.bg), "{kind:?} {x}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_danger_buttons_text_reads_on_its_fill_in_every_theme() {
        for name in crate::theme::NAMES.iter().filter(|name| **name != "mono") {
            let theme = Theme::named(name).expect("a listed theme exists");
            let (fill, ink) = danger(&theme);
            assert!(
                contrast(ink, fill) >= DANGER_CONTRAST,
                "{name}: {ink:?} on {fill:?} is {}",
                contrast(ink, fill)
            );
            assert!(ink == theme.bg || ink == theme.strong, "{name}");
        }
        // Neither Tinkery theme's `err` needs deepening.
        for name in ["tinkery", "tinkery-light"] {
            let theme = Theme::named(name).expect("a listed theme exists");
            assert_eq!(danger(&theme), (theme.err, theme.bg), "{name}");
        }
        // A red neither ink reads on is deepened until one does.
        let grey = Color::Rgb(0x76, 0x76, 0x76);
        let theme = Theme {
            err: grey,
            ..Theme::default()
        };
        assert!(contrast(theme.bg, grey) < DANGER_CONTRAST);
        assert!(contrast(theme.strong, grey) < DANGER_CONTRAST);
        let (fill, ink) = danger(&theme);
        assert_ne!(fill, grey);
        assert_eq!(ink, theme.strong);
        assert!(contrast(ink, fill) >= DANGER_CONTRAST);
        // One step short of it, the text didn't read yet.
        let steps = (1..=DANGER_STEPS)
            .find(|steps| {
                (0..*steps).fold(grey, |c, _| mix(c, Color::Rgb(0, 0, 0), DANGER_STEP)) == fill
            })
            .expect("the fill is `err` deepened a whole number of steps");
        let before = (1..steps).fold(grey, |c, _| mix(c, Color::Rgb(0, 0, 0), DANGER_STEP));
        assert!(contrast(theme.strong, before) < DANGER_CONTRAST);
    }

    #[test]
    fn contrast_is_the_wcag_ratio() {
        let (black, white) = (Color::Rgb(0, 0, 0), Color::Rgb(255, 255, 255));
        assert!((contrast(black, white) - 21.0).abs() < 1e-9);
        assert!((contrast(white, black) - 21.0).abs() < 1e-9);
        assert!((contrast(white, white) - 1.0).abs() < 1e-9);
        let grey = contrast(Color::Rgb(0x76, 0x76, 0x76), white);
        assert!((grey - 4.54).abs() < 0.01, "{grey}");
    }

    #[test]
    fn a_flush_card_is_eighth_blocks_on_the_ground_with_blank_corners() {
        for theme in [Theme::default(), mono()] {
            let mut buf = screen(14, 6);
            let card_look = Card::new(&theme, "");
            assert_eq!(
                (card_look.edge, card_look.fill, card_look.lit),
                (theme.line2, theme.raised, false)
            );
            let inside = card(ROUND, &theme, &mut buf, Rect::new(1, 1, 12, 4), &card_look);
            assert_eq!(inside, Rect::new(2, 2, 10, 2));
            assert_eq!(line(&buf, 0), "x".repeat(14));
            assert_eq!(line(&buf, 1), format!("x {} x", "▁".repeat(10)));
            assert_eq!(line(&buf, 2), format!("x▕{}▏x", " ".repeat(10)));
            assert_eq!(line(&buf, 3), format!("x▕{}▏x", " ".repeat(10)));
            assert_eq!(line(&buf, 4), format!("x {} x", "▔".repeat(10)));
            assert_eq!(line(&buf, 5), "x".repeat(14));
            // The edge sits on the ground: its cells keep the background that
            // was there and nothing else of what they covered.
            for x in 2..12 {
                assert_edge(&buf, (x, 1), "▁", theme.line2, false);
                assert_edge(&buf, (x, 4), "▔", theme.line2, false);
            }
            for y in 2..4 {
                assert_edge(&buf, (1, y), "▕", theme.line2, false);
                assert_edge(&buf, (12, y), "▏", theme.line2, false);
            }
            for at in [(1, 1), (12, 1), (1, 4), (12, 4)] {
                assert_edge(&buf, at, " ", theme.line2, false);
            }
            // The inside is the fill, emptied.
            for y in inside.top()..inside.bottom() {
                for x in inside.left()..inside.right() {
                    let cell = &buf[(x, y)];
                    assert_eq!((cell.symbol(), cell.bg), (" ", theme.raised), "{x},{y}");
                    assert_eq!(cell.fg, theme.text, "{x},{y}");
                    assert!(cell.modifier.is_empty(), "{x},{y}");
                }
            }
            // Outside it nothing changed.
            assert!(buf[(0, 2)].modifier.contains(Modifier::UNDERLINED));
            assert_eq!(buf[(13, 2)].fg, Color::Red);
        }
    }

    #[test]
    fn a_rounded_card_is_box_lines_with_round_corners() {
        for theme in [Theme::default(), mono()] {
            let mut buf = screen(14, 6);
            let inside = card(
                ROUNDED,
                &theme,
                &mut buf,
                Rect::new(1, 1, 12, 4),
                &Card::new(&theme, ""),
            );
            // The same inside as a flush card, so nothing in it moves.
            assert_eq!(inside, Rect::new(2, 2, 10, 2));
            assert_eq!(line(&buf, 1), format!("x╭{}╮x", "─".repeat(10)));
            assert_eq!(line(&buf, 2), format!("x│{}│x", " ".repeat(10)));
            assert_eq!(line(&buf, 4), format!("x╰{}╯x", "─".repeat(10)));
            assert_edge(&buf, (1, 1), "╭", theme.line2, false);
            assert_edge(&buf, (12, 4), "╯", theme.line2, false);
            assert_edge(&buf, (1, 2), "│", theme.line2, false);
            assert_eq!(buf[(5, 2)].bg, theme.raised);
        }
    }

    #[test]
    fn a_cards_title_sits_in_its_top_edge() {
        let theme = Theme::default();
        for (look, top, corner) in [(ROUND, "▁", " "), (ROUNDED, "─", "╭")] {
            // Resting: the title in `text`, bold, on an edge that isn't.
            let mut buf = screen(20, 4);
            let area = Rect::new(0, 0, 20, 4);
            card(look, &theme, &mut buf, area, &Card::new(&theme, "Find"));
            let rest = top.repeat(11);
            assert_eq!(
                line(&buf, 0),
                format!("{corner}{top} Find {rest}{}", {
                    if corner == " " { " " } else { "╮" }
                })
            );
            assert_edge(&buf, (1, 0), top, theme.line2, false);
            for x in 2..8 {
                let cell = &buf[(x, 0)];
                assert_eq!((cell.fg, cell.bg), (theme.text, GROUND), "{x}");
                assert_eq!(cell.modifier, Modifier::BOLD, "{x}");
            }
            assert_edge(&buf, (8, 0), top, theme.line2, false);

            // Lit: edge and title in the colour asked for, both bold.
            let mut buf = screen(20, 4);
            let lit = Card::new(&theme, "Find").lit(theme.accent);
            assert_eq!(
                (lit.edge, lit.fill, lit.lit),
                (theme.accent, theme.raised, true)
            );
            card(look, &theme, &mut buf, area, &lit);
            assert_edge(&buf, (1, 0), top, theme.accent, true);
            assert_eq!(buf[(3, 0)].symbol(), "F");
            assert_eq!(buf[(3, 0)].fg, theme.accent);
            assert_eq!(buf[(3, 0)].modifier, Modifier::BOLD);
            assert_edge(
                &buf,
                (0, 1),
                if corner == " " { "▕" } else { "│" },
                theme.accent,
                true,
            );

            // Too long, it is cut four cells short of the right edge.
            let mut buf = screen(20, 4);
            let long = Card::new(&theme, "Find in the whole project");
            card(look, &theme, &mut buf, area, &long);
            let drawn = line(&buf, 0);
            let cells: Vec<char> = drawn.chars().collect();
            assert_eq!(cells[2..16].iter().collect::<String>(), " Find in the w");
            assert_eq!(cells[16..19].iter().collect::<String>(), top.repeat(3));
        }
        // A card with no room for a title has none; its edge is whole.
        let mut buf = screen(8, 3);
        card(
            ROUND,
            &theme,
            &mut buf,
            Rect::new(0, 0, 8, 3),
            &Card::new(&theme, "Find"),
        );
        assert_eq!(line(&buf, 0), format!(" {} ", "▁".repeat(6)));
    }

    #[test]
    fn a_card_too_small_to_have_an_inside_draws_nothing() {
        let theme = Theme::default();
        for area in [Rect::new(0, 0, 3, 4), Rect::new(0, 0, 8, 1)] {
            let mut buf = screen(10, 5);
            let inside = card(ROUND, &theme, &mut buf, area, &Card::new(&theme, "x"));
            assert!(inside.is_empty(), "{area:?}");
            assert_eq!(buf, screen(10, 5), "{area:?}");
        }
        // Hanging off the buffer, it is drawn where it shows.
        let mut buf = screen(10, 5);
        let inside = card(
            ROUND,
            &theme,
            &mut buf,
            Rect::new(4, 2, 20, 20),
            &Card::new(&theme, ""),
        );
        assert_eq!(inside, Rect::new(5, 3, 4, 1));
    }

    #[test]
    fn a_panel_goes_in_the_middle_a_cell_inside_the_screen() {
        let screen = Rect::new(0, 0, 100, 30);
        assert_eq!(centred(screen, 46, 9), Rect::new(27, 10, 46, 9));
        assert_eq!(centred(screen, 44, 7), Rect::new(28, 11, 44, 7));
        // Offset areas are centred in themselves.
        assert_eq!(
            centred(Rect::new(10, 5, 20, 10), 10, 4),
            Rect::new(15, 8, 10, 4)
        );
        // Too big, it is clamped a cell inside on every side.
        assert_eq!(
            centred(Rect::new(0, 0, 40, 6), 46, 9),
            Rect::new(1, 1, 38, 4)
        );
        assert!(centred(Rect::new(0, 0, 1, 1), 46, 9).is_empty());
    }

    #[test]
    fn a_panel_is_a_centred_card_with_an_accent_edge_and_title_on_raised() {
        for (theme, look) in [
            (Theme::default(), ROUND),
            (Theme::default(), ROUNDED),
            (mono(), ROUND),
            (mono(), ROUNDED),
        ] {
            let mut buf = screen(40, 12);
            let area = buf.area;
            let inside = panel(look, &theme, &mut buf, area, (20, 6), "Run");
            let outer = centred(area, 20, 6);
            assert_eq!(outer, Rect::new(10, 3, 20, 6));
            assert_eq!(inside, Rect::new(11, 4, 18, 4));
            let rounded = look.corners == Corners::Rounded;
            let (top, side) = if rounded {
                ("─", "│")
            } else {
                ("▁", "▕")
            };
            assert_edge(&buf, (11, 3), top, theme.accent, true);
            assert_edge(&buf, (10, 5), side, theme.accent, true);
            assert_edge(
                &buf,
                (10, 3),
                if rounded { "╭" } else { " " },
                theme.accent,
                true,
            );
            let cells: Vec<char> = line(&buf, 3).chars().collect();
            assert_eq!(cells[12..17].iter().collect::<String>(), " Run ");
            assert_eq!(buf[(13, 3)].fg, theme.accent);
            assert_eq!(buf[(13, 3)].modifier, Modifier::BOLD);
            assert_eq!(buf[(20, 6)].bg, theme.raised);
            assert_eq!(buf[(20, 6)].symbol(), " ");
            // Around it the screen is as it was.
            assert_eq!(buf[(9, 5)].symbol(), "x");
            assert_eq!(buf[(20, 2)].symbol(), "x");
        }
    }

    #[test]
    fn dim_fades_text_into_its_ground_and_darkens_everything() {
        let theme = Theme::default();
        let black = Color::Rgb(0, 0, 0);
        let (fg, bg) = (Color::Rgb(200, 100, 50), Color::Rgb(40, 60, 80));
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 2));
        buf.set_string(0, 0, "ab", Style::new().fg(fg).bg(bg));
        // A cell nothing set colours on is the theme's text on its ground.
        buf.set_string(2, 0, "c", Style::new());
        buf.set_string(3, 0, CAP_L, Style::new().fg(fg).bg(bg));
        buf.set_string(4, 0, CAP_R, Style::new().fg(fg).bg(bg));
        // Reverse video: the text is drawn in `bg` on a ground of `fg`.
        buf.set_string(5, 0, "r", Theme::highlight(bg, fg));
        buf.set_string(0, 1, "z", Style::new().fg(fg).bg(bg));
        dim(&theme, &mut buf, Rect::new(0, 0, 6, 1));

        let text = mix(mix(fg, bg, 0.6), black, 0.45);
        let ground = mix(bg, black, 0.45);
        for x in 0..2 {
            assert_eq!((buf[(x, 0)].fg, buf[(x, 0)].bg), (text, ground), "{x}");
        }
        assert_eq!(
            buf[(2, 0)].fg,
            mix(mix(theme.fg, theme.bg, 0.6), black, 0.45)
        );
        assert_eq!(buf[(2, 0)].bg, mix(theme.bg, black, 0.45));
        // A cap is its pill's fill: it darkens with the pill and doesn't fade
        // into the ground, so the pill keeps its shape.
        for x in 3..5 {
            assert_eq!(buf[(x, 0)].fg, mix(fg, black, 0.45), "{x}");
            assert_eq!(buf[(x, 0)].bg, ground, "{x}");
        }
        assert_eq!(buf[(3, 0)].symbol(), CAP_L);
        // `highlight(bg, fg)` holds the fill in `fg` and the text in `bg`.
        let reversed = &buf[(5, 0)];
        assert!(reversed.modifier.contains(Modifier::REVERSED));
        assert_eq!(reversed.fg, ground);
        assert_eq!(reversed.bg, text);
        // Only `area` is dimmed, and nothing but colours changes.
        assert_eq!((buf[(0, 1)].fg, buf[(0, 1)].bg), (fg, bg));
        assert_eq!(buf[(0, 0)].symbol(), "a");
        assert!(!buf[(0, 0)].modifier.contains(Modifier::DIM));
    }

    #[test]
    fn dim_fades_a_diagnostic_underline_with_the_text() {
        let theme = Theme::default();
        let black = Color::Rgb(0, 0, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
        let area = buf.area;
        buf.set_style(area, Style::new().fg(theme.fg).bg(theme.bg));
        buf[(0, 0)].set_style(
            Style::new()
                .underline_color(theme.err)
                .add_modifier(Modifier::UNDERLINED),
        );
        dim(&theme, &mut buf, area);
        assert_eq!(
            buf[(0, 0)].underline_color,
            mix(mix(theme.err, theme.bg, 0.6), black, 0.45)
        );
        // A cell without an underline colour keeps none.
        assert_eq!(buf[(1, 0)].underline_color, Color::Reset);
    }

    #[test]
    fn dim_in_mono_sets_the_dim_modifier_and_blends_nothing() {
        let mono = mono();
        let mut buf = Buffer::empty(Rect::new(0, 0, 3, 1));
        buf.set_string(0, 0, "ab", Style::new().fg(Color::Gray));
        buf.set_string(2, 0, "c", Theme::highlight(mono.hov, mono.strong));
        let before = buf.clone();
        let area = buf.area;
        dim(&mono, &mut buf, area);
        for x in 0..3 {
            let (cell, was) = (&buf[(x, 0)], &before[(x, 0)]);
            assert!(cell.modifier.contains(Modifier::DIM), "{x}");
            assert_eq!((cell.fg, cell.bg), (was.fg, was.bg), "{x}");
            assert!(!is_rgb(cell.fg) && !is_rgb(cell.bg), "{x}");
            assert_eq!(cell.symbol(), was.symbol(), "{x}");
        }
        assert!(buf[(2, 0)].modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn a_panel_at_a_rect_is_the_panel_drawn_there() {
        let theme = Theme::default();
        let rect = Rect::new(3, 1, 20, 6);
        let mut buf = screen(40, 12);
        let inside = panel_at(ROUND, &theme, &mut buf, rect, "Run");
        assert_eq!(inside, Rect::new(4, 2, 18, 4));
        assert_edge(&buf, (4, 1), "▁", theme.accent, true);
        assert_edge(&buf, (3, 3), "▕", theme.accent, true);
        let cells: Vec<char> = line(&buf, 1).chars().collect();
        assert_eq!(cells[5..10].iter().collect::<String>(), " Run ");
        assert_eq!(buf[(6, 1)].fg, theme.accent);
        assert_eq!(buf[(10, 4)].bg, theme.raised);
        // `panel` is the same thing where `centred` puts it.
        let mut centre = screen(40, 12);
        let area = centre.area;
        panel(ROUND, &theme, &mut centre, area, (20, 6), "Run");
        let mut at = screen(40, 12);
        panel_at(ROUND, &theme, &mut at, centred(area, 20, 6), "Run");
        assert_eq!(centre, at);
    }

    #[test]
    fn a_header_is_a_strip_with_the_mark_and_what_goes_at_the_right() {
        for (look, theme, round) in looks() {
            let mut buf = screen(40, 8);
            let card = Rect::new(2, 1, 36, 6);
            panel_at(look, &theme, &mut buf, card, "");
            let scope = vec![
                Span::styled("cast", Style::new().fg(theme.accent)),
                Span::styled(" · files", Style::new().fg(theme.text)),
            ];
            let field = header(look, &theme, &mut buf, card, scope);
            let y = card.y + 2;
            let fill = theme.raised2;
            // The strip runs the card's inside, on the card's fill.
            assert_eq!(buf[(card.x, y)].symbol(), "▕");
            for (x, cap) in [(card.x + 1, CAP_L), (card.right() - 2, CAP_R)] {
                let cell = &buf[(x, y)];
                if round {
                    assert_eq!(cell.symbol(), cap, "{x}");
                    assert_eq!((cell.fg, cell.bg), (fill, theme.raised), "{x}");
                } else {
                    assert_eq!((cell.symbol(), cell.bg), (" ", fill), "{x}");
                }
            }
            for x in card.x + 2..card.right() - 2 {
                assert_eq!(buf[(x, y)].bg, fill, "{x}");
            }
            // One row only: the rows around it are the card's.
            assert_eq!(buf[(card.x + 6, y - 1)].bg, theme.raised);
            assert_eq!(buf[(card.x + 6, y + 1)].bg, theme.raised);
            assert_eq!(buf[(card.x + 6, y + 1)].symbol(), " ");
            // The mark four cells in, the right text ending four cells in.
            let mark = &buf[(card.x + 4, y)];
            assert_eq!(mark.symbol(), HEADER_MARK);
            assert_eq!((mark.fg, mark.bg), (theme.accent2, fill));
            assert!(mark.modifier.contains(Modifier::BOLD));
            let cells: Vec<char> = line(&buf, y).chars().collect();
            let end = usize::from(card.right() - 4);
            assert_eq!(
                cells[end - 12..end].iter().collect::<String>(),
                "cast · files"
            );
            assert_eq!(cells[end], ' ');
            let name = &buf[(card.right() - 16, y)];
            assert_eq!((name.fg, name.bg), (theme.accent, fill));
            assert_eq!(buf[(card.right() - 5, y)].fg, theme.text);
            // The field: after the mark and a blank, two short of the right.
            assert_eq!(field, Rect::new(card.x + 6, y, 36 - 6 - 4 - 12 - 2, 1));
            // With nothing at the right the field runs to where it would end.
            let mut buf = screen(40, 8);
            let field = header(look, &theme, &mut buf, card, Vec::new());
            assert_eq!(field, Rect::new(card.x + 6, y, 36 - 6 - 4, 1));
        }
        // No room for it: nothing drawn, nowhere to type.
        let theme = Theme::default();
        for card in [Rect::new(0, 0, 11, 6), Rect::new(0, 0, 36, 3)] {
            let mut buf = screen(40, 8);
            assert!(header(ROUND, &theme, &mut buf, card, Vec::new()).is_empty());
            assert_eq!(buf, screen(40, 8), "{card:?}");
        }
        let mut buf = screen(40, 2);
        assert!(header(ROUND, &theme, &mut buf, Rect::new(0, 0, 36, 6), Vec::new()).is_empty());
    }

    /// A row of a list on `GROUND`, drawn over what was there.
    fn list_row(look: Look, theme: &Theme, selected: bool, hovered: bool) -> (Buffer, RowInk) {
        let mut buf = screen(12, 1);
        let ink = sel_row(
            look,
            theme,
            &mut buf,
            Rect::new(1, 0, 10, 1),
            selected,
            hovered,
            GROUND,
        );
        (buf, ink)
    }

    #[test]
    fn a_selected_row_is_a_pill_of_hov_with_the_mark_and_a_strong_bold_label() {
        for (look, theme, round) in looks().into_iter().take(2) {
            let (buf, ink) = list_row(look, &theme, true, false);
            assert_end(&buf, 1, CAP_L, round, theme.hov, GROUND);
            assert_end(&buf, 10, CAP_R, round, theme.hov, GROUND);
            let mark = &buf[(2, 0)];
            assert_eq!(mark.symbol(), SEL_MARK);
            assert_eq!((mark.fg, mark.bg), (theme.accent, theme.hov));
            assert_eq!(mark.modifier, Modifier::BOLD);
            for x in 3..10 {
                let cell = &buf[(x, 0)];
                assert_eq!((cell.symbol(), cell.bg), (" ", theme.hov), "{x}");
                assert!(cell.modifier.is_empty(), "{x}");
            }
            // Around the row nothing changed.
            assert_eq!(buf[(0, 0)].symbol(), "x");
            assert_eq!(buf[(11, 0)].symbol(), "x");
            let bold = Style::new().bg(theme.hov).add_modifier(Modifier::BOLD);
            assert_eq!(ink.label, bold.fg(theme.strong));
            assert_eq!(ink.hit, bold.fg(theme.accent));
            assert_eq!(ink.faint, Style::new().bg(theme.hov).fg(theme.text));
            assert_eq!(
                ink.of(Style::new().fg(theme.err)),
                Style::new().fg(theme.err).bg(theme.hov)
            );
            // Accent is the mark and the matched letters, never the fill.
            assert_ne!(theme.hov, theme.accent);
            if !round {
                assert_no_caps(&buf);
            }
        }
    }

    #[test]
    fn a_hovered_row_is_the_pill_without_the_mark() {
        for (look, theme, round) in looks().into_iter().take(2) {
            let (buf, ink) = list_row(look, &theme, false, true);
            assert_end(&buf, 1, CAP_L, round, theme.hov, GROUND);
            assert_end(&buf, 10, CAP_R, round, theme.hov, GROUND);
            for x in 2..10 {
                let cell = &buf[(x, 0)];
                assert_eq!((cell.symbol(), cell.bg), (" ", theme.hov), "{x}");
            }
            // Lit, but not the row Enter acts on: its label keeps its weight.
            assert_eq!(ink.label, Style::new().bg(theme.hov).fg(theme.fg));
            assert_eq!(ink.faint, Style::new().bg(theme.hov).fg(theme.text));
            // Selected and hovered at once is the selected row.
            assert_eq!(
                list_row(look, &theme, true, true),
                list_row(look, &theme, true, false)
            );
        }
    }

    #[test]
    fn the_row_under_the_pointer_is_any_row_inside_the_cards_edges() {
        let card = Rect::new(10, 4, 20, 6);
        let under = |x, y| row_under(card, Position::new(x, y));
        // Rows 5 to 8, columns 11 to 28: a list's rows run edge to edge.
        for y in 5..9 {
            assert_eq!(under(11, y), Some(y));
            assert_eq!(under(28, y), Some(y));
            // The edges themselves, and beside the card, are no row.
            assert_eq!(under(10, y), None);
            assert_eq!(under(29, y), None);
            assert_eq!(under(9, y), None);
        }
        assert_eq!(under(15, 4), None);
        assert_eq!(under(15, 9), None);
        assert_eq!(under(15, 3), None);
        // Anywhere along a row is the same row.
        assert_eq!(under(12, 6), under(27, 6));
        assert_ne!(under(12, 6), under(12, 7));
        // A card with no inside has no rows.
        assert_eq!(row_under(Rect::new(0, 0, 2, 2), Position::new(0, 0)), None);
        assert_eq!(row_under(Rect::new(0, 0, 0, 0), Position::new(0, 0)), None);
    }

    #[test]
    fn a_strips_inside_is_known_without_drawing_it() {
        let theme = Theme::default();
        for area in [
            Rect::new(1, 0, 10, 2),
            Rect::new(0, 0, 3, 1),
            Rect::new(0, 0, 2, 1),
            Rect::new(4, 0, 0, 1),
        ] {
            let mut buf = Buffer::empty(Rect::new(0, 0, 12, 2));
            let drawn = strip(ROUND, &theme, &mut buf, area, FILL, GROUND);
            assert_eq!(strip_inside(area), drawn, "{area:?}");
        }
    }

    #[test]
    fn a_toggle_is_accent_while_on_and_an_off_one_lights_under_the_pointer() {
        for (look, theme, round) in looks().into_iter().take(2) {
            assert_ne!(theme.hov, theme.btn);
            let fills = [
                // On, whatever the pointer is over.
                (true, false, theme.accent, theme.acc_ink, true),
                (true, true, theme.accent, theme.acc_ink, true),
                (false, false, theme.btn, theme.muted, false),
                // `muted` is too close to `hov` to read on it.
                (false, true, theme.hov, theme.text, false),
            ];
            for (on, hovered, fill, fg, bold) in fills {
                let toggle = toggle(look, &theme, "Aa", on, hovered, GROUND);
                // As wide as ` Aa `, lit or not, so nothing beside it moves.
                assert_eq!(width(&toggle), 4, "{on} {hovered}");
                let buf = drawn(&toggle, 4);
                assert_end(&buf, 0, CAP_L, round, fill, GROUND);
                assert_end(&buf, 3, CAP_R, round, fill, GROUND);
                for x in 1..3 {
                    let cell = &buf[(x, 0)];
                    assert_eq!((cell.fg, cell.bg), (fg, fill), "{on} {hovered} {x}");
                    assert_eq!(cell.modifier.contains(Modifier::BOLD), bold, "{x}");
                }
            }
        }
        // `mono` has no fills: the pointer changes nothing.
        let mono = mono();
        for on in [false, true] {
            assert_eq!(
                toggle(ROUND, &mono, "Aa", on, true, GROUND),
                toggle(ROUND, &mono, "Aa", on, false, GROUND),
                "{on}"
            );
        }
    }

    #[test]
    fn a_row_neither_selected_nor_hovered_is_left_alone() {
        for (look, theme, _) in looks() {
            let (buf, ink) = list_row(look, &theme, false, false);
            assert_eq!(buf, screen(12, 1));
            let on = Style::new().bg(GROUND);
            assert_eq!(ink.label, on.fg(theme.fg));
            assert_eq!(ink.hit, on.fg(theme.accent).add_modifier(Modifier::BOLD));
            assert_eq!(ink.faint, on.fg(theme.muted));
            assert_eq!(
                ink.of(Style::new().fg(theme.err)),
                on.fg(theme.err),
                "on the ground"
            );
        }
        // Where `muted` is the ground's own colour, the faint text still reads.
        let mono = mono();
        let mut buf = screen(12, 1);
        let row = Rect::new(1, 0, 10, 1);
        let ink = sel_row(ROUND, &mono, &mut buf, row, false, false, mono.raised2);
        assert_eq!(ink.faint.fg, Some(mono.text));
        // Off the buffer there's nothing to draw.
        let theme = Theme::default();
        let mut buf = screen(12, 1);
        let off = Rect::new(1, 4, 10, 1);
        sel_row(ROUND, &theme, &mut buf, off, true, false, GROUND);
        assert_eq!(buf, screen(12, 1));
    }

    #[test]
    fn in_mono_the_selected_row_is_reverse_video_and_carries_no_colours() {
        let mono = mono();
        for look in [ROUND, SQUARE, ROUNDED] {
            let (buf, ink) = list_row(look, &mono, true, false);
            assert_eq!(row(&buf), format!("x {SEL_MARK}        x"));
            for x in 1..11 {
                let cell = &buf[(x, 0)];
                assert!(cell.modifier.contains(Modifier::REVERSED), "{x}");
                assert!(!cell.modifier.contains(Modifier::UNDERLINED), "{x}");
                // Terminal colours only: nothing blended.
                assert!(!is_rgb(cell.fg) && !is_rgb(cell.bg), "{x}");
            }
            assert!(buf[(2, 0)].modifier.contains(Modifier::BOLD));
            assert!(!buf[(0, 0)].modifier.contains(Modifier::REVERSED));
            assert_no_caps(&buf);
            // Its text has weight and nothing else.
            let weight = Style::new().add_modifier(Modifier::BOLD);
            assert_eq!((ink.label, ink.hit), (weight, weight));
            assert_eq!(ink.faint, Style::new());
            let asked = Style::new()
                .fg(mono.err)
                .bg(mono.raised)
                .add_modifier(Modifier::BOLD);
            assert_eq!(ink.of(asked), weight);
            assert_eq!(ink.of(Style::new().fg(mono.accent)), Style::new());

            // Hovering shows nothing: mono has no fill to show it with.
            let (buf, ink) = list_row(look, &mono, false, true);
            assert_eq!(buf, screen(12, 1));
            assert_eq!(ink.label, Style::new().fg(mono.fg).bg(GROUND));
        }
    }
}
