// SPDX-License-Identifier: Apache-2.0
//! The card list of the editor: geometry of nested paper cards (wrapped text, comments, containers that
//! hold their child cards), drop targets for drag and drop, and drawing. State and input live in `editor`.

use crate::blocks::{CardKind, Row, Slot};
use crate::gfx::Gfx;
use crate::syntax::{self, Piece};
use algoclerk_media::Rect;

/// Pixels a nested card is moved in per level.
const STEP: i32 = 5;
/// Deepest level that still moves further in.
const MAX_DEPTH: usize = 7;
/// Space left of the card text (stripe and icon).
const TEXT_X: i32 = 19;
/// Space right of the card text.
const TEXT_PAD: i32 = 4;

/// Where one card sits.
#[derive(Debug, Clone)]
pub struct CardBox {
    /// Top of the card in list coordinates (before scrolling).
    pub y: i32,
    /// Height.
    pub h: i32,
    /// Left edge.
    pub x: i32,
    /// Width.
    pub w: i32,
    /// The card text, wrapped and coloured.
    pub lines: Vec<Vec<Piece>>,
    /// Comment lines above the text (wrapped).
    pub comments: Vec<String>,
    /// End of the container (index of the last row it holds), for cards with a body.
    pub end: usize,
    /// Extra height added below the card for closing containers.
    pub closing: i32,
}

/// The layout of all cards of a list.
#[derive(Debug, Clone, Default)]
pub struct Geometry {
    /// One box per row.
    pub boxes: Vec<CardBox>,
    /// Total height of the content.
    pub content: i32,
}

fn left_of(list: Rect, depth: usize) -> i32 {
    list.x + 2 + depth.min(MAX_DEPTH) as i32 * STEP
}

fn right_of(list: Rect, depth: usize) -> i32 {
    list.right() - 6 - depth.min(MAX_DEPTH) as i32 * 2
}

/// Index of the last row held by the container that starts at row `i`.
fn container_end(rows: &[Row], i: usize) -> usize {
    let d = rows[i].depth;
    let mut end = i;
    for (j, r) in rows.iter().enumerate().skip(i + 1) {
        let continues = matches!(r.kind, CardKind::Else | CardKind::Foot) || r.chained;
        let inside = r.depth > d || (r.depth == d && continues);
        if !inside {
            break;
        }
        end = j;
    }
    end
}

fn has_children(r: &Row) -> bool {
    r.body.is_some() || r.kind == CardKind::Header
}

/// Lay out `rows` for a list drawn into `list`. `min_h` is the least card height (bigger for fingers).
pub fn geometry(rows: &[Row], list: Rect, min_h: i32) -> Geometry {
    let mut boxes: Vec<CardBox> = Vec::with_capacity(rows.len());
    let mut y = 2;
    let ends: Vec<usize> = (0..rows.len())
        .map(|i| {
            if has_children(&rows[i]) {
                container_end(rows, i)
            } else {
                i
            }
        })
        .collect();
    for (i, r) in rows.iter().enumerate() {
        let x = left_of(list, r.depth);
        let w = (right_of(list, r.depth) - x).max(30);
        let cols = ((w - TEXT_X - TEXT_PAD) / 6).max(4) as usize;
        let pieces = if r.kind == CardKind::Else || r.kind == CardKind::Foot {
            vec![(r.text.clone(), 'K')]
        } else {
            syntax::lines(&r.text, syntax::light, 'K')
                .into_iter()
                .next()
                .unwrap_or_default()
        };
        let lines = syntax::wrap_line(&pieces, cols);
        let mut comments: Vec<String> = Vec::new();
        for c in &r.comments {
            comments.extend(wrap_plain(&format!("//{c}"), cols));
        }
        if let Some(t) = &r.trailing {
            comments.extend(wrap_plain(&format!("//{t}"), cols));
        }
        let h = (5 + lines.len() as i32 * 9 + comments.len() as i32 * 8 - 2).max(min_h);
        let closing = rows
            .iter()
            .enumerate()
            .filter(|(k, _)| *k < i && ends[*k] == i && has_children(&rows[*k]))
            .count() as i32
            * 3;
        boxes.push(CardBox {
            y,
            h,
            x,
            w,
            lines,
            comments,
            end: ends[i],
            closing,
        });
        y += h + 1 + closing;
    }
    Geometry {
        boxes,
        content: y + 2,
    }
}

