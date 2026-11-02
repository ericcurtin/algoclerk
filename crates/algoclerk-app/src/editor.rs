// SPDX-License-Identifier: Apache-2.0
//! The program list: drag-and-drop on desktop, tap buttons on touch, with an instruction palette.

use crate::play::PlayState;
use crate::theme::Palette;
use crate::widgets::{Icon, draw_icon, icon_button_sized};
use algoclerk_core::asm::{Line, LineKind};
use algoclerk_core::i18n::Strings;
use algoclerk_core::isa::{Addr, Instr, Opcode, OperandKind};
use eframe::egui::{self, Frame, Margin, RichText, Sense, Stroke, Ui, vec2};

enum RowAction {
    None,
    Delete,
    Up,
    Down,
    Changed,
    Rename(String, String),
    Select,
}

fn labels_of(play: &PlayState) -> Vec<String> {
    play.listing
        .lines
        .iter()
        .filter_map(|l| match &l.kind {
            LineKind::Label(n) => Some(n.clone()),
            _ => None,
        })
        .collect()
}

fn fresh_label(play: &PlayState) -> String {
    let existing = labels_of(play);
    (1..)
        .map(|n| format!("L{n}"))
        .find(|c| !existing.contains(c))
        .unwrap_or_else(|| "L1".to_string())
}

fn sanitize_label(s: &str) -> String {
    let mut out: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(24)
        .collect();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, 'L');
    }
    out
}

/// Insert a new line after the selected one (or at the end).
pub fn insert_line(play: &mut PlayState, kind: NewLine) {
    let at = play.selected.map_or(play.listing.lines.len(), |i| {
        (i + 1).min(play.listing.lines.len())
    });
    let line = match kind {
        NewLine::Op(op) => {
            let target = match op.operand_kind() {
                OperandKind::Label => {
                    let labels = labels_of(play);
                    match labels.first() {
                        Some(l) => l.clone(),
                        None => {
                            let name = fresh_label(play);
                            play.listing.lines.push(Line::label(name.clone()));
                            name
                        }
                    }
                }
                _ => String::new(),
            };
            Line::instr(Instr::with_addr(op, Addr::Direct(0), target))
        }
        NewLine::Label => Line::label(fresh_label(play)),
        NewLine::Comment => Line::comment("note"),
    };
    let at = at.min(play.listing.lines.len());
    play.listing.lines.insert(at, line);
    play.selected = Some(at);
    play.after_edit(true);
}

/// What the palette can add.
#[derive(Clone, Copy)]
pub enum NewLine {
    Op(Opcode),
    Label,
    Comment,
}

/// The instruction palette. Returns the clicked item.
pub fn palette_ui(ui: &mut Ui, play: &PlayState, tr: &Strings, pal: &Palette) -> Option<NewLine> {
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        for op in play.allowed.opcodes() {
            let b = egui::Button::new(RichText::new(op.mnemonic()).monospace().size(13.0))
                .min_size(vec2(0.0, ui.spacing().interact_size.y));
            if ui
                .add(b)
                .on_hover_text(tr.t(&format!("op.{}", op.key())))
                .clicked()
            {
                picked = Some(NewLine::Op(op));
            }
        }
        if ui
            .button(RichText::new(tr.t("editor.label")).color(pal.accent))
            .clicked()
        {
            picked = Some(NewLine::Label);
        }
        if ui
            .button(RichText::new(tr.t("editor.comment")).color(pal.dim))
            .clicked()
        {
            picked = Some(NewLine::Comment);
        }
    });
    picked
}

fn address_ui(
    ui: &mut Ui,
    a: &mut Addr,
    floor_len: usize,
    allow_indirect: bool,
    names: &[String],
    pal: &Palette,
) -> bool {
    let mut changed = false;
    let bs = ui.spacing().interact_size.y.clamp(24.0, 40.0) * 0.85;
    let (mut n, mut indirect) = match *a {
        Addr::Direct(n) => (n, false),
        Addr::Indirect(n) => (n, true),
    };
    if allow_indirect {
        let label = if indirect { "[n]" } else { "n" };
        if ui
            .selectable_label(indirect, RichText::new(label).monospace())
            .on_hover_text("[n]")
            .clicked()
        {
            indirect = !indirect;
            changed = true;
        }
    }
    if icon_button_sized(ui, Icon::Minus, pal, n > 0, "", bs).clicked() {
        n = n.saturating_sub(1);
        changed = true;
    }
    let text = if indirect {
        format!("[{n}]")
    } else {
        n.to_string()
    };
    ui.label(RichText::new(text).monospace().strong());
    if icon_button_sized(ui, Icon::Plus, pal, n + 1 < floor_len.max(1), "", bs).clicked() {
        n += 1;
        changed = true;
    }
    if let Some(name) = names.get(n).filter(|s| !s.is_empty()) {
        ui.label(RichText::new(name).small().color(pal.dim));
    }
    if changed {
        *a = if indirect {
            Addr::Indirect(n)
        } else {
            Addr::Direct(n)
        };
    }
    changed
}

