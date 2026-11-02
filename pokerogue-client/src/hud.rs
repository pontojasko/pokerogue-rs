//! Local source HUD atlas/font drawing. All resources remain in original/assets.
use super::*;
use serde_json::Value;
use std::collections::BTreeMap;

pub struct Atlas {
    pub texture: Texture2D,
    frames: BTreeMap<String, AtlasFrame>,
}
#[derive(Clone, Copy)]
struct AtlasFrame {
    rect: Rect,
    source_size: Vec2,
    sprite_offset: Vec2,
}
impl Atlas {
    pub async fn load(images: &Path, name: &str) -> Option<Self> {
        Self::load_at(
            &images.join(format!("ui/{name}.png")),
            &images.join(format!("ui/{name}.json")),
        )
        .await
    }
    pub async fn load_at(image_path: &Path, metadata_path: &Path) -> Option<Self> {
        let bytes = load_file(&metadata_path.to_string_lossy()).await.ok()?;
        let data: Value = serde_json::from_slice(&bytes).ok()?;
        let texture = load_texture(&image_path.to_string_lossy()).await.ok()?;
        texture.set_filter(FilterMode::Nearest);
        let mut frames = BTreeMap::new();
        for frame in data["textures"][0]["frames"].as_array()? {
            let rect = &frame["frame"];
            let source = &frame["sourceSize"];
            let offset = &frame["spriteSourceSize"];
            frames.insert(
                frame["filename"].as_str()?.to_owned(),
                AtlasFrame {
                    rect: Rect::new(
                        rect["x"].as_f64()? as f32,
                        rect["y"].as_f64()? as f32,
                        rect["w"].as_f64()? as f32,
                        rect["h"].as_f64()? as f32,
                    ),
                    source_size: vec2(source["w"].as_f64()? as f32, source["h"].as_f64()? as f32),
                    sprite_offset: vec2(offset["x"].as_f64()? as f32, offset["y"].as_f64()? as f32),
                },
            );
        }
        Some(Self { texture, frames })
    }
    pub fn frame(&self, key: &str) -> Option<Rect> {
        self.frames.get(key).map(|frame| frame.rect)
    }
    pub fn draw_centered(&self, key: &str, x: f32, y: f32, scale: f32) {
        self.draw_centered_rotated(key, x, y, scale, 0.0);
    }
    pub fn draw_centered_rotated(&self, key: &str, x: f32, y: f32, scale: f32, rotation: f32) {
        let Some(frame) = self.frames.get(key) else {
            return;
        };
        let top_left = vec2(x, y) - frame.source_size * (scale / 2.0) + frame.sprite_offset * scale;
        draw_texture_ex(
            &self.texture,
            top_left.x.round(),
            top_left.y.round(),
            WHITE,
            DrawTextureParams {
                source: Some(frame.rect),
                dest_size: Some(frame.rect.size() * scale),
                rotation,
                pivot: (rotation != 0.0).then_some(vec2(x, y)),
                ..Default::default()
            },
        );
    }
    pub fn draw_at(&self, key: &str, x: f32, y: f32, scale: f32) {
        let Some(frame) = self.frames.get(key) else {
            return;
        };
        draw_texture_ex(
            &self.texture,
            (x + frame.sprite_offset.x * scale).round(),
            (y + frame.sprite_offset.y * scale).round(),
            WHITE,
            DrawTextureParams {
                source: Some(frame.rect),
                dest_size: Some(frame.rect.size() * scale),
                ..Default::default()
            },
        );
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
    pub exp_bar: Option<Texture2D>,
    pub font: Option<Font>,
}
impl Hud {
    pub async fn load(images: &Path) -> Self {
        async fn image(path: PathBuf) -> Option<Texture2D> {
            load_texture(&path.to_string_lossy())
                .await
                .ok()
                .inspect(|texture| texture.set_filter(FilterMode::Nearest))
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
            exp_bar: image(images.join("ui/overlay_exp.png")).await,
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
    pub fn gender(&self, name: &str, gender: &str, x: f32, y: f32) {
        let (symbol, color, shadow) = match gender {
            "male" => (
                "♂",
                Color::from_rgba(64, 200, 248, 255),
                Color::from_rgba(0, 96, 144, 255),
            ),
            "female" => (
                "♀",
                Color::from_rgba(248, 152, 144, 255),
                Color::from_rgba(152, 64, 56, 255),
            ),
            _ => return,
        };
        let width = self.font.as_ref().map_or_else(
            || measure_text(name, None, 25, 1.0).width,
            |font| measure_text(name, Some(font), 72, 0.5).width,
        );
        let position_x = x + width;
        if let Some(font) = &self.font {
            let params = TextParams {
                font: Some(font),
                font_size: 72,
                font_scale: 0.5,
                color,
                ..Default::default()
            };
            draw_text_ex(
                symbol,
                position_x + 1.75,
                y + 28.75,
                TextParams {
                    color: shadow,
                    ..params
                },
            );
            draw_text_ex(symbol, position_x, y + 27.0, params);
        } else {
            draw_text(symbol, position_x, y + 25.0, 25.0, color);
        }
    }
    pub fn window_text(&self, text: &str, x: f32, y: f32) {
        self.window_text_color(text, x, y, Color::from_rgba(248, 248, 248, 255));
    }
    pub fn window_text_color(&self, text: &str, x: f32, y: f32, color: Color) {
        if let Some(font) = &self.font {
            let params = TextParams {
                font: Some(font),
                font_size: 96,
                font_scale: 0.5,
                color,
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
    pub fn detail_text(&self, text: &str, x: f32, center_y: f32, right_aligned: bool) {
        if let Some(font) = &self.font {
            let dimensions = measure_text(text, Some(font), 56, 0.5);
            let text_x = if right_aligned {
                x - dimensions.width
            } else {
                x
            };
            let baseline = center_y - dimensions.height / 2.0 + dimensions.offset_y;
            let params = TextParams {
                font: Some(font),
                font_size: 56,
                font_scale: 0.5,
                color: Color::from_rgba(248, 248, 248, 255),
                ..Default::default()
            };
            draw_text_ex(
                text,
                text_x + 1.5,
                baseline + 1.5,
                TextParams {
                    color: Color::from_rgba(107, 90, 115, 255),
                    ..params
                },
            );
            draw_text_ex(text, text_x, baseline, params);
        } else {
            let width = measure_text(text, None, 28, 1.0).width;
            draw_text(
                text,
                if right_aligned { x - width } else { x },
                center_y + 10.0,
                28.0,
                WHITE,
            );
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
        // The source full-game capture places these atlas pixels one output
        // pixel up/left from Macroquad's initial 3× projected anchor.
        let x = anchor_x + lx * 3.0 - 8.0 * digits.len().saturating_sub(3) as f32 * 3.0 - 1.0;
        let y = anchor_y + ly * 3.0 - 1.0;
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
