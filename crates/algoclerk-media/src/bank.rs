// SPDX-License-Identifier: Apache-2.0
//! Loads the palettes, the fonts, every sprite and the recolourable character templates from `assets/art/`.
//!
//! Layout:
//! * `art/palette.txt` (warm, the master), `art/palette_cb.txt` (colour-blind safe), `art/palette_hc.txt`
//!   (high contrast): identical letters, different colours;
//! * `art/font.txt` (5x7) and `art/tinyfont.txt` (3x5);
//! * `art/<category>/<id>.px`: sprites (key `category/id`);
//! * `art/<category>/_<name>.px`: *templates* that use role digits (`1` hair, `2` skin, `3` shirt, `4` pants,
//!   `5` shoes, `6` extra) and are rendered per outfit with [`ArtBank::render_template`];
//! * `art/outfits.txt`: named outfit presets, one per line.
//!
//! Adapted from the sibling project Falabichos (Apache-2.0, same owner).

use crate::assets::Assets;
use crate::font::Font;
use crate::sprite::{Grid, Palette, Sprite, parse_grid};
use std::collections::{BTreeMap, HashMap};

/// The shipped palettes: `(id, file)`. The first one is the master.
pub const PALETTES: [(&str, &str); 3] = [
    ("warm", "art/palette.txt"),
    ("colorblind", "art/palette_cb.txt"),
    ("contrast", "art/palette_hc.txt"),
];

/// Character role digits used in `_*.px` templates.
pub const ROLES: [(char, &str); 6] = [
    ('1', "hair"),
    ('2', "skin"),
    ('3', "shirt"),
    ('4', "pants"),
    ('5', "shoes"),
    ('6', "extra"),
];

/// A clerk outfit: a body template plus one palette letter per role.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Outfit {
    /// Body template id (`a`, `b`, ...), i.e. `art/clerk/_body_<id>.px`.
    pub body: String,
    /// Hair letter.
    pub hair: char,
    /// Skin letter.
    pub skin: char,
    /// Shirt letter.
    pub shirt: char,
    /// Trousers letter.
    pub pants: char,
    /// Shoes letter.
    pub shoes: char,
    /// Extra (tie, badge, lanyard) letter.
    pub extra: char,
}

impl Outfit {
    /// The role map used to render templates.
    pub fn roles(&self) -> HashMap<char, char> {
        [
            ('1', self.hair),
            ('2', self.skin),
            ('3', self.shirt),
            ('4', self.pants),
            ('5', self.shoes),
            ('6', self.extra),
        ]
        .into_iter()
        .collect()
    }

    /// Parse `body=a hair=K skin=H ...` (any order; missing roles keep the default).
    ///
    /// # Errors
    /// Unknown role names or malformed pairs.
    pub fn parse(rest: &str) -> Result<Outfit, String> {
        let mut o = Outfit::default();
        for kv in rest.split_whitespace() {
            let (k, v) = kv
                .split_once('=')
                .ok_or(format!("bad `{kv}` (want role=letter)"))?;
            let ch = v.chars().next().ok_or("empty value")?;
            match k {
                "body" => o.body = v.to_string(),
                "hair" => o.hair = ch,
                "skin" => o.skin = ch,
                "shirt" => o.shirt = ch,
                "pants" => o.pants = ch,
                "shoes" => o.shoes = ch,
                "extra" => o.extra = ch,
                other => return Err(format!("unknown role `{other}`")),
            }
        }
        Ok(o)
    }

    /// The compact text form stored in settings (`a:K:H:B:k:K:R`).
    pub fn to_code(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}:{}:{}",
            self.body, self.hair, self.skin, self.shirt, self.pants, self.shoes, self.extra
        )
    }

    /// Parse [`Outfit::to_code`] output; `None` when malformed.
    pub fn from_code(s: &str) -> Option<Outfit> {
        let p: Vec<&str> = s.split(':').collect();
        if p.len() != 7 {
            return None;
        }
        let c = |i: usize| p[i].chars().next().filter(|_| p[i].chars().count() == 1);
        Some(Outfit {
            body: p[0].to_string(),
            hair: c(1)?,
            skin: c(2)?,
            shirt: c(3)?,
            pants: c(4)?,
            shoes: c(5)?,
            extra: c(6)?,
        })
    }
}

impl Default for Outfit {
    fn default() -> Outfit {
        Outfit {
            body: "a".into(),
            hair: 'N',
            skin: 'H',
            shirt: 'B',
            pants: 'k',
            shoes: 'K',
            extra: 'R',
        }
    }
}

/// Everything visual: palettes, fonts and sprites.
#[derive(Debug, Clone)]
pub struct ArtBank {
    /// The palettes in [`PALETTES`] order.
    pub palettes: Vec<Palette>,
    /// The 5x7 font.
    pub font: Font,
    /// The 3x5 font.
    pub tiny: Font,
    /// Named outfit presets from `art/outfits.txt`.
    pub outfits: Vec<(String, Outfit)>,
    sprites: BTreeMap<String, Sprite>,
    templates: BTreeMap<String, Grid>,
    missing: Sprite,
}