fn set_addr(i: &mut Instr<String>, a: Addr) {
    let label = i.target().cloned().unwrap_or_default();
    *i = Instr::with_addr(i.opcode(), a, label);
}

#[allow(clippy::too_many_arguments)]
fn row_ui(
    ui: &mut Ui,
    idx: usize,
    line: &mut Line,
    play_floor: usize,
    allow_indirect: bool,
    names: &[String],
    labels: &[String],
    executing: bool,
    selected: bool,
    pal: &Palette,
    tr: &Strings,
    count: usize,
) -> (RowAction, egui::Response) {
    let mut action = RowAction::None;
    let bs = ui.spacing().interact_size.y.clamp(24.0, 40.0) * 0.85;
    let bg = if executing {
        pal.highlight
            .gamma_multiply(if pal.dark { 0.35 } else { 0.7 })
    } else if selected {
        pal.accent.gamma_multiply(0.22)
    } else {
        egui::Color32::TRANSPARENT
    };
    let frame = Frame::new()
        .fill(bg)
        .stroke(if executing {
            Stroke::new(2.0, pal.highlight)
        } else {
            Stroke::NONE
        })
        .corner_radius(6)
        .inner_margin(Margin::symmetric(4, 2));
    let resp = frame
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Drag handle.
                let id = ui.make_persistent_id(("line-grip", idx));
                ui.dnd_drag_source(id, idx, |ui| {
                    let (r, _) = ui.allocate_exact_size(
                        vec2(18.0, ui.spacing().interact_size.y),
                        Sense::hover(),
                    );
                    draw_icon(ui.painter(), Icon::Grip, r.shrink2(vec2(2.0, 6.0)), pal.dim);
                });
                match &mut line.kind {
                    LineKind::Label(name) => {
                        let old = name.clone();
                        let mut edit = name.clone();
                        let (dr, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                        ui.painter().add(egui::Shape::convex_polygon(
                            vec![
                                dr.center_top(),
                                dr.right_center(),
                                dr.center_bottom(),
                                dr.left_center(),
                            ],
                            pal.accent,
                            Stroke::NONE,
                        ));
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut edit)
                                .desired_width(110.0)
                                .font(egui::TextStyle::Monospace),
                        );
                        ui.label(RichText::new(":").monospace().color(pal.accent));
                        if r.changed() {
                            let clean = sanitize_label(&edit);
                            action = RowAction::Rename(old, clean);
                        }
                    }
                    LineKind::Instr(instr) => {
                        let op = instr.opcode();
                        ui.label(
                            RichText::new(format!("{:<8}", op.mnemonic()))
                                .monospace()
                                .strong()
                                .color(pal.text),
                        );
                        match op.operand_kind() {
                            OperandKind::None => {}
                            OperandKind::Address => {
                                if let Some(mut a) = instr.addr()
                                    && address_ui(
                                        ui,
                                        &mut a,
                                        play_floor,
                                        allow_indirect,
                                        names,
                                        pal,
                                    )
                                {
                                    set_addr(instr, a);
                                    action = RowAction::Changed;
                                }
                            }
                            OperandKind::Label => {
                                let current = instr.target().cloned().unwrap_or_default();
                                egui::ComboBox::from_id_salt(("jump-target", idx))
                                    .selected_text(
                                        RichText::new(if current.is_empty() {
                                            "?"
                                        } else {
                                            current.as_str()
                                        })
                                        .monospace(),
                                    )
                                    .show_ui(ui, |ui| {
                                        for l in labels {
                                            if ui
                                                .selectable_label(
                                                    *l == current,
                                                    RichText::new(l).monospace(),
                                                )
                                                .clicked()
                                            {
                                                *instr = Instr::with_addr(
                                                    op,
                                                    Addr::Direct(0),
                                                    l.clone(),
                                                );
                                                action = RowAction::Changed;
                                            }
                                        }
                                    });
                            }
                        }
                    }
                    LineKind::Blank => {
                        let mut c = line.comment.clone().unwrap_or_default();
                        ui.label(RichText::new("--").monospace().color(pal.dim));
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut c)
                                .desired_width(f32::INFINITY.min(260.0))
                                .font(egui::TextStyle::Monospace),
                        );
                        if r.changed() {
                            line.comment = Some(c);
                            action = RowAction::Changed;
                        }
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button_sized(ui, Icon::Trash, pal, true, &tr.t("editor.delete"), bs)
                        .clicked()
                    {
                        action = RowAction::Delete;
                    }
                    if icon_button_sized(
                        ui,
                        Icon::Down,
                        pal,
                        idx + 1 < count,
                        &tr.t("editor.move_down"),
                        bs,
                    )
                    .clicked()
                    {
                        action = RowAction::Down;
                    }
                    if icon_button_sized(ui, Icon::Up, pal, idx > 0, &tr.t("editor.move_up"), bs)
                        .clicked()
                    {
                        action = RowAction::Up;
                    }
                });
            });
        })
        .response;
    if matches!(action, RowAction::None) && resp.interact(Sense::click()).clicked() {
        action = RowAction::Select;
    }
    (action, resp)
}

