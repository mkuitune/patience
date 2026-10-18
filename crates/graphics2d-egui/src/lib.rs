//! Rasterizes shared SVG components and paints transformed instances through egui.

use egui::{Color32, Rect, TextureHandle};
use graphics2d::{AssetId, Paint, SvgAsset, Vec2, View};
use resvg::{tiny_skia, usvg};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("unknown vector asset {0:?}")]
    UnknownAsset(AssetId),
    #[error("invalid SVG asset {name}: {reason}")]
    InvalidSvg { name: &'static str, reason: String },
    #[error("vector texture dimensions exceed the renderer limit")]
    TextureTooLarge,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct TextureKey {
    asset: AssetId,
    width: u32,
    height: u32,
    mask: bool,
}

pub struct Renderer {
    assets: &'static [SvgAsset],
    trees: HashMap<AssetId, usvg::Tree>,
    textures: HashMap<TextureKey, (TextureHandle, u64)>,
    frame: u64,
}

impl Renderer {
    pub fn new(assets: &'static [SvgAsset]) -> Self {
        Self {
            assets,
            trees: HashMap::new(),
            textures: HashMap::new(),
            frame: 0,
        }
    }

    /// Call once per frame. Textures unused for two frames are released.
    pub fn begin_frame(&mut self) {
        self.frame += 1;
        self.textures
            .retain(|_, (_, used)| self.frame.saturating_sub(*used) <= 2);
    }

    pub fn cached_textures(&self) -> usize {
        self.textures.len()
    }

    /// Fits the view into the target while preserving aspect ratio; clipping is explicit.
    pub fn paint(
        &mut self,
        ui: &egui::Ui,
        view: View<'_>,
        target: Rect,
    ) -> Result<(), RenderError> {
        if !target.is_finite() || target.width() <= 0.0 || target.height() <= 0.0 {
            return Ok(());
        }
        let scale =
            (target.width() / view.bounds.size().x).min(target.height() / view.bounds.size().y);
        let offset = Vec2::new(target.center().x, target.center().y) - view.bounds.center() * scale;
        let fitted_view = Rect::from_center_size(
            target.center(),
            egui::vec2(view.bounds.size().x * scale, view.bounds.size().y * scale),
        );
        let painter = if view.clip {
            ui.painter().with_clip_rect(fitted_view)
        } else {
            ui.painter().clone()
        };
        for instance in &view.document.instances {
            let asset = self
                .assets
                .get(instance.asset.0)
                .ok_or(RenderError::UnknownAsset(instance.asset))?;
            let matrix = instance.placement.matrix();
            let points = asset.bounds.corners().map(|point| {
                let point = matrix.transform_point2(point) * scale + offset;
                egui::pos2(point.x, point.y)
            });
            if points.iter().any(|point| !point.is_finite()) {
                return Err(RenderError::TextureTooLarge);
            }
            let rect = Rect::from_points(&points);
            if !painter.clip_rect().intersects(rect) {
                continue;
            }
            let pixel_scale = scale * ui.ctx().pixels_per_point();
            let width = (asset.bounds.size().x * matrix.x_axis.truncate().length() * pixel_scale)
                .ceil()
                .max(1.0);
            let height = (asset.bounds.size().y * matrix.y_axis.truncate().length() * pixel_scale)
                .ceil()
                .max(1.0);
            let limit = ui.ctx().input(|input| input.max_texture_side).min(4096) as f32;
            if !width.is_finite() || !height.is_finite() || width > limit || height > limit {
                return Err(RenderError::TextureTooLarge);
            }
            let key = TextureKey {
                asset: instance.asset,
                width: width as u32,
                height: height as u32,
                mask: matches!(instance.paint, Paint::Solid(_)),
            };
            let texture = self.texture(ui.ctx(), key)?;
            let color = match instance.paint {
                Paint::Original => Color32::WHITE,
                Paint::Solid([r, g, b, a]) => Color32::from_rgba_unmultiplied(r, g, b, a),
            };
            let mut mesh = egui::Mesh::with_texture(texture);
            for (pos, uv) in points.into_iter().zip([
                egui::pos2(0.0, 0.0),
                egui::pos2(1.0, 0.0),
                egui::pos2(1.0, 1.0),
                egui::pos2(0.0, 1.0),
            ]) {
                mesh.vertices.push(egui::epaint::Vertex { pos, uv, color });
            }
            mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
            painter.add(egui::Shape::mesh(mesh));
        }
        Ok(())
    }

    fn texture(
        &mut self,
        ctx: &egui::Context,
        key: TextureKey,
    ) -> Result<egui::TextureId, RenderError> {
        if let Some((texture, used)) = self.textures.get_mut(&key) {
            *used = self.frame;
            return Ok(texture.id());
        }
        let asset = self
            .assets
            .get(key.asset.0)
            .ok_or(RenderError::UnknownAsset(key.asset))?;
        if let std::collections::hash_map::Entry::Vacant(entry) = self.trees.entry(key.asset) {
            let tree =
                usvg::Tree::from_str(asset.svg, &usvg::Options::default()).map_err(|error| {
                    RenderError::InvalidSvg {
                        name: asset.name,
                        reason: error.to_string(),
                    }
                })?;
            entry.insert(tree);
        }
        let tree = &self.trees[&key.asset];
        let mut pixmap =
            tiny_skia::Pixmap::new(key.width, key.height).ok_or(RenderError::TextureTooLarge)?;
        let transform = tiny_skia::Transform::from_scale(
            key.width as f32 / tree.size().width(),
            key.height as f32 / tree.size().height(),
        );
        resvg::render(tree, transform, &mut pixmap.as_mut());
        if key.mask {
            for pixel in pixmap.data_mut().chunks_exact_mut(4) {
                pixel[0] = pixel[3];
                pixel[1] = pixel[3];
                pixel[2] = pixel[3];
            }
        }
        let image = egui::ColorImage::from_rgba_premultiplied(
            [key.width as usize, key.height as usize],
            pixmap.data(),
        );
        let texture = ctx.load_texture(asset.name, image, egui::TextureOptions::LINEAR);
        let id = texture.id();
        self.textures.insert(key, (texture, self.frame));
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphics2d::{Bounds, Document, Handedness, Instance, Placement, SimplePlacement};

    static TEST_ASSETS: &[SvgAsset] = &[SvgAsset {
        name: "square",
        svg: r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><path d="M0 0H10V10H0Z" fill="black"/></svg>"#,
        bounds: Bounds::from_size(10.0, 10.0),
    }];

    #[test]
    fn view_bounds_frame_nonzero_document_coordinates_and_clip_only_when_requested() {
        let ctx = egui::Context::default();
        let mut renderer = Renderer::new(TEST_ASSETS);
        let document = Document {
            instances: vec![Instance {
                asset: AssetId(0),
                placement: Placement::Simple(
                    SimplePlacement::new(Vec2::new(20.0, 0.0), Vec2::X, 1.0, Handedness::Positive)
                        .unwrap(),
                ),
                paint: Paint::Original,
            }],
        };
        let fitted = Rect::from_min_max(egui::pos2(70.0, 20.0), egui::pos2(170.0, 120.0));
        for clip in [false, true] {
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let view = View {
                        document: &document,
                        bounds: Bounds::new(Vec2::new(20.0, 0.0), Vec2::new(30.0, 10.0)).unwrap(),
                        clip,
                    };
                    renderer
                        .paint(
                            ui,
                            view,
                            Rect::from_min_max(egui::pos2(20.0, 20.0), egui::pos2(220.0, 120.0)),
                        )
                        .unwrap();
                });
            });
            let shape = output
                .shapes
                .iter()
                .find(|shape| matches!(shape.shape, egui::Shape::Mesh(_)))
                .unwrap();
            let egui::Shape::Mesh(mesh) = &shape.shape else {
                unreachable!()
            };
            assert_eq!(mesh.vertices[0].pos, fitted.min);
            assert_eq!(shape.clip_rect == fitted, clip);
        }
    }

    #[test]
    fn instances_preserve_order_mirror_geometry_and_share_recolored_textures() {
        let ctx = egui::Context::default();
        let mut renderer = Renderer::new(TEST_ASSETS);
        let document = Document {
            instances: vec![
                Instance {
                    asset: AssetId(0),
                    placement: Placement::Simple(SimplePlacement::IDENTITY),
                    paint: Paint::Solid([255, 0, 0, 255]),
                },
                Instance {
                    asset: AssetId(0),
                    placement: Placement::Simple(
                        SimplePlacement::new(
                            Vec2::new(0.0, 10.0),
                            Vec2::X,
                            1.0,
                            Handedness::Negative,
                        )
                        .unwrap(),
                    ),
                    paint: Paint::Solid([0, 0, 255, 255]),
                },
            ],
        };
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                renderer
                    .paint(
                        ui,
                        View::new(&document, Bounds::from_size(10.0, 10.0)),
                        Rect::from_min_max(egui::pos2(20.0, 20.0), egui::pos2(120.0, 120.0)),
                    )
                    .unwrap();
            });
        });
        let meshes: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) => Some(mesh),
                _ => None,
            })
            .collect();
        assert_eq!(meshes.len(), 2);
        assert_eq!(meshes[0].vertices[0].color, Color32::RED);
        assert_eq!(meshes[1].vertices[0].color, Color32::BLUE);
        assert_eq!(meshes[0].vertices[0].pos, egui::pos2(20.0, 20.0));
        assert_eq!(meshes[1].vertices[0].pos, egui::pos2(20.0, 120.0));
        assert_eq!(meshes[0].texture_id, meshes[1].texture_id);
        assert_eq!(renderer.cached_textures(), 1);
        let image = output
            .textures_delta
            .set
            .iter()
            .find(|(id, _)| *id == meshes[0].texture_id)
            .unwrap();
        let egui::ImageData::Color(pixels) = &image.1.image;
        assert!(pixels.pixels.iter().all(|color| *color == Color32::WHITE));
        for _ in 0..3 {
            renderer.begin_frame();
        }
        assert_eq!(renderer.cached_textures(), 0);
    }

    #[test]
    fn all_bundled_components_and_card_faces_render_at_multiple_scales() {
        use playing_card_assets as deck;
        let ctx = egui::Context::default();
        let mut renderer = Renderer::new(deck::ASSETS);
        for asset in 0..deck::ASSETS.len() {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let document = Document {
                        instances: vec![Instance {
                            asset: AssetId(asset),
                            placement: Placement::Simple(SimplePlacement::IDENTITY),
                            paint: Paint::Original,
                        }],
                    };
                    renderer
                        .paint(
                            ui,
                            View::new(&document, deck::ASSETS[asset].bounds),
                            Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(108.0, 151.0)),
                        )
                        .unwrap();
                });
            });
        }
        for width in [108.0, 216.0] {
            renderer.begin_frame();
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let target =
                        Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(width, width * 1.4));
                    for suit in [
                        deck::Suit::Clubs,
                        deck::Suit::Diamonds,
                        deck::Suit::Hearts,
                        deck::Suit::Spades,
                    ] {
                        for rank in 1..=13 {
                            renderer
                                .paint(ui, deck::view(&deck::face(rank, suit).unwrap()), target)
                                .unwrap();
                        }
                    }
                    renderer
                        .paint(ui, deck::view(&deck::back()), target)
                        .unwrap();
                });
            });
        }
        assert_eq!(renderer.trees.len(), deck::ASSETS.len());
    }
}
