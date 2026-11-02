// SPDX-License-Identifier: Apache-2.0
//! The bundled font (JetBrains Mono, SIL OFL 1.1, see assets/fonts/OFL.txt).

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

const FONT: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMonoNL-Regular.ttf");

/// The font definitions: one bundled font for every family.
pub fn definitions() -> FontDefinitions {
    let mut defs = FontDefinitions::empty();
    defs.font_data
        .insert("jbmono".to_string(), Arc::new(FontData::from_static(FONT)));
    defs.families
        .insert(FontFamily::Proportional, vec!["jbmono".to_string()]);
    defs.families
        .insert(FontFamily::Monospace, vec!["jbmono".to_string()]);
    defs
}

/// Install the font into a context.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions());
}
