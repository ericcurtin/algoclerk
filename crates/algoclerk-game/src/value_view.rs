// SPDX-License-Identifier: Apache-2.0
//! Pixel-art widgets for C++ values: ints are crates, chars are letter tiles, strings are ribbons of
//! tiles, vectors and arrays are shelves, maps are filing-cabinet drawers, sets are bins, stacks are spring
//! plates, queues and deques are belts, priority queues are pyramids, `ListNode*` are chains of tags and
//! `TreeNode*` are org charts.

use crate::gfx::Gfx;
use algoclerk_core::cpp::ast::Ty;
use algoclerk_core::cpp::value::{Heap, Plain, render};
use algoclerk_media::Rect;
use std::collections::VecDeque;

/// A laid-out widget.
#[derive(Debug, Clone)]
pub struct W {
    /// Width in virtual pixels.
    pub w: i32,
    /// Height in virtual pixels.
    pub h: i32,
    k: K,
}

#[derive(Debug, Clone)]
enum K {
    Crate(String, bool),
    Tile(char),
    Plate(String, char),
    Stamp(bool),
    Shelf {
        cells: Vec<(String, W)>,
        gap_at: Option<usize>,
        total: usize,
    },
    Rack(Vec<W>, usize),
    Drawers {
        rows: Vec<(W, W)>,
        hidden: usize,
    },
    Bin {
        items: Vec<W>,
        hidden: usize,
    },
    Plates {
        items: Vec<W>,
        hidden: usize,
    },
    Belt {
        items: Vec<W>,
        hidden: usize,
        double: bool,
    },
    Pyramid(Vec<Vec<W>>),
    Group(Vec<W>),
    Tag {
        label: &'static str,
        inner: Option<Box<W>>,
    },
    Chain {
        vals: Vec<String>,
        cycle: Option<usize>,
        more: bool,
    },
    Org {
        nodes: Vec<(i32, i32, String)>,
        edges: Vec<(usize, usize)>,
        more: bool,
    },
    Row(Vec<W>),
}

const CELL: i32 = 14;

fn crate_w(label: &str) -> i32 {
    (label.chars().count() as i32 * 4 + 5).max(16)
}

fn tag(label: &'static str) -> W {
    W {
        w: (label.len() as i32 * 4 + 10).max(22),
        h: 12,
        k: K::Tag { label, inner: None },
    }
}

fn tile(c: u8) -> W {
    let ch = if (0x20..0x7f).contains(&c) {
        c as char
    } else {
        '?'
    };
    W {
        w: 9,
        h: 11,
        k: K::Tile(ch),
    }
}

fn int_widget(i: i128) -> W {
    let label = if i.abs() >= 100_000 {
        format!("{}k", i / 1000)
    } else {
        i.to_string()
    };
    W {
        w: crate_w(&label),
        h: 16,
        k: K::Crate(label, i < 0),
    }
}

fn string_widget(s: &[u8], max_w: i32) -> W {
    let max_cells = ((max_w - 4) / 9).clamp(3, 24) as usize;
    let cell = |c: &u8| (String::new(), tile(*c));
    if s.len() <= max_cells {
        let cells: Vec<(String, W)> = s.iter().map(cell).collect();
        let w = cells.len() as i32 * 9 + 4;
        return W {
            w: w.max(13),
            h: 15,
            k: K::Shelf {
                cells,
                gap_at: None,
                total: s.len(),
            },
        };
    }
    let head = max_cells - 3;
    let mut cells: Vec<(String, W)> = s[..head].iter().map(cell).collect();
    cells.extend(s[s.len() - 2..].iter().map(cell));
    W {
        w: (cells.len() as i32 + 1) * 9 + 4,
        h: 15,
        k: K::Shelf {
            cells,
            gap_at: Some(head),
            total: s.len(),
        },
    }
}

