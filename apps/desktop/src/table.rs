use eframe::egui::{self, Color32, ColorImage, Rect, TextureHandle, TextureOptions};

pub fn load(ctx: &egui::Context) -> TextureHandle {
    let image = tiny_skia::Pixmap::decode_png(include_bytes!(
        "../../../Assets/Textures/DeepForestGreenFeltTexture.png"
    ))
    .expect("bundled felt texture must be a valid PNG");
    ctx.load_texture(
        "green felt",
        ColorImage::from_rgba_premultiplied(
            [image.width() as usize, image.height() as usize],
            image.data(),
        ),
        TextureOptions::LINEAR_MIRRORED_REPEAT,
    )
}

pub fn paint(ctx: &egui::Context, texture: &TextureHandle) {
    let rect = ctx.content_rect();
    let tile_size = texture.size_vec2();
    let uv = Rect::from_min_max(
        egui::pos2(rect.min.x / tile_size.x, rect.min.y / tile_size.y),
        egui::pos2(rect.max.x / tile_size.x, rect.max.y / tile_size.y),
    );
    let painter = ctx.layer_painter(egui::LayerId::background());
    painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 91, 61));
    painter.image(texture.id(), rect, uv, Color32::from_white_alpha(128));
}
