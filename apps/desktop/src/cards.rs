use eframe::egui::{self, Color32, Rect, Stroke, StrokeKind};
use graphics2d::Document;
use graphics2d_egui::Renderer;
use patience_core::{Card, Suit};
use playing_card_assets as deck;

pub struct Cards {
    renderer: Renderer,
    faces: [Document; 52],
    back: Document,
    pub error: Option<String>,
}

impl Cards {
    pub fn new() -> Self {
        let faces = std::array::from_fn(|index| {
            let suit = [
                deck::Suit::Clubs,
                deck::Suit::Diamonds,
                deck::Suit::Hearts,
                deck::Suit::Spades,
            ][index / 13];
            deck::face((index % 13 + 1) as u8, suit).expect("valid card rank")
        });
        Self {
            renderer: Renderer::new(deck::ASSETS),
            faces,
            back: deck::back(),
            error: None,
        }
    }

    pub fn begin_frame(&mut self) {
        self.renderer.begin_frame();
    }

    pub fn paint(&mut self, ui: &egui::Ui, rect: Rect, card: Option<Card>, selected: bool) {
        let document = match card {
            Some(card) => {
                let suit = match card.suit() {
                    Suit::Clubs => 0,
                    Suit::Diamonds => 1,
                    Suit::Hearts => 2,
                    Suit::Spades => 3,
                };
                &self.faces[suit * 13 + card.rank() as usize - 1]
            }
            None => &self.back,
        };
        if let Err(error) = self.renderer.paint(ui, deck::view(document), rect) {
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
