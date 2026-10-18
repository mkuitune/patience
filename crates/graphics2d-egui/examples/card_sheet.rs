//! Exports every composition for visual review, without a window or GPU.

use graphics2d::{Paint, Resources, Vec2, View, WidthUnit};
use playing_card_assets::{self as deck, Suit};
use resvg::{tiny_skia, usvg};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/card-sheet.png".into());
    let deck = deck::Deck::bundled()?;
    let resources = &deck.context.resources;
    let trees: Vec<_> = resources
        .assets
        .iter()
        .map(|asset| usvg::Tree::from_str(&asset.svg, &usvg::Options::default()))
        .collect::<Result<_, _>>()?;
    let mut sheet = tiny_skia::Pixmap::new(1450, 800).ok_or("sheet allocation failed")?;
    sheet.fill(tiny_skia::Color::from_rgba8(20, 91, 61, 255));
    for (row, suit) in [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades]
        .into_iter()
        .enumerate()
    {
        for rank in 1..=13 {
            let card = rasterize(deck.face_view(rank, suit).unwrap(), &trees, resources)?;
            sheet.draw_pixmap(
                8 + (rank as i32 - 1) * 110,
                8 + row as i32 * 156,
                card.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                tiny_skia::Transform::identity(),
                None,
            );
        }
    }
    let back = rasterize(deck.back_view(), &trees, resources)?;
    sheet.draw_pixmap(
        8,
        632,
        back.as_ref(),
        &tiny_skia::PixmapPaint::default(),
        tiny_skia::Transform::identity(),
        None,
    );
    sheet.save_png(&destination)?;
    println!("{destination}");
    Ok(())
}

fn rasterize(
    view: View<'_>,
    trees: &[usvg::Tree],
    resources: &Resources,
) -> Result<tiny_skia::Pixmap, Box<dyn std::error::Error>> {
    let mut card = tiny_skia::Pixmap::new(104, 146).ok_or("card allocation failed")?;
    let scale = (104.0 / view.bounds.size().x).min(146.0 / view.bounds.size().y);
    let offset = Vec2::new(52.0, 73.0) - view.bounds.center() * scale;
    for instance in &view.document.instances {
        let mut layer = tiny_skia::Pixmap::new(104, 146).ok_or("layer allocation failed")?;
        let matrix = instance.placement.matrix();
        let transform = tiny_skia::Transform::from_row(
            matrix.x_axis.x * scale,
            matrix.x_axis.y * scale,
            matrix.y_axis.x * scale,
            matrix.y_axis.y * scale,
            matrix.z_axis.x * scale + offset.x,
            matrix.z_axis.y * scale + offset.y,
        );
        resvg::render(&trees[instance.asset.0], transform, &mut layer.as_mut());
        if let Paint::Solid([r, g, b, a]) = instance.paint {
            for pixel in layer.data_mut().chunks_exact_mut(4) {
                let alpha = (u16::from(pixel[3]) * u16::from(a) + 127) / 255;
                pixel[0] = ((u16::from(r) * alpha + 127) / 255) as u8;
                pixel[1] = ((u16::from(g) * alpha + 127) / 255) as u8;
                pixel[2] = ((u16::from(b) * alpha + 127) / 255) as u8;
                pixel[3] = alpha as u8;
            }
        }
        if let Some(id) = instance.style {
            let style = resources.styles[id.0];
            let source_width = match style.unit {
                WidthUnit::Source => style.width,
                WidthUnit::LogicalPixels => {
                    style.width / (scale * matrix.x_axis.truncate().length())
                }
            };
            let tree = &trees[instance.asset.0];
            let outline = graphics2d_egui::rasterize_outline(
                tree,
                (tree.size().width() * matrix.x_axis.truncate().length() * scale).ceil() as u32,
                (tree.size().height() * matrix.y_axis.truncate().length() * scale).ceil() as u32,
                style.color,
                source_width,
            )?;
            let outline_transform = tiny_skia::Transform::from_row(
                transform.sx * tree.size().width() / outline.width() as f32,
                transform.ky * tree.size().width() / outline.width() as f32,
                transform.kx * tree.size().height() / outline.height() as f32,
                transform.sy * tree.size().height() / outline.height() as f32,
                transform.tx,
                transform.ty,
            );
            layer.draw_pixmap(
                0,
                0,
                outline.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                outline_transform,
                None,
            );
        }
        card.draw_pixmap(
            0,
            0,
            layer.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            tiny_skia::Transform::identity(),
            None,
        );
    }
    Ok(card)
}
