//! Rasterizes shared SVG components and paints transformed instances through egui.

use egui::{Color32, Rect, TextureHandle};
use graphics2d::{AssetId, Paint, Resources, Vec2, View, WidthUnit};
use resvg::{tiny_skia, usvg};
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("unknown vector asset {0:?}")]
    UnknownAsset(AssetId),
    #[error("invalid SVG asset {name}: {reason}")]
    InvalidSvg { name: String, reason: String },
    #[error("outline requires a single closed SVG path without effects")]
    UnsupportedOutline,
    #[error("unknown outline style")]
    UnknownStyle,
    #[error("vector texture dimensions exceed the renderer limit")]
    TextureTooLarge,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct TextureKey {
    asset: AssetId,
    width: u32,
    height: u32,
    layer: TextureLayer,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum TextureLayer {
    Original,
    Mask,
    Outline { color: [u8; 4], width: u32 },
}

pub struct Renderer {
    resources: Arc<Resources>,
    trees: HashMap<AssetId, usvg::Tree>,
    textures: HashMap<TextureKey, (TextureHandle, u64)>,
    frame: u64,
}

impl Renderer {
    pub fn new(resources: Arc<Resources>) -> Self {
        Self {
            resources,
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
                .resources
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
                layer: if matches!(instance.paint, Paint::Solid(_)) {
                    TextureLayer::Mask
                } else {
                    TextureLayer::Original
                },
            };
            let texture = self.texture(ui.ctx(), key)?;
            let color = match instance.paint {
                Paint::Original => Color32::WHITE,
                Paint::Solid([r, g, b, a]) => Color32::from_rgba_unmultiplied(r, g, b, a),
            };
            paint_quad(&painter, points, texture, color);
            if let Some(id) = instance.style {
                let style = *self
                    .resources
                    .styles
                    .get(id.0)
                    .ok_or(RenderError::UnknownStyle)?;
                let width = match style.unit {
                    WidthUnit::Source => style.width,
                    WidthUnit::LogicalPixels => {
                        style.width / (scale * matrix.x_axis.truncate().length())
                    }
                };
                let key = TextureKey {
                    layer: TextureLayer::Outline {
                        color: style.color,
                        width: width.to_bits(),
                    },
                    ..key
                };
                let texture = self.texture(ui.ctx(), key)?;
                paint_quad(&painter, points, texture, Color32::WHITE);
            }
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
            .resources
            .assets
            .get(key.asset.0)
            .ok_or(RenderError::UnknownAsset(key.asset))?;
        if let std::collections::hash_map::Entry::Vacant(entry) = self.trees.entry(key.asset) {
            let tree =
                usvg::Tree::from_str(&asset.svg, &usvg::Options::default()).map_err(|error| {
                    RenderError::InvalidSvg {
                        name: asset.name.clone(),
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
        if let TextureLayer::Outline { color, width } = key.layer {
            pixmap = rasterize_outline(tree, key.width, key.height, color, f32::from_bits(width))?;
        } else {
            resvg::render(tree, transform, &mut pixmap.as_mut());
        }
        if key.layer == TextureLayer::Mask {
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
        let texture = ctx.load_texture(&asset.name, image, egui::TextureOptions::LINEAR);
        let id = texture.id();
        self.textures.insert(key, (texture, self.frame));
        Ok(id)
    }
}

fn paint_quad(
    painter: &egui::Painter,
    points: [egui::Pos2; 4],
    texture: egui::TextureId,
    color: Color32,
) {
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

/// Strokes the actual source path, clipping a double-width stroke to its interior.
pub fn rasterize_outline(
    tree: &usvg::Tree,
    width: u32,
    height: u32,
    color: [u8; 4],
    source_width: f32,
) -> Result<tiny_skia::Pixmap, RenderError> {
    let [usvg::Node::Path(path)] = tree.root().children() else {
        return Err(RenderError::UnsupportedOutline);
    };
    if !source_width.is_finite() || source_width <= 0.0 {
        return Err(RenderError::UnsupportedOutline);
    }
    let path_data = path
        .data()
        .clone()
        .transform(path.abs_transform())
        .ok_or(RenderError::UnsupportedOutline)?;
    if !matches!(
        path_data.segments().last(),
        Some(tiny_skia::PathSegment::Close)
    ) {
        return Err(RenderError::UnsupportedOutline);
    }
    let transform = tiny_skia::Transform::from_scale(
        width as f32 / tree.size().width(),
        height as f32 / tree.size().height(),
    );
    let mut mask = tiny_skia::Mask::new(width, height).ok_or(RenderError::TextureTooLarge)?;
    mask.fill_path(&path_data, tiny_skia::FillRule::Winding, true, transform);
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or(RenderError::TextureTooLarge)?;
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    let stroke = tiny_skia::Stroke {
        width: source_width * 2.0,
        line_join: tiny_skia::LineJoin::Round,
        ..Default::default()
    };
    pixmap.stroke_path(&path_data, &paint, &stroke, transform, Some(&mask));
    Ok(pixmap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphics2d::{
        Bounds, Document, Handedness, Instance, Placement, SimplePlacement, SvgAsset,
    };

    #[test]
    fn logical_pixel_outlines_keep_their_width_across_card_zoom_and_display_scaling() {
        let tree = usvg::Tree::from_str(&test_resources().assets[0].svg, &usvg::Options::default())
            .unwrap();
        for zoom in [10.0, 20.0] {
            for pixels_per_point in [1.0, 2.0] {
                let size = (10.0 * zoom * pixels_per_point) as u32;
                let image =
                    rasterize_outline(&tree, size, size, [45, 45, 45, 255], 1.25 / zoom).unwrap();
                let row = image.height() / 2;
                let coverage: f32 = (0..size / 2)
                    .map(|x| f32::from(image.pixel(x, row).unwrap().alpha()) / 255.0)
                    .sum();
                assert!(
                    (coverage / pixels_per_point - 1.25).abs() < 0.06,
                    "{zoom}, {pixels_per_point}: {coverage}"
                );
                assert_eq!(image.pixel(size / 2, row).unwrap().alpha(), 0);
                let image = rasterize_outline(&tree, size, size, [45, 45, 45, 255], 0.2).unwrap();
                let coverage: f32 = (0..size / 2)
                    .map(|x| f32::from(image.pixel(x, row).unwrap().alpha()) / 255.0)
                    .sum();
                assert!((coverage / pixels_per_point - 0.2 * zoom).abs() < 0.06);
            }
        }
    }

    #[test]
    fn outlines_follow_curved_source_paths_and_remain_inside_them() {
        let deck = playing_card_assets::Deck::bundled().unwrap();
        let source = &deck.context.resources.assets[0];
        let tree = usvg::Tree::from_str(&source.svg, &usvg::Options::default()).unwrap();
        let image = rasterize_outline(&tree, 150, 210, [45, 45, 45, 255], 6.25).unwrap();
        assert_eq!(image.pixel(0, 0).unwrap().alpha(), 0);
        assert_eq!(image.pixel(75, 105).unwrap().alpha(), 0);
        assert!(image.pixel(75, 0).unwrap().alpha() > 0);
        assert!(image.pixel(0, 105).unwrap().alpha() > 0);
        assert!(image.pixel(2, 2).unwrap().alpha() > 0);
    }

    fn test_resources() -> Arc<Resources> {
        Arc::new(Resources { styles: vec![], assets: vec![SvgAsset {
        name: "square".into(),
        svg: r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><path d="M0 0H10V10H0Z" fill="black"/></svg>"#.into(),
        bounds: Bounds::from_size(10.0, 10.0),
    }] })
    }

    #[test]
    fn view_bounds_frame_nonzero_document_coordinates_and_clip_only_when_requested() {
        let ctx = egui::Context::default();
        let mut renderer = Renderer::new(test_resources());
        let document = Document {
            instances: vec![Instance {
                asset: AssetId(0),
                placement: Placement::Simple(
                    SimplePlacement::new(Vec2::new(20.0, 0.0), Vec2::X, 1.0, Handedness::Positive)
                        .unwrap(),
                ),
                paint: Paint::Original,
                style: None,
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
        let mut renderer = Renderer::new(test_resources());
        let document = Document {
            instances: vec![
                Instance {
                    asset: AssetId(0),
                    placement: Placement::Simple(SimplePlacement::IDENTITY),
                    paint: Paint::Solid([255, 0, 0, 255]),
                    style: None,
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
                    style: None,
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
        let deck = deck::Deck::bundled().unwrap();
        let assets = &deck.context.resources.assets;
        let mut renderer = Renderer::new(deck.context.resources.clone());
        for (asset, source) in assets.iter().enumerate() {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let document = Document {
                        instances: vec![Instance {
                            asset: AssetId(asset),
                            placement: Placement::Simple(SimplePlacement::IDENTITY),
                            paint: Paint::Original,
                            style: None,
                        }],
                    };
                    renderer
                        .paint(
                            ui,
                            View::new(&document, source.bounds),
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
                                .paint(ui, deck.face_view(rank, suit).unwrap(), target)
                                .unwrap();
                        }
                    }
                    renderer.paint(ui, deck.back_view(), target).unwrap();
                });
            });
        }
        assert_eq!(renderer.trees.len(), assets.len());
    }
}
