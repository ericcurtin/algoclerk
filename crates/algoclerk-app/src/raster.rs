// SPDX-License-Identifier: Apache-2.0
//! A small CPU rasteriser for egui meshes: renders the real UI to an image without a window or GPU.
//! Used for the README screenshots and for smoke-testing the UI in CI.

use eframe::egui::{ClippedPrimitive, Color32, TextureId, TexturesDelta, epaint::Primitive};
use std::collections::HashMap;

struct Texture {
    w: usize,
    h: usize,
    pixels: Vec<Color32>,
}

/// Software renderer target.
pub struct Rasterizer {
    pub width: usize,
    pub height: usize,
    /// Premultiplied RGBA in 0..1.
    pixels: Vec<[f32; 4]>,
    textures: HashMap<TextureId, Texture>,
}

fn to_f(c: Color32) -> [f32; 4] {
    [
        f32::from(c.r()) / 255.0,
        f32::from(c.g()) / 255.0,
        f32::from(c.b()) / 255.0,
        f32::from(c.a()) / 255.0,
    ]
}

impl Rasterizer {
    /// A target filled with an opaque background colour.
    pub fn new(width: usize, height: usize, bg: Color32) -> Rasterizer {
        let b = to_f(bg);
        Rasterizer {
            width,
            height,
            pixels: vec![b; width * height],
            textures: HashMap::new(),
        }
    }