/// The scrollable program list. Returns true if the program changed.
pub fn list_ui(ui: &mut Ui, play: &mut PlayState, tr: &Strings, pal: &Palette) -> bool {
    let mut changed = false;
    let labels = labels_of(play);
    let executing = play.executing_line();
    let count = play.listing.lines.len();
    let floor = play.floor_len;
    let allow_indirect = play.allowed.allows_indirect();
    let names = play.tile_names.clone();
    let selected = play.selected;
    let mut pending: Option<RowAction> = None;
    let mut pending_idx = 0;
    let mut dropped: Option<(usize, usize)> = None;

    if count == 0 {
        ui.add_space(12.0);
        ui.label(RichText::new(tr.t("editor.empty")).color(pal.dim));
    }
    for idx in 0..count {
        ui.push_id(("row", idx), |ui| {
            let (inner, drop) = ui.dnd_drop_zone::<usize, _>(
                Frame::new().inner_margin(Margin::symmetric(0, 1)),
                |ui| {
                    let line = &mut play.listing.lines[idx];
                    row_ui(
                        ui,
                        idx,
                        line,
                        floor,
                        allow_indirect,
                        &names,
                        &labels,
                        executing == Some(idx),
                        selected == Some(idx),
                        pal,
                        tr,
                        count,
                    )
                },
            );
            let (action, _) = inner.inner;
            if !matches!(action, RowAction::None) {
                pending = Some(action);
                pending_idx = idx;
            }
            if let Some(from) = drop {
                dropped = Some((*from, idx));
            }
        });
    }
    if let Some((from, to)) = dropped
        && from != to
        && from < count
        && to < count
    {
        let l = play.listing.lines.remove(from);
        play.listing.lines.insert(to, l);
        play.selected = Some(to);
        changed = true;
    }
    if let Some(a) = pending {
        match a {
            RowAction::Delete => {
                play.listing.lines.remove(pending_idx);
                play.selected = None;
                changed = true;
            }
            RowAction::Up if pending_idx > 0 => {
                play.listing.lines.swap(pending_idx, pending_idx - 1);
                play.selected = Some(pending_idx - 1);
                changed = true;
            }
            RowAction::Down if pending_idx + 1 < count => {
                play.listing.lines.swap(pending_idx, pending_idx + 1);
                play.selected = Some(pending_idx + 1);
                changed = true;
            }
            RowAction::Changed => changed = true,
            RowAction::Rename(old, new) => {
                let taken = play.listing.lines.iter().enumerate().any(|(i, l)| {
                    i != pending_idx && matches!(&l.kind, LineKind::Label(n) if *n == new)
                });
                if !taken && !new.is_empty() {
                    for l in &mut play.listing.lines {
                        match &mut l.kind {
                            LineKind::Label(n) if *n == old => *n = new.clone(),
                            LineKind::Instr(i) if i.target().is_some_and(|t| *t == old) => {
                                let op = i.opcode();
                                *i = Instr::with_addr(op, Addr::Direct(0), new.clone());
                            }
                            _ => {}
                        }
                    }
                    changed = true;
                }
            }
            RowAction::Select => {
                play.selected = if play.selected == Some(pending_idx) {
                    None
                } else {
                    Some(pending_idx)
                }
            }
            _ => {}
        }
    }
    if changed {
        play.after_edit(true);
    }
    changed
}
