use crate::cards::Cards;
use crate::language::{Key, Language};
use crate::toolbar::{self, Icon};
use eframe::egui::{self, Color32, Rect, Sense, Stroke, StrokeKind};
use patience_core::Suit;

#[derive(Default)]
pub struct HelpDocument {
    cards: Option<Cards>,
}

impl HelpDocument {
    pub fn show(&mut self, ctx: &egui::Context, language: &Language) -> bool {
        let cards = self.cards.get_or_insert_with(Cards::new);
        cards.begin_frame(ctx);
        let modal = egui::Modal::new(egui::Id::new("help_document"))
            .frame(toolbar::panel_frame())
            .backdrop_color(Color32::from_black_alpha(70))
            .show(ctx, |ui| {
                toolbar::settings_style(ui);
                ui.set_width((ctx.content_rect().width() - 72.0).clamp(160.0, 840.0));
                ui.set_height((ctx.content_rect().height() - 120.0).clamp(140.0, 900.0));
                ui.horizontal(|ui| {
                    ui.heading(egui::RichText::new(language.text(Key::Help)).size(23.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if toolbar::button(
                            ui,
                            language.text(Key::Close),
                            Icon::Close,
                            false,
                            None,
                            true,
                        )
                        .on_hover_text(language.text(Key::Close))
                        .clicked()
                        {
                            ui.close();
                        }
                    });
                });
                egui::ScrollArea::vertical()
                    .id_salt("help_document_scroll")
                    .max_height(ui.available_height())
                    .show(ui, |ui| {
                        ui.label(language.text(Key::HelpDeck));
                        ui.add_space(8.0);
                        let columns = ((ui.available_width() + 6.0) / 58.0)
                            .floor()
                            .clamp(3.0, 13.0) as usize;
                        let width = ((ui.available_width() - (columns - 1) as f32 * 6.0)
                            / columns as f32)
                            .min(68.0);
                        let height = width * 1.4;
                        for suit in Suit::ALL {
                            for start in (1..=13).step_by(columns) {
                                let (row, _) = ui.allocate_exact_size(
                                    egui::vec2(ui.available_width(), height + 6.0),
                                    Sense::hover(),
                                );
                                for (column, rank) in
                                    (start..=(start + columns as u8 - 1).min(13)).enumerate()
                                {
                                    let rect = Rect::from_min_size(
                                        row.min + egui::vec2(column as f32 * (width + 6.0), 0.0),
                                        egui::vec2(width, height),
                                    );
                                    cards.paint_rank(ui, rect, rank, suit);
                                }
                            }
                        }
                        section(ui, language, Key::HelpGoalTitle, Key::HelpGoal);
                        ui.add_space(10.0);
                        let stages = [
                            (Key::HelpHomeEmpty, [None; 4]),
                            (Key::HelpHomeBuilding, [Some(3), Some(6), Some(1), Some(4)]),
                            (Key::HelpHomeComplete, [Some(13); 4]),
                        ];
                        if ui.available_width() >= 660.0 {
                            ui.columns(3, |columns| {
                                for (column, (caption, ranks)) in columns.iter_mut().zip(stages) {
                                    home_row(column, cards, language, caption, ranks);
                                }
                            });
                        } else {
                            for (caption, ranks) in stages {
                                home_row(ui, cards, language, caption, ranks);
                                ui.add_space(8.0);
                            }
                        }
                        section(ui, language, Key::HelpPlayTitle, Key::HelpPlay);
                        section(ui, language, Key::HelpStockTitle, Key::HelpStock);
                        ui.label(language.text(Key::Draw));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            let (icon, _) =
                                ui.allocate_exact_size(egui::vec2(24.0, 24.0), Sense::hover());
                            toolbar::paint_icon(
                                ui.painter(),
                                icon.center(),
                                Icon::Teach,
                                ui.visuals().text_color(),
                            );
                            ui.heading(language.text(Key::Teach));
                        });
                        ui.label(language.text(Key::HelpTeach));
                        ui.add_space(12.0);
                        ui.heading(language.text(Key::HelpRulesTitle));
                        for key in [
                            Key::KingRequired,
                            Key::SameColor,
                            Key::WrongRank,
                            Key::FoundationSingleCard,
                            Key::FoundationWrongSuit,
                            Key::FoundationWrongRank,
                        ] {
                            ui.label(format!("• {}", language.text(key)));
                        }
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            let (icon, _) =
                                ui.allocate_exact_size(egui::vec2(24.0, 24.0), Sense::hover());
                            toolbar::paint_icon(
                                ui.painter(),
                                icon.center(),
                                Icon::Undo,
                                ui.visuals().text_color(),
                            );
                            ui.heading(language.text(Key::Undo));
                        });
                        ui.label(language.text(Key::HelpUndo));
                    });
            });
        modal.should_close()
    }
}

fn section(ui: &mut egui::Ui, language: &Language, title: Key, body: Key) {
    ui.add_space(12.0);
    ui.heading(language.text(title));
    ui.label(language.text(body));
}

fn home_row(
    ui: &mut egui::Ui,
    cards: &mut Cards,
    language: &Language,
    caption: Key,
    ranks: [Option<u8>; 4],
) {
    ui.label(language.text(caption));
    let width = ((ui.available_width() - 24.0) / 4.0).clamp(18.0, 62.0);
    let (row, _) =
        ui.allocate_exact_size(egui::vec2(width * 4.0 + 24.0, width * 1.4), Sense::hover());
    for (column, (suit, rank)) in Suit::ALL.into_iter().zip(ranks).enumerate() {
        let target = Rect::from_min_size(
            row.min + egui::vec2(column as f32 * (width + 8.0), 0.0),
            egui::vec2(width, width * 1.4),
        );
        ui.painter().rect(
            target,
            crate::cards::corner_radius(target),
            Color32::from_black_alpha(22),
            Stroke::new(1.0, Color32::from_white_alpha(90)),
            StrokeKind::Inside,
        );
        cards.paint_rank_marker(ui, target, 1, 42);
        if let Some(rank) = rank {
            cards.paint_rank(ui, target, rank, suit);
        }
    }
}
