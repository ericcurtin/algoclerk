// SPDX-License-Identifier: Apache-2.0
//! The virtual screen as one nearest-neighbour sprite with integer scaling and letterboxing.
//! Adapted from the sibling project Falabichos (Apache-2.0, same owner).

use algoclerk_game::layout::{Display, choose};
use algoclerk_media::Canvas;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

#[derive(Component)]
pub struct FrameSprite;

#[derive(Resource)]
pub struct Frame {
    pub handle: Handle<Image>,
    pub canvas: Canvas,
    pub drawn_tick: u64,
    pub size: (usize, usize),
    pub display: Display,
}

#[derive(Resource, Clone, Copy, Default)]
pub struct SafeInsets(pub (f32, f32, f32, f32));

pub fn make_image(c: &Canvas) -> Image {
    let mut img = Image::new(
        Extent3d {
            width: c.w as u32,
            height: c.h as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        c.px.clone(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    img.sampler = ImageSampler::nearest();
    img
}

pub fn frame_resource(images: &mut Assets<Image>) -> Frame {
    let canvas = Canvas::new(480, 270);
    let handle = images.add(make_image(&canvas));
    Frame {
        handle,
        canvas,
        drawn_tick: u64::MAX,
        size: (480, 270),
        display: choose(960.0, 540.0, (0.0, 0.0, 0.0, 0.0), false),
    }
}

pub fn spawn_display(commands: &mut Commands, frame: &Frame) {
    commands.spawn((Camera2d, Msaa::Off));
    commands.spawn((
        Sprite::from_image(frame.handle.clone()),
        Transform::default(),
        FrameSprite,
    ));
}

/// Position the frame sprite for the current window (integer scale, centred).
pub fn layout_frame(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut sprite: Query<&mut Transform, With<FrameSprite>>,
    frame: Res<Frame>,
) {
    let Ok(window) = windows.single() else { return };
    let sf = window.scale_factor();
    let d = &frame.display;
    if let Ok(mut t) = sprite.single_mut() {
        let k = d.scale / sf;
        t.scale = Vec3::new(k, k, 1.0);
        // the window centre is the world origin; the sprite is centred in the window
        t.translation = Vec3::ZERO;
    }
}

#[cfg(test)]
mod tests {
    use algoclerk_game::layout::choose;

    #[test]
    fn full_hd_is_four_times_landscape() {
        let d = choose(1920.0, 1080.0, (0.0, 0.0, 0.0, 0.0), false);
        assert_eq!(d.scale, 4.0);
        assert!(!d.vp.portrait);
    }
}