/// Element type of a sequence-like container.
fn elem_ty(t: &Ty) -> Option<&Ty> {
    match t {
        Ty::Vector(e)
        | Ty::Array(e, _)
        | Ty::Deque(e)
        | Ty::List(e)
        | Ty::Stack(e)
        | Ty::Queue(e)
        | Ty::Set(e)
        | Ty::UnorderedSet(e)
        | Ty::PriorityQueue(e, _) => Some(e),
        _ => None,
    }
}

fn is_grid_elem(t: &Ty) -> bool {
    matches!(t, Ty::Vector(_) | Ty::Array(..))
}

/// Build the widget for a value inside a width budget. `heap` holds the struct instances behind pointers.
pub fn build(v: &Plain, t: &Ty, heap: &Heap, max_w: i32) -> W {
    match (v, t) {
        (Plain::Uninit, _) => tag("uninit"),
        (_, Ty::Iter { .. }) => tag("iterator"),
        (Plain::Void, _) => tag("..."),
        (Plain::Int(i), Ty::Bool) => W {
            w: 14,
            h: 14,
            k: K::Stamp(*i != 0),
        },
        (Plain::Int(i), Ty::Char) => tile(*i as u8),
        (Plain::Int(i), _) => int_widget(*i),
        (Plain::Str(s), _) => string_widget(s, max_w),
        (Plain::Seq(items), Ty::Pair(a, b)) if items.len() == 2 => {
            group(items, &[(**a).clone(), (**b).clone()], heap, max_w)
        }
        (Plain::Seq(items), Ty::Tuple(ts)) => group(items, ts, heap, max_w),
        (Plain::Seq(items), Ty::Stack(et)) => plates(items, et, heap, max_w),
        (Plain::Seq(items), Ty::Queue(et) | Ty::Deque(et)) => {
            belt(items, et, heap, max_w, matches!(t, Ty::Deque(_)))
        }
        (Plain::Seq(items), Ty::PriorityQueue(et, _)) => pyramid(items, et, heap),
        (Plain::Seq(items), Ty::Set(et) | Ty::UnorderedSet(et)) => bin(items, et, heap, max_w),
        (Plain::Seq(items), _) => match elem_ty(t) {
            Some(et) => vec_widget(items, et, heap, max_w),
            None => other(t),
        },
        (Plain::Map(items), Ty::Map(kt, vt) | Ty::UnorderedMap(kt, vt)) => {
            drawers(items, kt, vt, heap, max_w)
        }
        (Plain::Opt(None), _) => tag("nullopt"),
        (Plain::Opt(Some(x)), Ty::Optional(et)) => {
            let inner = build(x, et, heap, max_w - 24);
            W {
                w: inner.w + 6,
                h: inner.h + 8,
                k: K::Tag {
                    label: "some",
                    inner: Some(Box::new(inner)),
                },
            }
        }
        (Plain::Ptr(h), Ty::Ptr(inner)) => pointer(*h, inner, heap, max_w),
        (Plain::Struct(_), Ty::Struct(name)) => W {
            w: (name.chars().count() as i32 * 4 + 10).max(24),
            h: 12,
            k: K::Plate(name.chars().take(10).collect(), 'A'),
        },
        _ => other(t),
    }
}

fn other(t: &Ty) -> W {
    let label: String = t.to_string().chars().take(5).collect();
    W {
        w: (label.chars().count() as i32 * 4 + 10).max(24),
        h: 12,
        k: K::Plate(label, 'A'),
    }
}

fn group(items: &[Plain], ts: &[Ty], heap: &Heap, max_w: i32) -> W {
    let each = (max_w / items.len().max(1) as i32).max(30);
    let parts: Vec<W> = items
        .iter()
        .zip(ts.iter())
        .map(|(x, t)| build(x, t, heap, each))
        .collect();
    let w = parts.iter().map(|x| x.w + 3).sum::<i32>() + 6;
    let h = parts.iter().map(|x| x.h).max().unwrap_or(10) + 2;
    W {
        w,
        h,
        k: K::Group(parts),
    }
}

