use eframe::egui::{self, Color32, Rect, Stroke, StrokeKind};
use graphics2d_egui::Renderer;
use patience_core::{Card, Suit};
use playing_card_assets as deck;

pub struct Cards {
    renderer: Renderer,
    deck: deck::Deck,
    pub error: Option<String>,
}

impl Cards {
    pub fn new() -> Self {
        let deck = deck::Deck::bundled().expect("card resources validated during build");
        Self {
            renderer: Renderer::new(deck.context.resources.clone()),
            deck,
            error: None,
        }
    }

    pub fn begin_frame(&mut self) {
        self.renderer.begin_frame();
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
        if let Err(error) = self.renderer.paint(ui, view, rect) {
            self.error = Some(error.to_string());
        }
        if selected {
            ui.painter().rect_stroke(
                rect,
                5.0,
                Stroke::new(3.0, Color32::from_rgb(255, 204, 65)),
                StrokeKind::Inside,
            );
        }
    }
}
