//! Local source HUD atlas/font drawing. All resources remain in original/assets.
use super::*;
use serde_json::Value;
use std::collections::BTreeMap;

pub struct Atlas {
    pub texture: Texture2D,
    frames: BTreeMap<String, Rect>,
}
impl Atlas {
    pub async fn load(images: &Path, name: &str) -> Option<Self> {
        let data: Value =
            serde_json::from_slice(&std::fs::read(images.join(format!("ui/{name}.json"))).ok()?)
                .ok()?;
        let texture = load_texture(&images.join(format!("ui/{name}.png")).to_string_lossy())
            .await
            .ok()?;
        texture.set_filter(FilterMode::Nearest);
        let mut frames = BTreeMap::new();
        for frame in data["textures"][0]["frames"].as_array()? {
            let rect = &frame["frame"];
            frames.insert(
                frame["filename"].as_str()?.to_owned(),
                Rect::new(
                    rect["x"].as_f64()? as f32,
                    rect["y"].as_f64()? as f32,
                    rect["w"].as_f64()? as f32,
                    rect["h"].as_f64()? as f32,
                ),
            );
        }
        Some(Self { texture, frames })
    }
    pub fn frame(&self, key: &str) -> Option<Rect> {
        self.frames.get(key).copied()
    }
    pub fn centered(&self, key: &str, x: f32, y: f32) {
        if let Some(rect) = self.frame(key) {
            draw_texture_ex(
                &self.texture,
                (x - rect.w * 1.5).round(),
                (y - rect.h * 1.5).round(),
                WHITE,
                DrawTextureParams {
                    source: Some(rect),
                    dest_size: Some(vec2(rect.w * 3.0, rect.h * 3.0)),
                    ..Default::default()
                },
            );
        }
    }
}

pub struct Hud {
    pub numbers: Option<Atlas>,
    pub hp: Option<Atlas>,
    pub boss_hp: Option<Atlas>,
    pub level: Option<Texture2D>,
    pub hp_label: Option<Texture2D>,
    pub exp_label: Option<Texture2D>,
    pub font: Option<Font>,
}
impl Hud {
    pub async fn load(images: &Path) -> Self {
        async fn image(path: PathBuf) -> Option<Texture2D> {
            load_texture(&path.to_string_lossy())
                .await
                .ok()
                .inspect(|t| t.set_filter(FilterMode::Nearest))
        }
        let mut font = load_ttf_font(
            &images
                .parent()
                .unwrap_or(images)
                .join("fonts/pokemon-emerald-pro.ttf")
                .to_string_lossy(),
        )
        .await
        .ok();
        if let Some(font) = &mut font {
            font.set_filter(FilterMode::Nearest);
        }
        Self {
            numbers: Atlas::load(images, "numbers").await,
            hp: Atlas::load(images, "overlay_hp").await,
            boss_hp: Atlas::load(images, "overlay_hp_boss").await,
            level: image(images.join("ui/text_images/en/battle_ui/overlay_lv.png")).await,
            hp_label: image(images.join("ui/text_images/en/battle_ui/overlay_hp_label.png")).await,
            exp_label: image(images.join("ui/text_images/en/battle_ui/overlay_exp_label.png"))
                .await,
            font,
        }
    }
    pub fn name(&self, text: &str, x: f32, y: f32) {
        if let Some(font) = &self.font {
            let params = TextParams {
                font: Some(font),
                font_size: 72,
                font_scale: 0.5,
                color: Color::from_rgba(248, 248, 248, 255),
                ..Default::default()
            };
            draw_text_ex(
                text,
                x + 1.75,
                y + 27.0 + 1.75,
                TextParams {
                    color: Color::from_rgba(107, 90, 115, 255),
                    ..params
                },
            );
            draw_text_ex(text, x, y + 27.0, params);
        } else {
            draw_text(text, x, y + 25.0, 25.0, WHITE);
        }
    }
    pub fn window_text(&self, text: &str, x: f32, y: f32) {
        if let Some(font) = &self.font {
            let params = TextParams {
                font: Some(font),
                font_size: 96,
                font_scale: 0.5,
                color: Color::from_rgba(248, 248, 248, 255),
                ..Default::default()
            };
            draw_text_ex(
                text,
                x + 1.5,
                y + 36.0 + 1.5,
                TextParams {
                    color: Color::from_rgba(107, 90, 115, 255),
                    ..params
                },
            );
            draw_text_ex(text, x, y + 36.0, params);
        } else {
            draw_text(text, x, y + 36.0, 48.0, WHITE);
        }
    }
    pub fn image(texture: Option<&Texture2D>, x: f32, y: f32, origin: Vec2) {
        if let Some(texture) = texture {
            draw_texture_ex(
                texture,
                (x - texture.width() * 3.0 * origin.x).round(),
                (y - texture.height() * 3.0 * origin.y).round(),
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(texture.width() * 3.0, texture.height() * 3.0)),
                    ..Default::default()
                },
            );
        }
    }
    pub fn level(&self, player: bool, level: u8, anchor_x: f32, anchor_y: f32) {
        let (lx, ly) = if player {
            (-41.0, -10.0)
        } else {
            (-50.0, -5.0)
        };
        let digits = level.to_string();
        let x = anchor_x + lx * 3.0 - 8.0 * digits.len().saturating_sub(3) as f32 * 3.0;
        let y = anchor_y + ly * 3.0;
        Self::image(self.level.as_ref(), x + 5.5 * 3.0, y, vec2(1.0, 0.5));
        if let Some(numbers) = &self.numbers {
            for (i, c) in digits.chars().enumerate() {
                numbers.centered(&c.to_string(), x + (9.5 + i as f32 * 8.0) * 3.0, y - 1.5);
            }
        }
    }
    pub fn hp_numbers(&self, hp: i32, max: i32, x: f32, y: f32) {
        if let Some(numbers) = &self.numbers {
            for (i, c) in format!("{hp}/{max}").chars().rev().enumerate() {
                numbers.centered(&c.to_string(), x - i as f32 * 24.0, y);
            }
        }
    }
}
