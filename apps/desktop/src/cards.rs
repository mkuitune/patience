use eframe::egui::{self, Color32, Rect, Stroke, StrokeKind};
use graphics2d::{Document, Instance, Paint, Placement, SimplePlacement, View};
use graphics2d_egui::{RasterScale, Renderer};
use patience_core::{Card, Suit};
use playing_card_assets as deck;

pub struct Cards {
    renderer: Renderer,
    deck: deck::Deck,
    viewport: Option<(egui::Vec2, f32)>,
    resize_changed_at: Option<f64>,
    defer_resize: bool,
    raster_size: Option<(f32, f32)>,
    pub error: Option<String>,
}

impl Cards {
    pub fn new() -> Self {
        let deck = deck::Deck::bundled().expect("card resources validated during build");
        Self {
            renderer: Renderer::new(deck.context.resources.clone()),
            deck,
            viewport: None,
            resize_changed_at: None,
            defer_resize: false,
            raster_size: None,
            error: None,
        }
    }

    pub fn begin_frame(&mut self, ctx: &egui::Context) {
        self.renderer.begin_frame();
        let viewport = (ctx.content_rect().size(), ctx.pixels_per_point());
        let now = ctx.input(|i| i.time);
        if self.viewport.is_some_and(|previous| previous != viewport) {
            self.resize_changed_at = Some(now);
        }
        self.viewport = Some(viewport);
        let remaining = self
            .resize_changed_at
            .map(|t| 0.18 - (now - t))
            .unwrap_or(0.0);
        self.defer_resize = remaining > 0.0;
        if self.defer_resize {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(remaining));
        } else {
            self.resize_changed_at = None;
        }
    }

    pub fn paint(&mut self, ui: &egui::Ui, rect: Rect, card: Option<Card>, selected: bool) {
        let view = match card {
            Some(card) => {
                let suit = match card.suit() {
                    Suit::Clubs => deck::Suit::Clubs,
                    Suit::Diamonds => deck::Suit::Diamonds,
                    Suit::Hearts => deck::Suit::Hearts,
                    Suit::Spades => deck::Suit::Spades,
                };
                self.deck
                    .face_view(card.rank(), suit)
                    .expect("valid card rank")
            }
            None => self.deck.back_view(),
        };
        if !self.defer_resize || self.raster_size.is_none() {
            self.raster_size = Some((
                rect.width() * ui.ctx().pixels_per_point(),
                ui.ctx().pixels_per_point(),
            ));
        }
        let (width, pixels_per_point) = self.raster_size.expect("initialized raster size");
        let raster = RasterScale {
            pixels_per_document_unit: width / view.bounds.size().x,
            pixels_per_point,
        };
        if let Err(error) = self
            .renderer
            .paint_with_raster_scale(ui, view, rect, Some(raster))
        {
            self.error = Some(error.to_string());
        }
        if selected {
            ui.painter().rect_stroke(
                rect,
                corner_radius(rect),
                Stroke::new(3.0, Color32::from_rgb(255, 204, 65)),
                StrokeKind::Inside,
            );
        }
    }

    pub fn paint_rank_marker(&mut self, ui: &egui::Ui, rect: Rect, rank: u8, opacity: u8) {
        let asset = self.deck.rank_asset(rank).expect("valid marker rank");
        let bounds = self.deck.context.resources.assets[asset.0].bounds;
        let document = Document {
            instances: vec![Instance {
                asset,
                placement: Placement::Simple(SimplePlacement::IDENTITY),
                paint: Paint::Solid([255, 255, 255, opacity]),
                style: None,
            }],
        };
        let height = rect.width() * 0.16;
        let target = Rect::from_center_size(
            rect.center(),
            egui::vec2(height * bounds.size().x / bounds.size().y, height),
        );
        let (card_width, pixels_per_point) = if self.defer_resize {
            self.raster_size.unwrap_or((
                rect.width() * ui.ctx().pixels_per_point(),
                ui.ctx().pixels_per_point(),
            ))
        } else {
            (
                rect.width() * ui.ctx().pixels_per_point(),
                ui.ctx().pixels_per_point(),
            )
        };
        let raster = RasterScale {
            pixels_per_document_unit: card_width * 0.16 / bounds.size().y,
            pixels_per_point,
        };
        if let Err(error) = self.renderer.paint_with_raster_scale(
            ui,
            View::new(&document, bounds),
            target,
            Some(raster),
        ) {
            self.error = Some(error.to_string());
        }
    }
}

pub fn corner_radius(rect: Rect) -> f32 {
    rect.width() * 0.05
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_resizing_defers_rasterization_until_the_last_change_settles() {
        let ctx = egui::Context::default();
        let mut cards = Cards::new();
        let frame = |cards: &mut Cards, window_width, card_width, time| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(window_width, 800.0),
                    )),
                    time: Some(time),
                    ..Default::default()
                },
                |ctx| {
                    cards.begin_frame(ctx);
                    egui::CentralPanel::default().show(ctx, |ui| {
                        cards.paint(
                            ui,
                            Rect::from_min_size(
                                egui::pos2(20.0, 20.0),
                                egui::vec2(card_width, card_width * 1.4),
                            ),
                            None,
                            false,
                        );
                    });
                },
            )
        };
        let _ = frame(&mut cards, 1000.0, 108.0, 0.0);
        for (window, card, time) in [
            (1200.0, 200.0, 0.02),
            (1300.0, 220.0, 0.08),
            (1300.0, 220.0, 0.20),
        ] {
            let output = frame(&mut cards, window, card, time);
            assert_eq!(cards.raster_size, Some((108.0, 1.0)));
            assert!(
                output.textures_delta.set.is_empty(),
                "resizing must not upload new card textures"
            );
        }
        let output = frame(&mut cards, 1300.0, 220.0, 0.30);
        assert_eq!(cards.raster_size, Some((220.0, 1.0)));
        assert!(!output.textures_delta.set.is_empty());
        assert!(cards.error.is_none());
    }
}