/// The function has no statement cards yet (only its header and read-only items).
pub fn is_empty_body(rows: &[Row]) -> bool {
    rows.iter().any(|r| r.kind == CardKind::Header)
        && rows
            .iter()
            .all(|r| matches!(r.kind, CardKind::Header | CardKind::Item))
}

/// Where the "add a statement" button goes on an empty body: just under the last card, scrolled with the list.
pub fn empty_slot(rows: &[Row], geo: &Geometry, list: Rect, scroll: i32, h: i32) -> Option<Rect> {
    if !is_empty_body(rows) {
        return None;
    }
    Some(Rect::new(
        list.x + 10,
        list.y + geo.content + 4 - scroll,
        (list.w - 20).max(30),
        h,
    ))
}

/// Wrap a comment to `cols` characters, keeping every character (long words are cut).
pub fn wrap_plain(s: &str, cols: usize) -> Vec<String> {
    let cols = cols.max(4);
    let mut out = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    for ch in chars.chunks(cols) {
        out.push(ch.iter().collect());
    }
    out
}

/// Where a dragged card would land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drop {
    /// The block to drop into.
    pub slot: Slot,
    /// Position in that block (before the card that is there now).
    pub index: usize,
    /// Row the indicator line belongs to, and whether it sits under it (else above it).
    pub row: usize,
    /// The indicator line is below the row.
    pub below: bool,
    /// Depth the dropped card will have (for the indicator's indent).
    pub depth: usize,
}

/// Which drop target a pointer at list coordinates `(px, py)` means.
pub fn drop_at(rows: &[Row], geo: &Geometry, px: i32, py: i32) -> Option<Drop> {
    let i = geo
        .boxes
        .iter()
        .position(|b| py < b.y + b.h + 1 + b.closing)
        .unwrap_or(rows.len().checked_sub(1)?);
    let (r, b) = (rows.get(i)?, geo.boxes.get(i)?);
    let lower = py >= b.y + b.h / 2;
    if !lower {
        let (slot, index) = r.at?;
        return Some(Drop {
            slot,
            index,
            row: i,
            below: false,
            depth: r.depth,
        });
    }
    // the left margin of the cards closing here means "after the container": the outermost margin hit wins
    let mut closers: Vec<usize> = (0..i)
        .filter(|k| geo.boxes[*k].end == i && has_children(&rows[*k]) && rows[*k].at.is_some())
        .collect();
    closers.sort_by_key(|k| rows[*k].depth);
    for k in closers {
        let child_x = geo.boxes[k].x + STEP;
        if px < child_x {
            let (slot, index) = rows[k].at?;
            return Some(Drop {
                slot,
                index: index + 1,
                row: i,
                below: true,
                depth: rows[k].depth,
            });
        }
    }
    if let Some(body) = r.body.filter(|_| r.kind != CardKind::Raw) {
        return Some(Drop {
            slot: body,
            index: 0,
            row: i,
            below: true,
            depth: r.depth + 1,
        });
    }
    if r.kind == CardKind::Foot {
        return None;
    }
    let (slot, index) = r.at?;
    Some(Drop {
        slot,
        index: index + 1,
        row: i,
        below: true,
        depth: r.depth,
    })
}

/// What the list shows besides the cards.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    /// The list area.
    pub list: Rect,
    /// Scroll in pixels.
    pub scroll: i32,
    /// Selected row.
    pub sel: Option<usize>,
    /// Statement being executed.
    pub executing: Option<u32>,
    /// The program does not compile (the selected card is shown in red).
    pub broken: bool,
    /// Row being dragged.
    pub dragging: Option<usize>,
    /// Where the dragged card would land.
    pub drop: Option<Drop>,
}

/// The rectangle of a card on screen.
pub fn card_rect(geo: &Geometry, i: usize, list: Rect, scroll: i32) -> Option<Rect> {
    let b = geo.boxes.get(i)?;
    Some(Rect::new(b.x, list.y + b.y - scroll, b.w, b.h))
}