fn plates(items: &[Plain], et: &Ty, heap: &Heap, max_w: i32) -> W {
    let cap = 5usize;
    let shown: Vec<W> = items
        .iter()
        .rev()
        .take(cap)
        .rev()
        .map(|x| build(x, et, heap, max_w - 8))
        .collect();
    let hidden = items.len().saturating_sub(cap);
    let w = shown.iter().map(|x| x.w).max().unwrap_or(10) + 8;
    let h = shown.iter().map(|x| x.h + 3).sum::<i32>() + 6 + if hidden > 0 { 8 } else { 0 };
    W {
        w: w.max(26),
        h,
        k: K::Plates {
            items: shown,
            hidden,
        },
    }
}

fn belt(items: &[Plain], et: &Ty, heap: &Heap, max_w: i32, double: bool) -> W {
    let cap = ((max_w - 12) / 18).clamp(2, 7) as usize;
    let shown: Vec<W> = items
        .iter()
        .take(cap)
        .map(|x| build(x, et, heap, 30))
        .collect();
    let hidden = items.len().saturating_sub(cap);
    let w = shown.iter().map(|x| x.w + 2).sum::<i32>() + 12 + if hidden > 0 { 14 } else { 0 };
    let h = shown.iter().map(|x| x.h).max().unwrap_or(12) + 8;
    W {
        w: w.max(30),
        h,
        k: K::Belt {
            items: shown,
            hidden,
            double,
        },
    }
}

fn pyramid(items: &[Plain], et: &Ty, heap: &Heap) -> W {
    let mut levels: Vec<Vec<W>> = Vec::new();
    let mut i = 0;
    let mut width = 1;
    while i < items.len() && levels.len() < 4 {
        let row: Vec<W> = items[i..(i + width).min(items.len())]
            .iter()
            .map(|x| build(x, et, heap, 30))
            .collect();
        i += width;
        width *= 2;
        levels.push(row);
    }
    let w = levels
        .iter()
        .map(|r| r.iter().map(|x| x.w + 2).sum::<i32>())
        .max()
        .unwrap_or(10)
        + 6;
    let h = levels
        .iter()
        .map(|r| r.iter().map(|x| x.h).max().unwrap_or(0) + 2)
        .sum::<i32>()
        + 4;
    W {
        w: w.max(24),
        h: h.max(14),
        k: K::Pyramid(levels),
    }
}

fn bin(items: &[Plain], et: &Ty, heap: &Heap, max_w: i32) -> W {
    let cap = ((max_w - 8) / 20).clamp(2, 8) as usize;
    let shown: Vec<W> = items
        .iter()
        .take(cap)
        .map(|x| build(x, et, heap, 30))
        .collect();
    let hidden = items.len().saturating_sub(cap);
    let w = shown.iter().map(|x| x.w + 2).sum::<i32>() + 10 + if hidden > 0 { 14 } else { 0 };
    let h = shown.iter().map(|x| x.h).max().unwrap_or(12) + 8;
    W {
        w: w.max(26),
        h,
        k: K::Bin {
            items: shown,
            hidden,
        },
    }
}

fn drawers(items: &[(Plain, Plain)], kt: &Ty, vt: &Ty, heap: &Heap, max_w: i32) -> W {
    let cap = 5usize;
    let rows: Vec<(W, W)> = items
        .iter()
        .take(cap)
        .map(|(k, x)| {
            (
                build(k, kt, heap, 50),
                build(x, vt, heap, (max_w - 70).max(30)),
            )
        })
        .collect();
    let hidden = items.len().saturating_sub(cap);
    let kw = rows.iter().map(|r| r.0.w).max().unwrap_or(10);
    let vw = rows.iter().map(|r| r.1.w).max().unwrap_or(10);
    let h =
        rows.iter().map(|r| r.0.h.max(r.1.h) + 4).sum::<i32>() + 4 + if hidden > 0 { 8 } else { 0 };
    W {
        w: kw + vw + 14,
        h: h.max(18),
        k: K::Drawers { rows, hidden },
    }
}
fn node_label(heap: &Heap, id: u32) -> String {
    match heap.nodes.get(id as usize).and_then(|f| f.first()) {
        Some(Plain::Int(i)) => i.to_string(),
        Some(p) => render(p, &Ty::Unknown, heap).chars().take(4).collect(),
        None => "?".to_string(),
    }
}

