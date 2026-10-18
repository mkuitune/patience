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
    Outline {
        color: [u8; 4],
        width: u32,
        blur: u32,
    },
    Shadow {
        color: [u8; 4],
        blur: u32,
    },
}

impl TextureLayer {
    fn padding(self) -> Result<u32, RenderError> {
        let blur = match self {
            Self::Outline { blur, .. } | Self::Shadow { blur, .. } => f32::from_bits(blur),
            _ => 0.0,
        };
        effect_padding(blur)
    }
}

pub struct Renderer {
    resources: Arc<Resources>,
    trees: HashMap<AssetId, usvg::Tree>,
    textures: HashMap<TextureKey, (TextureHandle, u64)>,
    frame: u64,
}

#[derive(Clone, Copy)]
pub struct RasterScale {
    pub pixels_per_document_unit: f32,
    pub pixels_per_point: f32,
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
        self.paint_with_raster_scale(ui, view, target, None)
    }

    /// Reuses a fixed texture resolution while geometry follows an interactive resize.
    pub fn paint_with_raster_scale(
        &mut self,
        ui: &egui::Ui,
        view: View<'_>,
        target: Rect,
        raster_scale: Option<RasterScale>,
    ) -> Result<(), RenderError> {
        if !target.is_finite() || target.width() <= 0.0 || target.height() <= 0.0 {
            return Ok(());
        }
        let scale =
            (target.width() / view.bounds.size().x).min(target.height() / view.bounds.size().y);
        let raster = raster_scale.unwrap_or(RasterScale {
            pixels_per_document_unit: scale * ui.ctx().pixels_per_point(),
            pixels_per_point: ui.ctx().pixels_per_point(),
        });
        if !raster.pixels_per_document_unit.is_finite()
            || raster.pixels_per_document_unit <= 0.0
            || !raster.pixels_per_point.is_finite()
            || raster.pixels_per_point <= 0.0
        {
            return Err(RenderError::TextureTooLarge);
        }
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
            let style = instance
                .style
                .map(|id| {
                    self.resources
                        .styles
                        .get(id.0)
                        .copied()
                        .ok_or(RenderError::UnknownStyle)
                })
                .transpose()?;
            let paint = match instance.paint {
                Paint::Original => style
                    .and_then(|s| s.fill_color)
                    .map(Paint::Solid)
                    .unwrap_or(Paint::Original),
                paint => paint,
            };
            let matrix = instance.placement.matrix();
            let points = asset.bounds.corners().map(|point| {
                let point = matrix.transform_point2(point) * scale + offset;
                egui::pos2(point.x, point.y)
            });
            if points.iter().any(|point| !point.is_finite()) {
                return Err(RenderError::TextureTooLarge);
            }
            let rect = Rect::from_points(&points);
            let local_to_ui = scale * matrix.x_axis.truncate().length();
            let to_ui = |unit| match unit {
                WidthUnit::Source => local_to_ui,
                WidthUnit::LogicalPixels => 1.0,
            };
            let raster_to_ui = |unit| match unit {
                WidthUnit::Source => {
                    raster.pixels_per_document_unit / raster.pixels_per_point
                        * matrix.x_axis.truncate().length()
                }
                WidthUnit::LogicalPixels => 1.0,
            };
            let effect_extent = style
                .map(|s| {
                    let outline = s.blur * to_ui(s.unit) * 3.0;
                    let shadow = s
                        .shadow
                        .map(|v| (v.blur * 3.0 + v.offset.abs().max_element()) * to_ui(v.unit))
                        .unwrap_or(0.0);
                    outline.max(shadow) + 1.0
                })
                .unwrap_or(0.0);
            if !painter.clip_rect().intersects(rect.expand(effect_extent)) {
                continue;
            }
            let pixel_scale = raster.pixels_per_document_unit;
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
                layer: if matches!(paint, Paint::Solid(_)) {
                    TextureLayer::Mask
                } else {
                    TextureLayer::Original
                },
            };
            if let Some(shadow) = style.and_then(|s| s.shadow) {
                let blur = shadow.blur * raster_to_ui(shadow.unit) * raster.pixels_per_point;
                let shadow_key = TextureKey {
                    layer: TextureLayer::Shadow {
                        color: shadow.color,
                        blur: blur.to_bits(),
                    },
                    ..key
                };
                self.check_texture_limit(ui.ctx(), shadow_key)?;
                let texture = self.texture(ui.ctx(), shadow_key)?;
                let points = expanded_points(
                    points,
                    key.width,
                    key.height,
                    shadow_key.layer.padding()?,
                    shadow.offset * to_ui(shadow.unit),
                );
                paint_quad(&painter, points, texture, Color32::WHITE);
            }
            if !matches!(paint, Paint::Solid([_, _, _, 0])) {
                let texture = self.texture(ui.ctx(), key)?;
                let color = match paint {
                    Paint::Original => Color32::WHITE,
                    Paint::Solid([r, g, b, a]) => Color32::from_rgba_unmultiplied(r, g, b, a),
                };
                paint_quad(&painter, points, texture, color);
            }
            if let Some(style) = style {
                let width = match style.unit {
                    WidthUnit::Source => style.width,
                    WidthUnit::LogicalPixels => style.width / raster_to_ui(WidthUnit::Source),
                };
                let key = TextureKey {
                    layer: TextureLayer::Outline {
                        color: style.color,
                        width: width.to_bits(),
                        blur: (style.blur * raster_to_ui(style.unit) * raster.pixels_per_point)
                            .to_bits(),
                    },
                    ..key
                };
                self.check_texture_limit(ui.ctx(), key)?;
                let texture = self.texture(ui.ctx(), key)?;
                let points = expanded_points(
                    points,
                    key.width,
                    key.height,
                    key.layer.padding()?,
                    Vec2::ZERO,
                );
                paint_quad(&painter, points, texture, Color32::WHITE);
            }
        }
        Ok(())
    }

    fn check_texture_limit(&self, ctx: &egui::Context, key: TextureKey) -> Result<(), RenderError> {
        let pad = key.layer.padding()?;
        let limit = ctx.input(|i| i.max_texture_side).min(4096);
        if key.width as usize + 2 * pad as usize > limit
            || key.height as usize + 2 * pad as usize > limit
        {
            return Err(RenderError::TextureTooLarge);
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
        match key.layer {
            TextureLayer::Outline { color, width, blur } => {
                pixmap = rasterize_soft_outline(
                    tree,
                    key.width,
                    key.height,
                    color,
                    f32::from_bits(width),
                    f32::from_bits(blur),
                )?;
            }
            TextureLayer::Shadow { color, blur } => {
                pixmap =
                    rasterize_shadow(tree, key.width, key.height, color, f32::from_bits(blur))?;
            }
            _ => resvg::render(tree, transform, &mut pixmap.as_mut()),
        }
        if key.layer == TextureLayer::Mask {
            for pixel in pixmap.data_mut().chunks_exact_mut(4) {
                pixel[0] = pixel[3];
                pixel[1] = pixel[3];
                pixel[2] = pixel[3];
            }
        }
        let image = egui::ColorImage::from_rgba_premultiplied(
            [pixmap.width() as usize, pixmap.height() as usize],
            pixmap.data(),
        );
        let texture = ctx.load_texture(&asset.name, image, egui::TextureOptions::LINEAR);
        let id = texture.id();
        self.textures.insert(key, (texture, self.frame));
        Ok(id)
    }
}

