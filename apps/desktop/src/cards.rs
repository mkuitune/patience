use eframe::egui::{self, Align2, Color32, FontId, Rect, Stroke, StrokeKind};
use patience_core::{Card, Suit};

const PAPER: Color32 = Color32::WHITE;

pub(crate) fn paint_card(ui: &egui::Ui, rect: Rect, card: Option<Card>, selected: bool) {
    let border = if selected {
        Color32::from_rgb(255, 204, 65)
    } else {
        Color32::from_gray(45)
    };
    ui.painter().rect(
        rect,
        5.0,
        if card.is_some() {
            PAPER
        } else {
            Color32::from_rgb(35, 65, 125)
        },
        Stroke::new(if selected { 3.0 } else { 1.0 }, border),
        StrokeKind::Inside,
    );
    let Some(card) = card else {
        ui.painter().rect_stroke(
            rect.shrink(5.0),
            2.0,
            Stroke::new(1.0, Color32::from_white_alpha(130)),
            StrokeKind::Inside,
        );
        return;
    };
    if card.rank() >= 11 {
        court(ui, rect, card);
        ui.painter().rect_stroke(
            rect,
            5.0,
            Stroke::new(if selected { 3.0 } else { 1.0 }, border),
            StrokeKind::Inside,
        );
        return;
    }
    let ink = if card.suit().is_red() {
        Color32::from_rgb(181, 28, 40)
    } else {
        Color32::from_rgb(23, 27, 34)
    };
    let w = rect.width();
    let h = rect.height();
    let at = |x: f32, y: f32| rect.min + egui::vec2(x * w, y * h);
    for inverted in [false, true] {
        let position = |x, y| {
            if inverted {
                at(1.0 - x, 1.0 - y)
            } else {
                at(x, y)
            }
        };
        text(
            ui,
            position(0.115, 0.085),
            &card.rank_label(),
            w * 0.21,
            ink,
            inverted,
        );
        text(
            ui,
            position(0.115, 0.19),
            card.suit().symbol(),
            w * 0.16,
            ink,
            inverted,
        );
    }
    match card.rank() {
        1 => {
            let size = if card.suit() == Suit::Spades {
                0.60
            } else {
                0.32
            };
            text(
                ui,
                rect.center(),
                card.suit().symbol(),
                w * size,
                ink,
                false,
            );
        }
        2..=10 => {
            for &(x, y) in pip_layout(card.rank()) {
                text(
                    ui,
                    at(0.5 + x * 0.20, 0.5 + y * 0.30),
                    card.suit().symbol(),
                    w * 0.25,
                    ink,
                    y > 0.0,
                );
            }
        }
        _ => {}
    }
}

fn text(ui: &egui::Ui, center: egui::Pos2, value: &str, size: f32, ink: Color32, inverted: bool) {
    let galley = ui
        .painter()
        .layout_no_wrap(value.into(), FontId::proportional(size), ink);
    let position = center - galley.size() * 0.5;
    let angle = if inverted { std::f32::consts::PI } else { 0.0 };
    ui.painter().add(
        egui::epaint::TextShape::new(position, galley, ink)
            .with_angle_and_anchor(angle, Align2::CENTER_CENTER),
    );
}

fn pip_layout(rank: u8) -> &'static [(f32, f32)] {
    match rank {
        2 => &[(0.0, -1.0), (0.0, 1.0)],
        3 => &[(0.0, -1.0), (0.0, 0.0), (0.0, 1.0)],
        4 => &[(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)],
        5 => &[
            (-1.0, -1.0),
            (1.0, -1.0),
            (0.0, 0.0),
            (-1.0, 1.0),
            (1.0, 1.0),
        ],
        6 => &[
            (-1.0, -1.0),
            (1.0, -1.0),
            (-1.0, 0.0),
            (1.0, 0.0),
            (-1.0, 1.0),
            (1.0, 1.0),
        ],
        7 => &[
            (-1.0, -1.0),
            (1.0, -1.0),
            (0.0, -0.5),
            (-1.0, 0.0),
            (1.0, 0.0),
            (-1.0, 1.0),
            (1.0, 1.0),
        ],
        8 => &[
            (-1.0, -1.0),
            (1.0, -1.0),
            (0.0, -0.5),
            (-1.0, 0.0),
            (1.0, 0.0),
            (0.0, 0.5),
            (-1.0, 1.0),
            (1.0, 1.0),
        ],
        9 => &[
            (-1.0, -1.0),
            (1.0, -1.0),
            (-1.0, -1.0 / 3.0),
            (1.0, -1.0 / 3.0),
            (0.0, 0.0),
            (-1.0, 1.0 / 3.0),
            (1.0, 1.0 / 3.0),
            (-1.0, 1.0),
            (1.0, 1.0),
        ],
        10 => &[
            (-1.0, -1.0),
            (1.0, -1.0),
            (0.0, -2.0 / 3.0),
            (-1.0, -1.0 / 3.0),
            (1.0, -1.0 / 3.0),
            (-1.0, 1.0 / 3.0),
            (1.0, 1.0 / 3.0),
            (0.0, 2.0 / 3.0),
            (-1.0, 1.0),
            (1.0, 1.0),
        ],
        _ => &[],
    }
}