fn ptr_field(heap: &Heap, id: u32, idx: usize) -> u32 {
    match heap.nodes.get(id as usize).and_then(|f| f.get(idx)) {
        Some(Plain::Ptr(p)) => *p,
        _ => 0,
    }
}

fn pointer(h: u32, inner: &Ty, heap: &Heap, max_w: i32) -> W {
    if h == 0 {
        return tag("nullptr");
    }
    let name = match inner {
        Ty::Struct(n) => n.as_str(),
        _ => "",
    };
    if name == "TreeNode" {
        return tree_widget(h, heap, max_w);
    }
    let fields = heap.nodes.get(h as usize).map_or(0, Vec::len);
    if name == "ListNode" || (fields == 2 && matches!(heap.nodes[h as usize][1], Plain::Ptr(_))) {
        return chain_widget(h, heap, max_w);
    }
    W {
        w: (name.chars().count() as i32 * 4 + 10).max(24),
        h: 12,
        k: K::Plate(name.chars().take(10).collect(), 'y'),
    }
}

fn chain_widget(head: u32, heap: &Heap, max_w: i32) -> W {
    let mut vals = Vec::new();
    let mut seen: Vec<u32> = Vec::new();
    let mut cur = head;
    let mut cycle = None;
    let mut more = false;
    let cap = ((max_w - 8) / 22).clamp(2, 8) as usize;
    while cur != 0 {
        if let Some(p) = seen.iter().position(|s| *s == cur) {
            cycle = Some(p.min(7));
            break;
        }
        if vals.len() >= cap {
            more = true;
            break;
        }
        seen.push(cur);
        vals.push(node_label(heap, cur));
        cur = ptr_field(heap, cur, 1);
    }
    W {
        w: vals.iter().map(|s| crate_w(s).max(14) + 6).sum::<i32>() + 4 + if more { 10 } else { 0 },
        h: 22,
        k: K::Chain { vals, cycle, more },
    }
}

fn tree_widget(root: u32, heap: &Heap, max_w: i32) -> W {
    // level order, at most 15 nodes
    let mut layout: Vec<(i32, i32, String)> = Vec::new();
    let mut edges = Vec::new();
    let mut q: VecDeque<(u32, i32, i32, i32, Option<usize>)> = VecDeque::new();
    q.push_back((root, 0, 0, 64, None));
    let mut more = false;
    while let Some((n, depth, x, span, parent)) = q.pop_front() {
        if layout.len() >= 15 || depth > 3 {
            more = true;
            continue;
        }
        let id = layout.len();
        layout.push((x, depth, node_label(heap, n)));
        if let Some(p) = parent {
            edges.push((p, id));
        }
        let (l, r) = (ptr_field(heap, n, 1), ptr_field(heap, n, 2));
        if l != 0 {
            q.push_back((l, depth + 1, x - span / 2, span / 2, Some(id)));
        }
        if r != 0 {
            q.push_back((r, depth + 1, x + span / 2, span / 2, Some(id)));
        }
    }
    let min_x = layout.iter().map(|n| n.0).min().unwrap_or(0);
    let max_x = layout.iter().map(|n| n.0).max().unwrap_or(0);
    let max_d = layout.iter().map(|n| n.1).max().unwrap_or(0);
    let scale_x = ((max_w - 24) as f32 / ((max_x - min_x).max(1)) as f32).clamp(0.2, 0.5);
    let nodes_l: Vec<(i32, i32, String)> = layout
        .iter()
        .map(|(x, d, s)| {
            (
                ((x - min_x) as f32 * scale_x) as i32 + 8,
                d * 15 + 7,
                s.clone(),
            )
        })
        .collect();
    let w = nodes_l.iter().map(|n| n.0).max().unwrap_or(10) + 12;
    W {
        w,
        h: (max_d + 1) * 15 + 4,
        k: K::Org {
            nodes: nodes_l,
            edges,
            more,
        },
    }
}