    /// Apply texture uploads (the font atlas) from a frame.
    pub fn apply_textures(&mut self, delta: &TexturesDelta) {
        for (id, d) in delta
            .set
            .iter()
            .flat_map(|(id, ds)| ds.iter().map(move |d| (id, d)))
        {
            let eframe::egui::ImageData::Color(img) = &d.image;
            let [w, h] = img.size;
            match d.pos {
                None => {
                    self.textures.insert(
                        *id,
                        Texture {
                            w,
                            h,
                            pixels: img.pixels.clone(),
                        },
                    );
                }
                Some([px, py]) => {
                    if let Some(t) = self.textures.get_mut(id) {
                        for y in 0..h {
                            for x in 0..w {
                                let (tx, ty) = (px + x, py + y);
                                if tx < t.w && ty < t.h {
                                    t.pixels[ty * t.w + tx] = img.pixels[y * w + x];
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn sample(t: &Texture, u: f32, v: f32) -> [f32; 4] {
        // Bilinear with clamping.
        let x = (u * t.w as f32 - 0.5).clamp(0.0, t.w as f32 - 1.0);
        let y = (v * t.h as f32 - 0.5).clamp(0.0, t.h as f32 - 1.0);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(t.w - 1), (y0 + 1).min(t.h - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let p = |xx: usize, yy: usize| to_f(t.pixels[yy * t.w + xx]);
        let (a, b, c, d) = (p(x0, y0), p(x1, y0), p(x0, y1), p(x1, y1));
        let mut out = [0.0; 4];
        for i in 0..4 {
            let top = a[i] * (1.0 - fx) + b[i] * fx;
            let bot = c[i] * (1.0 - fx) + d[i] * fx;
            out[i] = top * (1.0 - fy) + bot * fy;
        }
        out
    }

    /// Paint tessellated primitives. `ppp` is pixels per point.
    pub fn paint(&mut self, prims: &[ClippedPrimitive], ppp: f32) {
        for prim in prims {
            let Primitive::Mesh(mesh) = &prim.primitive else {
                continue;
            };
            let tex = self.textures.get(&mesh.texture_id);
            let clip = prim.clip_rect;
            let (cx0, cy0) = (
                (clip.min.x * ppp).floor().max(0.0) as i64,
                (clip.min.y * ppp).floor().max(0.0) as i64,
            );
            let (cx1, cy1) = (
                ((clip.max.x * ppp).ceil() as i64).min(self.width as i64),
                ((clip.max.y * ppp).ceil() as i64).min(self.height as i64),
            );
            for tri in mesh.indices.chunks_exact(3) {
                let v = [
                    mesh.vertices[tri[0] as usize],
                    mesh.vertices[tri[1] as usize],
                    mesh.vertices[tri[2] as usize],
                ];
                let p = [
                    (v[0].pos.x * ppp, v[0].pos.y * ppp),
                    (v[1].pos.x * ppp, v[1].pos.y * ppp),
                    (v[2].pos.x * ppp, v[2].pos.y * ppp),
                ];
                let area =
                    (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[1].1 - p[0].1) * (p[2].0 - p[0].0);
                if area.abs() < 1e-6 {
                    continue;
                }
                let minx = (p[0].0.min(p[1].0).min(p[2].0).floor() as i64).max(cx0);
                let maxx = (p[0].0.max(p[1].0).max(p[2].0).ceil() as i64).min(cx1);
                let miny = (p[0].1.min(p[1].1).min(p[2].1).floor() as i64).max(cy0);
                let maxy = (p[0].1.max(p[1].1).max(p[2].1).ceil() as i64).min(cy1);
                let col = [to_f(v[0].color), to_f(v[1].color), to_f(v[2].color)];
                let flat_color = v[0].color == v[1].color && v[1].color == v[2].color;
                let flat_uv = v[0].uv == v[1].uv && v[1].uv == v[2].uv;
                // Constant texture sample for solid fills (they all use the same texel).
                let const_tex = match (tex, flat_uv) {
                    (Some(t), true) => Some(Self::sample(t, v[0].uv.x, v[0].uv.y)),
                    _ => None,
                };
                for y in miny..maxy {
                    for x in minx..maxx {
                        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                        let w0 =
                            ((p[1].0 - px) * (p[2].1 - py) - (p[1].1 - py) * (p[2].0 - px)) / area;
                        let w1 =
                            ((p[2].0 - px) * (p[0].1 - py) - (p[2].1 - py) * (p[0].0 - px)) / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                            continue;
                        }
                        let mut c = if flat_color {
                            col[0]
                        } else {
                            let mut c = [0.0f32; 4];
                            for i in 0..4 {
                                c[i] = col[0][i] * w0 + col[1][i] * w1 + col[2][i] * w2;
                            }
                            c
                        };
                        if let Some(s) = const_tex {
                            for i in 0..4 {
                                c[i] *= s[i];
                            }
                        } else if let Some(t) = tex {
                            let u = v[0].uv.x * w0 + v[1].uv.x * w1 + v[2].uv.x * w2;
                            let vv = v[0].uv.y * w0 + v[1].uv.y * w1 + v[2].uv.y * w2;
                            let s = Self::sample(t, u, vv);
                            for i in 0..4 {
                                c[i] *= s[i];
                            }
                        }
                        let dst = &mut self.pixels[y as usize * self.width + x as usize];
                        let inv = 1.0 - c[3];
                        for i in 0..4 {
                            dst[i] = c[i] + dst[i] * inv;
                        }
                    }
                }
            }
        }
    }

    /// The image as straight RGBA bytes.
    pub fn rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 4);
        for p in &self.pixels {
            let a = p[3].max(1e-6);
            for (i, c) in p.iter().enumerate() {
                let v = if i < 3 { (c / a).clamp(0.0, 1.0) } else { *c };
                out.push((v * 255.0 + 0.5) as u8);
            }
        }
        out
    }

    /// Encode as PNG.
    pub fn to_png(&self) -> Result<Vec<u8>, String> {
        let mut buf = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut buf, self.width as u32, self.height as u32);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().map_err(|e| e.to_string())?;
            w.write_image_data(&self.rgba())
                .map_err(|e| e.to_string())?;
        }
        Ok(buf)
    }

    /// Mean luminance and the number of distinct colours (to check that something was drawn).
    pub fn stats(&self) -> (f32, usize) {
        let mut set = std::collections::HashSet::new();
        let mut sum = 0.0;
        for p in &self.pixels {
            sum += 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
            set.insert([
                (p[0] * 255.0) as u8,
                (p[1] * 255.0) as u8,
                (p[2] * 255.0) as u8,
            ]);
        }
        (sum / self.pixels.len().max(1) as f32, set.len())
    }
}