fn court_source(card: Card) -> egui::ImageSource<'static> {
    match (card.rank(), card.suit()) {
        (11, Suit::Clubs) => egui::include_image!("../assets/cards/source/jack-clubs.svg"),
        (11, Suit::Diamonds) => egui::include_image!("../assets/cards/source/jack-diamonds.svg"),
        (11, Suit::Hearts) => egui::include_image!("../assets/cards/source/jack-hearts.svg"),
        (11, Suit::Spades) => egui::include_image!("../assets/cards/source/jack-spades.svg"),
        (12, Suit::Clubs) => egui::include_image!("../assets/cards/source/queen-clubs.svg"),
        (12, Suit::Diamonds) => egui::include_image!("../assets/cards/source/queen-diamonds.svg"),
        (12, Suit::Hearts) => egui::include_image!("../assets/cards/source/queen-hearts.svg"),
        (12, Suit::Spades) => egui::include_image!("../assets/cards/source/queen-spades.svg"),
        (13, Suit::Clubs) => egui::include_image!("../assets/cards/source/king-clubs.svg"),
        (13, Suit::Diamonds) => egui::include_image!("../assets/cards/source/king-diamonds.svg"),
        (13, Suit::Hearts) => egui::include_image!("../assets/cards/source/king-hearts.svg"),
        (13, Suit::Spades) => egui::include_image!("../assets/cards/source/king-spades.svg"),
        _ => unreachable!("only court cards have court artwork"),
    }
}

fn court(ui: &egui::Ui, rect: Rect, card: Card) {
    egui::Image::new(court_source(card)).paint_at(ui, rect.shrink(1.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_court_svgs_render_and_cache_at_the_requested_resolution() {
        use patience_core::{Command, Game};

        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        let mut game = Game::deal(42);
        let mut cards: Vec<_> = game
            .tableau()
            .iter()
            .flat_map(|pile| pile.cards().iter().copied())
            .collect();
        while game.stock_len() > 0 {
            game.apply(Command::Draw).unwrap();
        }
        cards.extend_from_slice(game.waste());
        let courts: Vec<_> = cards.into_iter().filter(|card| card.rank() >= 11).collect();
        assert_eq!(courts.len(), 12);
        for card in courts {
            for (width, height) in [(108, 151), (216, 302)] {
                let hint = egui::load::SizeHint::Size {
                    width,
                    height,
                    maintain_aspect_ratio: false,
                };
                let load = || {
                    court_source(card)
                        .load(&ctx, egui::TextureOptions::LINEAR, hint)
                        .unwrap()
                };
                let egui::load::TexturePoll::Ready { texture } = load() else {
                    panic!("bundled SVG should load synchronously");
                };
                let pixel_size = ctx.tex_manager().read().meta(texture.id).unwrap().size;
                assert_eq!(pixel_size, [width as usize, height as usize]);
                let egui::load::TexturePoll::Ready { texture: cached } = load() else {
                    panic!("cached SVG should be ready");
                };
                assert_eq!(texture.id, cached.id);
            }
        }
    }

    #[test]
    fn numbered_cards_have_their_rank_in_distinct_pips() {
        for rank in 2..=10 {
            let pips = pip_layout(rank);
            assert_eq!(pips.len(), rank as usize);
            for (index, &(x, y)) in pips.iter().enumerate() {
                assert!((-1.0..=1.0).contains(&x) && (-1.0..=1.0).contains(&y));
                assert!(!pips[..index].contains(&(x, y)));
            }
        }
    }
}