fn vec_widget(items: &[Plain], et: &Ty, heap: &Heap, max_w: i32) -> W {
    if is_grid_elem(et) && !items.is_empty() {
        // a rack of shelves
        let cap = 4usize;
        let rows: Vec<W> = items
            .iter()
            .take(cap)
            .map(|x| build(x, et, heap, max_w - 10))
            .collect();
        let hidden = items.len().saturating_sub(cap);
        let w = rows.iter().map(|r| r.w).max().unwrap_or(10) + 8;
        let h = rows.iter().map(|r| r.h + 1).sum::<i32>() + 4 + if hidden > 0 { 8 } else { 0 };
        return W {
            w,
            h,
            k: K::Rack(rows, hidden),
        };
    }
    if items.is_empty() {
        return W {
            w: 34,
            h: 20,
            k: K::Shelf {
                cells: vec![],
                gap_at: None,
                total: 0,
            },
        };
    }
    let probe = build(&items[0], et, heap, 40);
    let cell_w = probe.w.max(CELL - 2) + 3;
    let cap = ((max_w - 6) / cell_w).clamp(2, 12) as usize;
    let mk = |i: usize, x: &Plain| (i.to_string(), build(x, et, heap, 40));
    let (cells, gap) = if items.len() <= cap {
        (
            items
                .iter()
                .enumerate()
                .map(|(i, x)| mk(i, x))
                .collect::<Vec<_>>(),
            None,
        )
    } else {
        let head = cap.saturating_sub(3).max(1);
        let mut c: Vec<(String, W)> = items
            .iter()
            .take(head)
            .enumerate()
            .map(|(i, x)| mk(i, x))
            .collect();
        for i in items.len() - 2..items.len() {
            c.push(mk(i, &items[i]));
        }
        (c, Some(head))
    };
    let w = cells.iter().map(|c| c.1.w.max(CELL - 2) + 3).sum::<i32>()
        + 6
        + if gap.is_some() { 10 } else { 0 };
    let h = cells.iter().map(|c| c.1.h).max().unwrap_or(10) + 12;
    W {
        w: w.max(24),
        h,
        k: K::Shelf {
            cells,
            gap_at: gap,
            total: items.len(),
        },
    }
}