fn expanded_points(
    points: [egui::Pos2; 4],
    width: u32,
    height: u32,
    padding: u32,
    offset: Vec2,
) -> [egui::Pos2; 4] {
    let x = (points[1] - points[0]) * (padding as f32 / width as f32);
    let y = (points[3] - points[0]) * (padding as f32 / height as f32);
    let offset = egui::vec2(offset.x, offset.y);
    [
        points[0] - x - y + offset,
        points[1] + x - y + offset,
        points[2] + x + y + offset,
        points[3] - x + y + offset,
    ]
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
    let path_data = outline_path(tree)?;
    if !source_width.is_finite() || source_width <= 0.0 {
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

fn outline_path(tree: &usvg::Tree) -> Result<tiny_skia::Path, RenderError> {
    let mut group = tree.root();
    let path = loop {
        if group.opacity().get() != 1.0
            || group.blend_mode() != usvg::BlendMode::Normal
            || group.isolate()
            || group.clip_path().is_some()
            || group.mask().is_some()
            || !group.filters().is_empty()
        {
            return Err(RenderError::UnsupportedOutline);
        }
        match group.children() {
            [usvg::Node::Group(inner)] => group = inner,
            [usvg::Node::Path(path)] => break path,
            _ => return Err(RenderError::UnsupportedOutline),
        }
    };

    let data = path
        .data()
        .clone()
        .transform(path.abs_transform())
        .ok_or(RenderError::UnsupportedOutline)?;
    if !matches!(data.segments().last(), Some(tiny_skia::PathSegment::Close)) {
        return Err(RenderError::UnsupportedOutline);
    }
    Ok(data)
}

pub fn effect_padding(blur: f32) -> Result<u32, RenderError> {
    if !blur.is_finite() || !(0.0..=1024.0).contains(&blur) {
        return Err(RenderError::TextureTooLarge);
    }
    Ok(if blur == 0.0 {
        0
    } else {
        (blur * 3.0).ceil() as u32 + 1
    })
}

/// Rasterizes a padded, reusable silhouette using resvg's Gaussian filter.
pub fn rasterize_shadow(
    tree: &usvg::Tree,
    width: u32,
    height: u32,
    color: [u8; 4],
    blur: f32,
) -> Result<tiny_skia::Pixmap, RenderError> {
    rasterize_effect(tree, width, height, color, blur, None)
}

/// Blurs the inward stroke after clipping, allowing a soft outer edge.
pub fn rasterize_soft_outline(
    tree: &usvg::Tree,
    width: u32,
    height: u32,
    color: [u8; 4],
    source_width: f32,
    blur: f32,
) -> Result<tiny_skia::Pixmap, RenderError> {
    if blur == 0.0 {
        return rasterize_outline(tree, width, height, color, source_width);
    }
    rasterize_effect(tree, width, height, color, blur, Some(source_width))
}

fn rasterize_effect(
    tree: &usvg::Tree,
    width: u32,
    height: u32,
    color: [u8; 4],
    blur: f32,
    stroke: Option<f32>,
) -> Result<tiny_skia::Pixmap, RenderError> {
    use std::fmt::Write;
    let path = outline_path(tree)?;
    let mut data = String::new();
    for segment in path.segments() {
        match segment {
            tiny_skia::PathSegment::MoveTo(p) => write!(data, "M{} {}", p.x, p.y),
            tiny_skia::PathSegment::LineTo(p) => write!(data, "L{} {}", p.x, p.y),
            tiny_skia::PathSegment::QuadTo(a, b) => {
                write!(data, "Q{} {} {} {}", a.x, a.y, b.x, b.y)
            }
            tiny_skia::PathSegment::CubicTo(a, b, c) => {
                write!(data, "C{} {} {} {} {} {}", a.x, a.y, b.x, b.y, c.x, c.y)
            }
            tiny_skia::PathSegment::Close => write!(data, "Z"),
        }
        .expect("writing to a string");
    }
    let pad = effect_padding(blur)?;
    let w = width
        .checked_add(pad * 2)
        .filter(|v| *v > 0 && *v <= 4096)
        .ok_or(RenderError::TextureTooLarge)?;
    let h = height
        .checked_add(pad * 2)
        .filter(|v| *v > 0 && *v <= 4096)
        .ok_or(RenderError::TextureTooLarge)?;
    let ink = format!("rgb({},{},{})", color[0], color[1], color[2]);
    let body = match stroke {
        Some(value) if value.is_finite() && value > 0.0 => format!(
            r##"<g clip-path="url(#inside)"><use href="#shape" fill="none" stroke="{ink}" stroke-opacity="{}" stroke-width="{}" stroke-linejoin="round"/></g>"##,
            color[3] as f32 / 255.0,
            value * 2.0
        ),
        Some(_) => return Err(RenderError::UnsupportedOutline),
        None => format!(
            r##"<use href="#shape" fill="{ink}" fill-opacity="{}"/>"##,
            color[3] as f32 / 255.0
        ),
    };
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}"><defs><path id="shape" d="{data}"/><clipPath id="inside"><use href="#shape"/></clipPath><filter id="soft" filterUnits="userSpaceOnUse" x="0" y="0" width="{w}" height="{h}" color-interpolation-filters="sRGB"><feGaussianBlur stdDeviation="{blur}"/></filter></defs><g filter="url(#soft)"><g transform="translate({pad} {pad}) scale({} {})">{body}</g></g></svg>"##,
        width as f32 / tree.size().width(),
        height as f32 / tree.size().height()
    );
    let effect = usvg::Tree::from_str(&svg, &usvg::Options::default()).map_err(|e| {
        RenderError::InvalidSvg {
            name: "generated vector effect".into(),
            reason: e.to_string(),
        }
    })?;
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or(RenderError::TextureTooLarge)?;
    resvg::render(
        &effect,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphics2d::{
        Bounds, Document, Handedness, Instance, Placement, SimplePlacement, SvgAsset,
    };

    #[test]
    fn cards_share_padded_shadow_textures_across_faces_and_frames() {
        let deck = playing_card_assets::Deck::bundled().unwrap();
        let ctx = egui::Context::default();
        let mut renderer = Renderer::new(deck.context.resources.clone());
        let render = |renderer: &mut Renderer| {
            ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    for rank in [1, 8, 13] {
                        renderer
                            .paint(
                                ui,
                                deck.face_view(rank, playing_card_assets::Suit::Hearts)
                                    .unwrap(),
                                Rect::from_min_size(
                                    egui::pos2(30.0, 30.0),
                                    egui::vec2(150.0, 210.0),
                                ),
                            )
                            .unwrap();
                    }
                });
            })
        };
        let first = render(&mut renderer);
        let shadows: Vec<_> = renderer
            .textures
            .iter()
            .filter(|(k, _)| matches!(k.layer, TextureLayer::Shadow { .. }))
            .collect();
        assert_eq!(shadows.len(), 1);
        let (key, (texture, _)) = shadows[0];
        let image = first
            .textures_delta
            .set
            .iter()
            .find(|(id, _)| *id == texture.id())
            .unwrap();
        let egui::ImageData::Color(pixels) = &image.1.image;
        let pad = key.layer.padding().unwrap() as usize;
        assert_eq!(
            pixels.size,
            [key.width as usize + 2 * pad, key.height as usize + 2 * pad]
        );
        assert!(
            pixels[(pad - 1, pad + key.height as usize / 2)].a() > 0,
            "shadow must extend beyond the card"
        );
        assert!(
            pixels[(0, 0)].a() < 2,
            "padding must contain the blur falloff"
        );
        renderer.begin_frame();
        let second = render(&mut renderer);
        assert!(
            second.textures_delta.set.is_empty(),
            "unchanged cards must reuse their textures"
        );
        let resized = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                renderer
                    .paint_with_raster_scale(
                        ui,
                        deck.face_view(8, playing_card_assets::Suit::Hearts)
                            .unwrap(),
                        Rect::from_min_size(egui::pos2(30.0, 30.0), egui::vec2(225.0, 315.0)),
                        Some(RasterScale {
                            pixels_per_document_unit: 150.0 / 750.0,
                            pixels_per_point: 1.0,
                        }),
                    )
                    .unwrap();
            });
        });
        assert!(
            resized.textures_delta.set.is_empty(),
            "interactive resizing must stretch cached textures without rerasterizing"
        );
        let settled = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                renderer
                    .paint(
                        ui,
                        deck.face_view(8, playing_card_assets::Suit::Hearts)
                            .unwrap(),
                        Rect::from_min_size(egui::pos2(30.0, 30.0), egui::vec2(225.0, 315.0)),
                    )
                    .unwrap();
            });
        });
        assert!(
            !settled.textures_delta.set.is_empty(),
            "settled resizing must render at the new resolution"
        );
    }

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