impl ArtBank {
    /// Load everything. Returns every problem found, not just the first.
    ///
    /// # Errors
    /// A list of readable messages (file name first).
    pub fn load(assets: &Assets) -> Result<ArtBank, Vec<String>> {
        let mut errs = vec![];
        let mut palettes = vec![];
        for (id, file) in PALETTES {
            match assets.text(file).map(Palette::parse) {
                Some(Ok(p)) => palettes.push(p),
                Some(Err(e)) => errs.push(format!("{file}: {e}")),
                None => errs.push(format!("{file} missing ({id} palette)")),
            }
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        let font = match assets.text("art/font.txt").map(|t| Font::parse(t, 5, 8)) {
            Some(Ok(f)) => f,
            Some(Err(e)) => return Err(vec![format!("art/font.txt: {e}")]),
            None => return Err(vec!["art/font.txt missing".into()]),
        };
        let tiny = match assets
            .text("art/tinyfont.txt")
            .map(|t| Font::parse(t, 3, 5))
        {
            Some(Ok(f)) => f,
            Some(Err(e)) => return Err(vec![format!("art/tinyfont.txt: {e}")]),
            None => return Err(vec!["art/tinyfont.txt missing".into()]),
        };
        let master = &palettes[0];
        let mut sprites = BTreeMap::new();
        let mut templates = BTreeMap::new();
        for path in assets.list("art/") {
            let rel = &path["art/".len()..];
            let parts: Vec<&str> = rel.split('/').collect();
            let [cat, file] = parts.as_slice() else {
                continue;
            };
            let Some(stem) = file.strip_suffix(".px") else {
                continue;
            };
            let text = assets.text(path).unwrap_or("");
            match parse_grid(text) {
                Ok(g) => {
                    if let Some(t) = stem.strip_prefix('_') {
                        templates.insert(format!("{cat}/{t}"), g);
                    } else {
                        match g.render(master, &HashMap::new()) {
                            Ok(s) => {
                                sprites.insert(format!("{cat}/{stem}"), s);
                            }
                            Err(e) => errs.push(format!("{path}: {e}")),
                        }
                    }
                }
                Err(e) => errs.push(format!("{path}: {e}")),
            }
        }
        let mut outfits = vec![];
        if let Some(t) = assets.text("art/outfits.txt") {
            for (n, raw) in t.lines().enumerate() {
                let line = raw.split('#').next().unwrap_or("").trim();
                if line.is_empty() {
                    continue;
                }
                let Some((name, rest)) = line.split_once(':') else {
                    errs.push(format!(
                        "art/outfits.txt line {}: expected `name: role=letter ...`",
                        n + 1
                    ));
                    continue;
                };
                match Outfit::parse(rest) {
                    Ok(o) => outfits.push((name.trim().to_string(), o)),
                    Err(e) => errs.push(format!("art/outfits.txt line {}: {e}", n + 1)),
                }
            }
        }
        if outfits.is_empty() {
            outfits.push(("clerk".into(), Outfit::default()));
        }
        if errs.is_empty() {
            Ok(ArtBank {
                palettes,
                font,
                tiny,
                outfits,
                sprites,
                templates,
                missing: Sprite::flat(16, 16, b'.'),
            })
        } else {
            Err(errs)
        }
    }

    /// A sprite by key (`tiles/carpet`).
    pub fn get(&self, key: &str) -> Option<&Sprite> {
        self.sprites.get(key)
    }

    /// A sprite by key, or an invisible placeholder (never panics).
    pub fn sp(&self, key: &str) -> &Sprite {
        self.sprites.get(key).unwrap_or(&self.missing)
    }

    /// All sprite ids in a category.
    pub fn names(&self, cat: &str) -> Vec<&str> {
        let p = format!("{cat}/");
        self.sprites
            .keys()
            .filter_map(|k| k.strip_prefix(p.as_str()))
            .collect()
    }

    /// Every sprite.
    pub fn all(&self) -> impl Iterator<Item = (&str, &Sprite)> {
        self.sprites.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Template keys such as `clerk/body_a`.
    pub fn template_keys(&self) -> impl Iterator<Item = &str> {
        self.templates.keys().map(String::as_str)
    }

    /// Does a template exist?
    pub fn has_template(&self, key: &str) -> bool {
        self.templates.contains_key(key)
    }

    /// Render a role-digit template with an outfit.
    ///
    /// # Errors
    /// Unknown template or a letter missing from the master palette.
    pub fn render_template(
        &self,
        key: &str,
        roles: &HashMap<char, char>,
    ) -> Result<Sprite, String> {
        let g = self
            .templates
            .get(key)
            .ok_or(format!("no template `{key}`"))?;
        g.render(&self.palettes[0], roles)
    }

    /// The clerk sprite (all animation frames) for an outfit.
    ///
    /// # Errors
    /// See [`ArtBank::render_template`].
    pub fn clerk(&self, outfit: &Outfit) -> Result<Sprite, String> {
        self.render_template(&format!("clerk/body_{}", outfit.body), &outfit.roles())
    }

    /// Ids of the body templates (`a`, `b`, ...).
    pub fn bodies(&self) -> Vec<String> {
        self.templates
            .keys()
            .filter_map(|k| k.strip_prefix("clerk/body_"))
            .map(String::from)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outfit_code_roundtrip() {
        let o = Outfit::default();
        assert_eq!(Outfit::from_code(&o.to_code()), Some(o));
        assert!(Outfit::from_code("a:K").is_none());
        assert!(Outfit::from_code("a:KK:H:B:k:K:R").is_none());
    }

    #[test]
    fn outfit_parse() {
        let o = Outfit::parse("body=b hair=Y shirt=g").unwrap();
        assert_eq!(o.body, "b");
        assert_eq!(o.hair, 'Y');
        assert_eq!(o.shirt, 'g');
        assert!(Outfit::parse("sock=K").is_err());
        assert!(Outfit::parse("hair").is_err());
    }
}