impl W {
    /// Draw at the top-left corner `(x, y)`.
    pub fn draw(&self, g: &mut Gfx<'_>, x: i32, y: i32, hot: bool) {
        match &self.k {
            K::Crate(label, neg) => {
                g.sprite(
                    if hot {
                        "tiles/crate_c"
                    } else {
                        "tiles/crate_a"
                    },
                    0,
                    x + (self.w - 16) / 2,
                    y,
                );
                let plate = Rect::new(x + (self.w - 16) / 2 + 2, y + 4, 12, 9);
                g.tiny_c(
                    plate.x + plate.w / 2 + (self.w - 16) / 2 * 0,
                    y + 7,
                    label,
                    if *neg { 'R' } else { 'K' },
                );
            }
            K::Tile(c) => {
                g.round_rect(Rect::new(x, y, 9, 11), 'W', 'K');
                g.text(x + 2, y + 2, &c.to_string(), 'K');
            }
            K::Plate(s, fill) => {
                g.round_rect(Rect::new(x, y, self.w, self.h), *fill, 'K');
                g.tiny_c(x + self.w / 2, y + 3, s, 'K');
            }
            K::Stamp(b) => {
                g.round_rect(Rect::new(x, y, 14, 14), if *b { 'g' } else { 'R' }, 'K');
                g.icon(if *b { "tick" } else { "cross" }, x + 3, y + 3, 'W');
            }
            K::Shelf {
                cells,
                gap_at,
                total,
            } => {
                let mut cx = x + 3;
                g.rect(Rect::new(x, y + self.h - 4, self.w, 3), 'N');
                g.hline(x, y + self.h - 4, self.w, 'n');
                for (i, (label, w)) in cells.iter().enumerate() {
                    if Some(i) == *gap_at {
                        g.text(cx, y + self.h - 14, "..", 'K');
                        cx += 10;
                    }
                    let ww = w.w.max(CELL - 2);
                    if !label.is_empty() {
                        g.tiny_c(cx + ww / 2, y, label, 'a');
                    }
                    let top = if label.is_empty() { y + 1 } else { y + 7 };
                    w.draw(g, cx + (ww - w.w) / 2, top, hot);
                    cx += ww + 3;
                }
                if cells.is_empty() {
                    g.tiny(x + 4, y + 4, "EMPTY", 'a');
                }
                if *gap_at.as_ref().map_or(&0, |_| total) > 0 {
                    g.tiny(x + self.w - 22, y, &format!("n={total}"), 'a');
                }
            }
            K::Rack(rows, hidden) => {
                g.round_rect(Rect::new(x, y, self.w, self.h), 'w', 'k');
                let mut cy = y + 2;
                for r in rows {
                    r.draw(g, x + 4, cy, hot);
                    cy += r.h + 1;
                }
                if *hidden > 0 {
                    g.tiny(x + 4, cy, &format!("+{hidden} more"), 'k');
                }
            }
            K::Drawers { rows, hidden } => {
                g.round_rect(Rect::new(x, y, self.w, self.h), 'a', 'K');
                let mut cy = y + 2;
                let kw = rows.iter().map(|r| r.0.w).max().unwrap_or(10);
                for (k, v) in rows {
                    let rh = k.h.max(v.h) + 2;
                    g.round_rect(Rect::new(x + 2, cy, self.w - 4, rh), 'A', 'k');
                    g.round_rect(Rect::new(x + 4, cy + 1, kw + 2, rh - 2), 'W', 'k');
                    k.draw(g, x + 5, cy + 1 + (rh - 2 - k.h) / 2, hot);
                    v.draw(g, x + kw + 10, cy + 1 + (rh - 2 - v.h) / 2, hot);
                    cy += rh + 2;
                }
                if *hidden > 0 {
                    g.tiny(x + 4, cy, &format!("+{hidden} more"), 'K');
                }
            }
            K::Bin { items, hidden } => {
                g.round_rect(Rect::new(x, y + 2, self.w, self.h - 2), 'E', 'N');
                g.hline(x + 1, y + 3, self.w - 2, 'w');
                let mut cx = x + 5;
                for it in items {
                    it.draw(g, cx, y + 5, hot);
                    cx += it.w + 2;
                }
                if *hidden > 0 {
                    g.tiny(cx, y + 8, &format!("+{hidden}"), 'N');
                }
            }
            K::Plates { items, hidden } => {
                let mut cy = y + self.h - 4;
                g.rect(Rect::new(x + 2, cy, self.w - 4, 3), 'a');
                for it in items {
                    cy -= it.h + 3;
                    g.round_rect(Rect::new(x + 2, cy, self.w - 4, it.h + 3), 'A', 'K');
                    it.draw(g, x + (self.w - it.w) / 2, cy + 1, hot);
                }
                if *hidden > 0 {
                    g.tiny(x + 2, y, &format!("+{hidden} below"), 'k');
                }
            }
            K::Belt {
                items,
                hidden,
                double,
            } => {
                g.rect(Rect::new(x, y + self.h - 6, self.w, 5), 'k');
                g.checker(Rect::new(x, y + self.h - 5, self.w, 3), 'K', 'm', 0);
                let mut cx = x + 6;
                for it in items {
                    it.draw(g, cx, y + self.h - 6 - it.h, hot);
                    cx += it.w + 2;
                }
                if *hidden > 0 {
                    g.tiny(cx, y + 2, &format!("+{hidden}"), 'k');
                }
                g.icon(
                    if *double { "left" } else { "right" },
                    x + self.w - 8,
                    y,
                    'K',
                );
            }
            K::Pyramid(levels) => {
                let mut cy = y + 2;
                for row in levels {
                    let rw: i32 = row.iter().map(|x| x.w + 2).sum();
                    let mut cx = x + (self.w - rw) / 2;
                    let rh = row.iter().map(|x| x.h).max().unwrap_or(0);
                    for it in row {
                        it.draw(g, cx, cy, hot);
                        cx += it.w + 2;
                    }
                    cy += rh + 2;
                }
            }
            K::Group(parts) => {
                g.text(x, y + self.h / 2 - 3, "(", 'k');
                let mut cx = x + 4;
                for p in parts {
                    p.draw(g, cx, y + 1 + (self.h - 2 - p.h) / 2, hot);
                    cx += p.w + 3;
                }
                g.text(x + self.w - 5, y + self.h / 2 - 3, ")", 'k');
            }
            K::Tag { label, inner } => {
                let fill = if *label == "none" || *label == "null" {
                    'A'
                } else {
                    'y'
                };
                g.round_rect(Rect::new(x, y, self.w, self.h), fill, 'K');
                match inner {
                    Some(i) => {
                        g.tiny(x + 3, y + 1, label, 'K');
                        i.draw(g, x + 3, y + 7, hot);
                    }
                    None => g.tiny_c(x + self.w / 2, y + 3, label, 'K'),
                }
            }
            K::Chain { vals, cycle, more } => {
                let mut cx = x + 2;
                let mut starts = Vec::new();
                for v in vals {
                    let w = crate_w(v).max(14);
                    g.round_rect(Rect::new(cx, y + 3, w, 14), 'y', 'K');
                    g.px(cx + 3, y + 6, 'K');
                    g.tiny_c(cx + w / 2 + 1, y + 8, v, 'K');
                    starts.push((cx, w));
                    cx += w;
                    g.hline(cx, y + 10, 6, 'K');
                    g.px(cx + 5, y + 9, 'K');
                    g.px(cx + 5, y + 11, 'K');
                    cx += 6;
                }
                if *more {
                    g.text(cx - 4, y + 7, "..", 'K');
                }
                if let Some(c) = cycle {
                    if let Some((sx, sw)) = starts.get(*c) {
                        g.vline(cx - 5, y + 10, 8, 'R');
                        g.hline(sx + sw / 2, y + 18, cx - 5 - (sx + sw / 2), 'R');
                        g.vline(sx + sw / 2, y + 17, 2, 'R');
                    }
                }
            }
            K::Org { nodes, edges, more } => {
                for (a, b) in edges {
                    let (na, nb) = (&nodes[*a], &nodes[*b]);
                    g.line(x + na.0, y + na.1 + 5, x + nb.0, y + nb.1 - 5, 'k');
                }
                for (nx, ny, label) in nodes {
                    let w = (label.chars().count() as i32 * 4 + 6).max(10);
                    g.round_rect(Rect::new(x + nx - w / 2, y + ny - 5, w, 11), 'W', 'K');
                    g.tiny_c(x + nx, y + ny - 2, label, 'K');
                }
                if *more {
                    g.tiny(x + self.w - 8, y + self.h - 6, "..", 'K');
                }
            }
            K::Row(items) => {
                let mut cx = x;
                for it in items {
                    it.draw(g, cx, y, hot);
                    cx += it.w + 2;
                }
            }
        }
    }
}

/// A one-line widget row for the examples on the brief.
pub fn row(items: Vec<W>) -> W {
    let w = items.iter().map(|x| x.w + 2).sum::<i32>();
    let h = items.iter().map(|x| x.h).max().unwrap_or(0);
    W {
        w,
        h,
        k: K::Row(items),
    }
}
