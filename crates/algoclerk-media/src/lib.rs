// SPDX-License-Identifier: Apache-2.0
//! Pixel art, fonts, a software canvas and a chiptune synthesiser, all engine-agnostic.
//!
//! Everything visual is defined in compact text files under `assets/art/` (see
//! `docs/ART_PIPELINE.md`) and rendered by this crate into an RGBA [`canvas::Canvas`], so the
//! game can be screenshotted and tested without a GPU. The design follows the sibling project
//! Falabichos (Apache-2.0, same owner); the art, the font and the songs here are original.
#![forbid(unsafe_code)]
#![allow(missing_docs)]

pub mod assets;
pub mod bank;
pub mod canvas;
pub mod font;
pub mod sprite;
pub mod synth;

pub use assets::Assets;
pub use bank::{ArtBank, Outfit};
pub use canvas::{Canvas, Rect, Rgba};
pub use font::Font;
pub use sprite::{Palette, Sprite};