fn colours(kind: CardKind) -> (char, char) {
    match kind {
        CardKind::Header => ('k', 'm'),
        CardKind::Compound => ('W', 'B'),
        CardKind::Else => ('W', 'B'),
        CardKind::Simple => ('W', 'g'),
        CardKind::Foot => ('W', 'B'),
        CardKind::Raw => ('w', 'o'),
        CardKind::Item => ('A', 'a'),
    }
}

/// Draw the cards (the caller clips to the list).
pub fn draw(g: &mut Gfx<'_>, rows: &[Row], geo: &Geometry, look: &Look) {
    let list = look.list;
    let top = |i: usize| list.y + geo.boxes[i].y - look.scroll;
    // containers first, outermost before inner ones so the inner frames sit on top
    for (i, r) in rows.iter().enumerate() {
        if !(r.body.is_some() && r.kind != CardKind::Raw) || r.kind == CardKind::Header {
            continue;
        }
        let b = &geo.boxes[i];
        let end = b.end;
        let closers_inner = (0..rows.len())
            .filter(|k| {
                *k != i
                    && geo.boxes[*k].end == end
                    && rows[*k].body.is_some()
                    && rows[*k].kind != CardKind::Raw
                    && rows[*k].kind != CardKind::Header
                    && (rows[*k].depth, *k) > (r.depth, i)
            })
            .count() as i32;
        let bottom = top(end) + geo.boxes[end].h + 3 * (1 + closers_inner);
        let frame = Rect::new(b.x, top(i), b.w, bottom - top(i));
        if frame.bottom() < list.y || frame.y > list.bottom() {
            continue;
        }
        let stripe = if r.kind == CardKind::Else {
            'B'
        } else {
            colours(r.kind).1
        };
        g.round_rect(frame, 'w', stripe);
    }
    for (i, r) in rows.iter().enumerate() {
        let b = &geo.boxes[i];
        let rect = Rect::new(b.x, top(i), b.w, b.h);
        if rect.bottom() < list.y || rect.y > list.bottom() {
            continue;
        }
        let selected = look.sel == Some(i);
        let running = look.executing == Some(r.node) && r.kind != CardKind::Else;
        let (fill, stripe) = colours(r.kind);
        let fill = if running {
            'c'
        } else if selected {
            'y'
        } else {
            fill
        };
        let faded = look.dragging == Some(i);
        g.round_rect(
            rect,
            if faded { 'A' } else { fill },
            if selected { 'R' } else { 'K' },
        );
        g.rect(Rect::new(rect.x + 1, rect.y + 1, 3, rect.h - 2), stripe);
        let ink = if r.kind == CardKind::Header { 'W' } else { 'K' };
        let content_h = b.comments.len() as i32 * 8 + b.lines.len() as i32 * 9 - 2;
        let mut ty = rect.y + ((rect.h - content_h) / 2).max(1);
        for c in &b.comments {
            g.text(
                rect.x + TEXT_X,
                ty,
                c,
                syntax::light(algoclerk_core::cpp::lexer::HlKind::Comment),
            );
            ty += 8;
        }
        let text_y = ty;
        if r.kind != CardKind::Else && b.h >= 12 {
            g.icon(
                r.icon,
                rect.x + 6,
                text_y,
                if r.kind == CardKind::Header { 'W' } else { 'k' },
            );
        }
        for l in &b.lines {
            let mut x = rect.x + TEXT_X;
            for (t, col) in l {
                let col = if r.kind == CardKind::Header {
                    'W'
                } else if look.broken && selected {
                    'R'
                } else if r.kind == CardKind::Else || r.kind == CardKind::Foot {
                    ink
                } else {
                    *col
                };
                g.text(x, ty, t, col);
                x += t.chars().count() as i32 * 6;
            }
            ty += 9;
        }
        if running {
            g.cursor(rect.right() - 11, rect.y + (rect.h - 9) / 2);
        }
    }
    if let (Some(d), true) = (look.drop, look.dragging.is_some()) {
        let b = &geo.boxes[d.row];
        let y = if d.below {
            top(d.row) + b.h + 1
        } else {
            top(d.row) - 1
        };
        let x = left_of(list, d.depth);
        let w = (right_of(list, d.depth) - x).max(10);
        g.rect(Rect::new(x, y, w, 2), 'R');
        g.rect(Rect::new(x - 2, y - 1, 3, 4), 'R');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::rows;
    use crate::session::parse_with_prelude;

    const SRC: &str = "int f(int n) {\n    int t = 0;\n    for (int i = 0; i < n; ++i) {\n        if (i > 2) {\n            t += i;\n        } else {\n            t -= 1;\n        }\n    }\n    return t;\n}\n";

    fn setup() -> (Vec<Row>, Geometry, Rect) {
        let p = parse_with_prelude("", SRC).unwrap();
        let r = rows(&p);
        let list = Rect::new(0, 0, 150, 200);
        let g = geometry(&r, list, 14);
        (r, g, list)
    }

    fn mid(g: &Geometry, i: usize, lower: bool) -> i32 {
        let b = &g.boxes[i];
        if lower { b.y + b.h - 2 } else { b.y + 1 }
    }

    #[test]
    fn nested_cards_sit_inside_their_containers() {
        let (r, g, _) = setup();
        for (i, row) in r.iter().enumerate() {
            if row.body.is_some() && row.kind != CardKind::Header {
                let end = g.boxes[i].end;
                assert!(end > i, "{}", row.text);
                for k in i + 1..=end {
                    assert!(g.boxes[k].x >= g.boxes[i].x, "{}", r[k].text);
                    assert!(g.boxes[k].x + g.boxes[k].w <= g.boxes[i].x + g.boxes[i].w);
                }
            }
        }
        assert!(g.boxes.windows(2).all(|w| w[0].y + w[0].h <= w[1].y));
    }

    #[test]
    fn drop_targets_follow_the_pointer() {
        let (r, g, _) = setup();
        let at = |text: &str| r.iter().position(|x| x.text == text).unwrap();
        let lp = at("for (int i = 0; i < n; ++i)");
        // top half: before the card, in its own block
        let d = drop_at(&r, &g, 40, mid(&g, lp, false)).unwrap();
        assert_eq!((d.slot, d.index), (r[lp].at.unwrap().0, 1));
        // lower half of a container: first place inside it
        let d = drop_at(&r, &g, 40, mid(&g, lp, true)).unwrap();
        assert_eq!((d.slot, d.index), (r[lp].body.unwrap(), 0));
        // the else label opens the else block
        let el = r.iter().position(|x| x.kind == CardKind::Else).unwrap();
        let d = drop_at(&r, &g, 40, mid(&g, el, true)).unwrap();
        assert_eq!((d.slot, d.index), (r[el].body.unwrap(), 0));
        // the left margin under the last card of the for loop means "after the loop"
        let last = r.iter().position(|x| x.text == "t -= 1;").unwrap();
        let d = drop_at(&r, &g, g.boxes[lp].x + 1, mid(&g, last, true)).unwrap();
        assert_eq!((d.slot, d.index), (r[lp].at.unwrap().0, 2));
        // below everything: the end of the function body is still a target
        assert!(drop_at(&r, &g, 40, 10_000).is_some());
    }

    #[test]
    fn long_cards_wrap_and_keep_every_character() {
        let src = "int f(int n) {\n    // explains\n    auto veryLongVariableName = static_cast<long long>(n) * static_cast<long long>(n) + 12345678;\n    return 0;\n}\n";
        let p = parse_with_prelude("", src).unwrap();
        let r = rows(&p);
        let g = geometry(&r, Rect::new(0, 0, 100, 200), 14);
        let i = r.iter().position(|x| x.text.starts_with("auto ")).unwrap();
        assert!(g.boxes[i].lines.len() >= 3);
        assert_eq!(g.boxes[i].comments.concat(), "// explains");
        let joined: String = g.boxes[i]
            .lines
            .iter()
            .flatten()
            .map(|p| p.0.as_str())
            .collect();
        assert_eq!(joined.replace(' ', ""), r[i].text.replace(' ', ""));
    }
}
