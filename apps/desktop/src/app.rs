use crate::platform::Platform;
use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind};
use patience_core::{Card, Command, Destination, DrawMode, Engine, Game, Source, Suit};

const TABLE: Color32 = Color32::from_rgb(20, 91, 61);

pub struct PatienceApp {
    engine: Engine,
    selected: Option<Source>,
    drag: Option<Drag>,
    returning: Option<ReturnAnimation>,
    next_draw_mode: DrawMode,
    message: String,
    load_error: Option<String>,
    platform: Platform,
}

struct Drag {
    source: Source,
    offset: egui::Vec2,
}

struct ReturnAnimation {
    source: Source,
    cards: Vec<Card>,
    from: egui::Pos2,
    started_at: f64,
}

const RETURN_SECONDS: f64 = 0.18;

fn belongs_to(card: Source, moving: Source) -> bool {
    match (card, moving) {
        (
            Source::Tableau { column, index },
            Source::Tableau {
                column: from,
                index: first,
            },
        ) => column == from && index >= first,
        _ => card == moving,
    }
}

#[derive(Clone, Copy)]
enum Click {
    Stock,
    Card {
        source: Source,
        destination: Option<Destination>,
        double: bool,
    },
    Empty(Destination),
}

impl PatienceApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        cc.egui_ctx.style_mut(|style| {
            style.spacing.button_padding = egui::vec2(14.0, 9.0);
            style
                .text_styles
                .insert(egui::TextStyle::Body, FontId::proportional(16.0));
            style
                .text_styles
                .insert(egui::TextStyle::Button, FontId::proportional(16.0));
        });
        let platform = Platform::new(&cc.egui_ctx);
        let fragment = platform.fragment();
        let restored = if fragment.is_empty() {
            Ok(Game::deal(rand::random()))
        } else {
            Game::decode(&fragment)
        };
        let (game, load_error) = match restored {
            Ok(game) => (game, None),
            Err(error) => (Game::deal(rand::random()), Some(error.to_string())),
        };
        let mut app = Self {
            engine: Engine::new(game),
            selected: None,
            drag: None,
            returning: None,
            next_draw_mode: DrawMode::Three,
            message: String::new(),
            load_error,
            platform,
        };
        if app.load_error.is_none() {
            app.save_url();
        }
        app
    }

    fn save_url(&mut self) {
        if let Err(error) = self.platform.save(&self.engine.game().encode()) {
            self.message = format!("Could not update the bookmark URL: {error}");
        }
    }

    fn execute(&mut self, command: Command) -> bool {
        match self.engine.execute(command) {
            Ok(_) => {
                self.selected = None;
                self.drag = None;
                self.returning = None;
                self.message.clear();
                self.save_url();
                true
            }
            Err(error) => {
                self.message = error.to_string();
                false
            }
        }
    }

    fn clicked(&mut self, click: Click) {
        match click {
            Click::Stock => {
                self.execute(Command::Draw);
            }
            Click::Empty(to) => {
                if let Some(from) = self.selected {
                    self.execute(Command::Move { from, to });
                }
            }
            Click::Card {
                source,
                destination,
                double,
            } => {
                if double {
                    if let Some(to) = (0..4)
                        .map(Destination::Foundation)
                        .find(|&to| self.engine.game().can_move(source, to))
                    {
                        self.execute(Command::Move { from: source, to });
                    } else {
                        self.message = "That card cannot move to a foundation yet.".into();
                    }
                    return;
                }
                if self.selected == Some(source) {
                    self.selected = None;
                    return;
                }
                if let (Some(from), Some(to)) = (self.selected, destination) {
                    self.execute(Command::Move { from, to });
                    return;
                }
                self.selected = Some(source);
                self.message.clear();
            }
        }
    }

    fn board(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width().max(650.0);
        let w = ((width - 84.0) / 7.0).clamp(74.0, 108.0);
        let h = w * 1.4;
        let front_step = w * 0.36;
        let back_step = w * 0.18;
        let game = self.engine.game();
        let max_height = game
            .tableau()
            .iter()
            .map(|pile| {
                pile.hidden() as f32 * back_step
                    + pile.cards().len().saturating_sub(pile.hidden() + 1) as f32 * front_step
                    + h
            })
            .fold(h, f32::max);
        let (board, _) =
            ui.allocate_exact_size(egui::vec2(width, h + 58.0 + max_height), Sense::hover());
        let origin = board.min + egui::vec2((width - (w * 7.0 + 84.0)) * 0.5, 8.0);
        let at = |column: usize, y: f32| {
            Rect::from_min_size(
                origin + egui::vec2(column as f32 * (w + 14.0), y),
                egui::vec2(w, h),
            )
        };
        let mut click = None;
        let mut sources = Vec::new();
        let mut targets = Vec::new();
        let mut cards_to_paint = Vec::new();

        let stock = at(0, 0.0);
        slot(ui, stock, if game.stock_len() == 0 { "↻" } else { "" });
        if game.stock_len() > 0 {
            paint_card(ui, stock, None, false);
        }
        if ui
            .interact(stock, ui.id().with("stock"), Sense::click())
            .on_hover_text(format!(
                "Draw {} card(s); recycle waste when empty.",
                game.draw_mode().count()
            ))
            .clicked()
        {
            click = Some(Click::Stock);
        }
        let ink = Color32::from_white_alpha(200);
        let count = ui.painter().layout_no_wrap(
            game.stock_len().to_string(),
            FontId::proportional(13.0),
            ink,
        );
        let center = stock.center_bottom() + egui::vec2(0.0, 14.0);
        let left = center.x - (20.0 + count.size().x) * 0.5;
        for layer in (0..3).rev() {
            let offset = egui::vec2(layer as f32 * 2.0, -(layer as f32) * 1.5);
            let outline = Rect::from_min_size(
                egui::pos2(left, center.y - 5.0) + offset,
                egui::vec2(9.0, 12.0),
            );
            ui.painter().rect(
                outline,
                1.5,
                TABLE,
                Stroke::new(1.0, ink),
                StrokeKind::Inside,
            );
        }
        ui.painter().galley(
            egui::pos2(left + 20.0, center.y - count.size().y * 0.5),
            count,
            ink,
        );

        let waste = at(1, 0.0);
        slot(ui, waste, "Waste");
        let visible = game.waste().len().min(game.draw_mode().count());
        for (index, &value) in game
            .waste()
            .iter()
            .skip(game.waste().len() - visible)
            .enumerate()
        {
            let target = waste.translate(egui::vec2(index as f32 * w * 0.48, 0.0));
            let top = index + 1 == visible;
            cards_to_paint.push((
                target,
                Some(value),
                top && self.selected == Some(Source::Waste),
                top.then_some(Source::Waste),
            ));
            if !top {
                continue;
            }
            let response = ui.interact(target, ui.id().with("waste"), Sense::click_and_drag());
            if response.clicked() || response.double_clicked() {
                click = Some(Click::Card {
                    source: Source::Waste,
                    destination: None,
                    double: response.double_clicked(),
                });
            }
            sources.push((Source::Waste, response, target));
        }
        for (suit, kind) in Suit::ALL.into_iter().enumerate() {
            let target = at(3 + suit, 0.0);
            slot(ui, target, kind.symbol());
            if let Some(top) = game.foundation(suit) {
                cards_to_paint.push((
                    target,
                    Some(top),
                    self.selected == Some(Source::Foundation(suit)),
                    Some(Source::Foundation(suit)),
                ));
            }
            let response = ui.interact(
                target,
                ui.id().with(("foundation", suit)),
                Sense::click_and_drag(),
            );
            if response.clicked() {
                click = Some(if game.foundation(suit).is_some() {
                    Click::Card {
                        source: Source::Foundation(suit),
                        destination: Some(Destination::Foundation(suit)),
                        double: false,
                    }
                } else {
                    Click::Empty(Destination::Foundation(suit))
                });
            }
            if game.foundation(suit).is_some() {
                sources.push((Source::Foundation(suit), response.clone(), target));
            }
            targets.push((Destination::Foundation(suit), response, target));
        }
        for (column, pile) in game.tableau().iter().enumerate() {
            let empty = at(column, h + 42.0);
            slot(ui, empty, "K");
            if pile.cards().is_empty() {
                let response = ui.interact(empty, ui.id().with(("empty", column)), Sense::click());
                if response.clicked() {
                    click = Some(Click::Empty(Destination::Tableau(column)));
                }
                targets.push((Destination::Tableau(column), response, empty));
            }
            let mut y = h + 42.0;
            for (index, &value) in pile.cards().iter().enumerate() {
                let hidden = index < pile.hidden();
                let source = Source::Tableau { column, index };
                let selected = matches!(self.selected, Some(Source::Tableau { column: c, index: i }) if c == column && index >= i);
                let target = at(column, y);
                cards_to_paint.push((
                    target,
                    (!hidden).then_some(value),
                    selected,
                    (!hidden).then_some(source),
                ));
                let step = if hidden { back_step } else { front_step };
                if !hidden {
                    // Only the exposed strip belongs to a covered card's hit region.
                    let hit = if index + 1 == pile.cards().len() {
                        target
                    } else {
                        Rect::from_min_size(target.min, egui::vec2(w, step))
                    };
                    let response = ui.interact(
                        hit,
                        ui.id().with(("card", column, index)),
                        Sense::click_and_drag(),
                    );
                    if response.clicked() || response.double_clicked() {
                        click = Some(Click::Card {
                            source,
                            destination: Some(Destination::Tableau(column)),
                            double: response.double_clicked(),
                        });
                    }
                    targets.push((Destination::Tableau(column), response.clone(), target));
                    sources.push((source, response, target));
                }
                y += step;
            }
        }
        for (source, response, rect) in &sources {
            if response.drag_started() {
                let press = ui
                    .input(|input| input.pointer.press_origin())
                    .unwrap_or(rect.min);
                self.drag = Some(Drag {
                    source: *source,
                    offset: press - rect.min,
                });
                self.returning = None;
                self.selected = Some(*source);
                self.message.clear();
            }
        }
        let return_home = self.returning.as_ref().and_then(|animation| {
            sources
                .iter()
                .find(|(source, _, _)| *source == animation.source)
                .map(|(_, _, rect)| *rect)
        });
        if let Some(animation) = &self.returning {
            let home_pile = match animation.source {
                Source::Tableau { column, .. } => {
                    let last = cards_to_paint.iter().rev().find(|(_, _, _, source)|
                        matches!(source, Some(Source::Tableau { column: c, .. }) if *c == column));
                    last.map(|(rect, _, _, _)| at(column, h + 42.0).union(*rect))
                }
                _ => return_home,
            };
            if ui.input(|input| input.time) - animation.started_at >= RETURN_SECONDS
                || home_pile.is_none()
                || home_pile.is_some_and(|rect| ui.rect_contains_pointer(rect))
            {
                self.returning = None;
            }
        }
        let moving = self
            .drag
            .as_ref()
            .map(|drag| drag.source)
            .or_else(|| self.returning.as_ref().map(|animation| animation.source));
        for (rect, value, selected, source) in cards_to_paint {
            if !matches!((source, moving), (Some(card), Some(moving)) if belongs_to(card, moving)) {
                paint_card(ui, rect, value, selected);
            }
        }
        if let (Some(animation), Some(home)) = (&self.returning, return_home) {
            let progress = ((ui.input(|input| input.time) - animation.started_at) / RETURN_SECONDS)
                .clamp(0.0, 1.0) as f32;
            let eased = 1.0 - (1.0 - progress).powi(3);
            let position = animation.from.lerp(home.min, eased);
            for (index, &value) in animation.cards.iter().enumerate() {
                let rect = Rect::from_min_size(
                    position + egui::vec2(0.0, index as f32 * front_step),
                    egui::vec2(w, h),
                );
                paint_card(ui, rect, Some(value), false);
            }
            ui.ctx().request_repaint();
        }
        let candidate = targets
            .iter()
            .rev()
            .find(|(_, response, _)| response.contains_pointer());
        if let (Some(from), Some((to, _, rect))) = (self.selected, candidate) {
            let assessment = game.assess_move(from, *to);
            let color = if assessment.is_ok() {
                Color32::from_rgb(80, 255, 125)
            } else {
                Color32::from_rgb(255, 80, 80)
            };
            ui.painter().rect_stroke(
                rect.expand(3.0),
                5.0,
                Stroke::new(3.0, color),
                StrokeKind::Outside,
            );
            if let Err(reason) = assessment {
                // A tooltip is visual only; it must not take input from the drop target.
                egui::Area::new(ui.id().with("move_reason"))
                    .order(egui::Order::Tooltip)
                    .interactable(false)
                    .fixed_pos(rect.left_bottom() + egui::vec2(0.0, 6.0))
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.set_max_width(280.0);
                            ui.colored_label(color, reason.to_string());
                        });
                    });
            }
        }
        if let Some(drag) = &self.drag {
            if let Some(position) = ui.input(|input| input.pointer.interact_pos()) {
                let cards = if let Source::Foundation(suit) = drag.source {
                    game.foundation(suit).into_iter().collect::<Vec<_>>()
                } else {
                    game.movable_cards(drag.source).unwrap_or_default().to_vec()
                };
                for (index, value) in cards.into_iter().enumerate() {
                    let ghost = Rect::from_min_size(
                        position - drag.offset + egui::vec2(0.0, index as f32 * front_step),
                        egui::vec2(w, h),
                    );
                    paint_card(ui, ghost, Some(value), true);
                }
            }
            let released = sources
                .iter()
                .any(|(source, response, _)| *source == drag.source && response.drag_stopped());
            if released {
                let source = drag.source;
                let from = ui
                    .input(|input| input.pointer.interact_pos())
                    .map(|position| position - drag.offset);
                let cards = if let Source::Foundation(suit) = source {
                    game.foundation(suit).into_iter().collect()
                } else {
                    game.movable_cards(source).unwrap_or_default().to_vec()
                };
                let destination = candidate.map(|(to, _, _)| *to);
                self.drag = None;
                self.selected = None;
                let accepted =
                    destination.is_some_and(|to| self.execute(Command::Move { from: source, to }));
                if !accepted && let Some(from) = from {
                    self.returning = Some(ReturnAnimation {
                        source,
                        cards,
                        from,
                        started_at: ui.input(|input| input.time),
                    });
                }
                ui.ctx().request_repaint();
                return;
            }
        }
        if let Some(click) = click {
            self.clicked(click);
            ui.ctx().request_repaint();
        }
    }
}

impl eframe::App for PatienceApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(fragment) = self.platform.changed_fragment() {
            match Game::decode(&fragment) {
                Ok(game) => {
                    self.engine = Engine::new(game);
                    self.selected = None;
                    self.drag = None;
                    self.returning = None;
                    self.load_error = None;
                    self.message.clear();
                }
                Err(error) => {
                    self.returning = None;
                    self.load_error = Some(error.to_string());
                }
            }
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.selected = None;
            self.drag = None;
            self.returning = None;
        }
        if !ctx.input(|input| input.focused) {
            self.drag = None;
            self.returning = None;
            self.selected = None;
        }
        if self.load_error.is_none()
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z))
            && self.engine.undo()
        {
            self.selected = None;
            self.drag = None;
            self.returning = None;
            self.message.clear();
            self.save_url();
        }
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("Klondike");
                ui.add_space(12.0);
                egui::ComboBox::from_id_salt("new_game_draw_mode")
                    .selected_text(format!("Draw {}", self.next_draw_mode.count()))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.next_draw_mode,
                            DrawMode::Three,
                            "Draw three",
                        );
                        ui.selectable_value(&mut self.next_draw_mode, DrawMode::One, "Draw one");
                    })
                    .response
                    .on_hover_text(
                        "Rules for the next new game. The current game keeps its rules.",
                    );
                if ui.button("New game").clicked() {
                    self.engine =
                        Engine::new(Game::deal_with_mode(rand::random(), self.next_draw_mode));
                    self.selected = None;
                    self.drag = None;
                    self.returning = None;
                    self.load_error = None;
                    self.message.clear();
                    self.save_url();
                }
                if ui
                    .add_enabled(
                        self.load_error.is_none() && self.engine.can_undo(),
                        egui::Button::new("Undo"),
                    )
                    .clicked()
                {
                    self.engine.undo();
                    self.selected = None;
                    self.drag = None;
                    self.returning = None;
                    self.message.clear();
                    self.save_url();
                }
                ui.label(format!(
                    "Draw {} · {} / 52",
                    self.engine.game().draw_mode().count(),
                    self.engine.game().foundation_count()
                ));
                if self.engine.game().is_won() {
                    ui.strong("You won!");
                }
            });
        });
        egui::TopBottomPanel::bottom("help").show(ctx, |ui| {
            ui.label("Drag a card or sequence and release on its destination. Green allows the move; red explains why it is blocked.");
            ui.small("You can also click a card, then its destination. Double-click sends a card to a foundation. Esc cancels.");
            #[cfg(target_arch = "wasm32")]
            ui.small("The address saves your board after every move. Bookmark it to continue later.");
            if !self.message.is_empty() { ui.colored_label(Color32::YELLOW, &self.message); }
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(TABLE).inner_margin(12.0)).show(ctx, |ui| {
            if let Some(error) = &self.load_error {
                ui.colored_label(Color32::WHITE, error);
                ui.label("The link has been left unchanged. Open a valid game link or choose New game.");
            } else { egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| self.board(ui)); }
        });
    }
}

fn slot(ui: &egui::Ui, rect: Rect, label: &str) {
    ui.painter().rect(
        rect,
        5.0,
        Color32::from_black_alpha(22),
        Stroke::new(1.0, Color32::from_white_alpha(90)),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(28.0),
        Color32::from_white_alpha(130),
    );
}

fn paint_card(ui: &egui::Ui, rect: Rect, value: Option<Card>, selected: bool) {
    let border = if selected {
        Color32::from_rgb(255, 204, 65)
    } else {
        Color32::from_gray(45)
    };
    let fill = if value.is_some() {
        Color32::from_rgb(255, 253, 244)
    } else {
        Color32::from_rgb(35, 65, 125)
    };
    ui.painter().rect(
        rect,
        5.0,
        fill,
        Stroke::new(if selected { 3.0 } else { 1.0 }, border),
        StrokeKind::Inside,
    );
    if let Some(value) = value {
        let ink = if value.suit().is_red() {
            Color32::from_rgb(181, 28, 40)
        } else {
            Color32::from_rgb(23, 27, 34)
        };
        let label = format!("{}{}", value.rank_label(), value.suit().symbol());
        let font = FontId::proportional(rect.width() * 0.23);
        ui.painter().text(
            rect.min + egui::vec2(6.0, 4.0),
            Align2::LEFT_TOP,
            &label,
            font.clone(),
            ink,
        );
        ui.painter().text(
            rect.max - egui::vec2(6.0, 4.0),
            Align2::RIGHT_BOTTOM,
            label,
            font,
            ink,
        );
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            value.suit().symbol(),
            FontId::proportional(rect.width() * 0.47),
            ink,
        );
    } else {
        ui.painter().rect_stroke(
            rect.shrink(5.0),
            2.0,
            Stroke::new(1.0, Color32::from_white_alpha(130)),
            StrokeKind::Inside,
        );
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn app(game: Game, ctx: &egui::Context) -> PatienceApp {
        PatienceApp {
            engine: Engine::new(game),
            selected: None,
            drag: None,
            returning: None,
            next_draw_mode: DrawMode::Three,
            message: String::new(),
            load_error: None,
            platform: Platform::new(ctx),
        }
    }

    fn frame(app: &mut PatienceApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000.0, 900.0),
            )),
            events,
            focused: true,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| app.board(ui));
        });
    }

    fn button(position: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn playable_pair(legal: bool) -> (Game, Source, Destination) {
        for seed in 0..100 {
            let game = Game::deal(seed);
            for from in 0..7 {
                for to in 0..7 {
                    let source = Source::Tableau {
                        column: from,
                        index: from,
                    };
                    let destination = Destination::Tableau(to);
                    if from != to && game.can_move(source, destination) == legal {
                        return (game, source, destination);
                    }
                }
            }
        }
        panic!("test deals must contain a suitable move");
    }

    fn column_center(column: usize) -> egui::Pos2 {
        // Test the rendered board at a fixed 1000-point window width.
        egui::pos2(
            80.0 + column as f32 * 122.0 + 54.0,
            201.2 + column as f32 * 19.44 + 75.6,
        )
    }

    fn drag(app: &mut PatienceApp, ctx: &egui::Context, from: egui::Pos2, to: egui::Pos2) {
        frame(app, ctx, vec![]);
        frame(app, ctx, vec![]);
        frame(
            app,
            ctx,
            vec![egui::Event::PointerMoved(from), button(from, true)],
        );
        frame(
            app,
            ctx,
            vec![egui::Event::PointerMoved(from + egui::vec2(15.0, 0.0))],
        );
        assert!(app.drag.is_some(), "egui must acquire the card drag");
        frame(app, ctx, vec![egui::Event::PointerMoved(to)]);
    }

    #[test]
    fn dragging_previews_then_commits_only_on_release() {
        let (game, source, to) = playable_pair(true);
        let Source::Tableau { column: from, .. } = source else {
            unreachable!()
        };
        let Destination::Tableau(destination) = to else {
            unreachable!()
        };
        let expected = game.updated(Command::Move { from: source, to }).unwrap();
        let ctx = egui::Context::default();
        let mut app = app(game.clone(), &ctx);
        let position = column_center(destination);
        drag(&mut app, &ctx, column_center(from), position);
        assert_eq!(app.engine.game(), &game);
        assert!(!app.engine.can_undo());
        frame(&mut app, &ctx, vec![button(position, false)]);
        assert_eq!(app.engine.game(), &expected);
        assert!(app.engine.can_undo());
        assert!(app.drag.is_none());
    }

    #[test]
    fn illegal_drop_and_drop_outside_board_leave_state_unchanged() {
        for outside in [false, true] {
            let (game, source, to) = playable_pair(false);
            let Source::Tableau { column: from, .. } = source else {
                unreachable!()
            };
            let Destination::Tableau(destination) = to else {
                unreachable!()
            };
            let ctx = egui::Context::default();
            let mut app = app(game.clone(), &ctx);
            let position = if outside {
                egui::pos2(5.0, 100.0)
            } else {
                column_center(destination)
            };
            drag(&mut app, &ctx, column_center(from), position);
            frame(&mut app, &ctx, vec![button(position, false)]);
            assert_eq!(app.engine.game(), &game);
            assert!(!app.engine.can_undo());
            assert!(app.drag.is_none());
            assert!(app.returning.is_some());
            if !outside {
                assert!(!app.message.is_empty());
            }
            for _ in 0..15 {
                frame(&mut app, &ctx, vec![]);
            }
            assert!(app.returning.is_none());
            assert_eq!(app.engine.game(), &game);
        }
    }

    #[test]
    fn returning_cards_restore_immediately_when_pointer_enters_the_original_pile() {
        let (game, source, to) = playable_pair(false);
        let Source::Tableau { column: from, .. } = source else {
            unreachable!()
        };
        let Destination::Tableau(destination) = to else {
            unreachable!()
        };
        let ctx = egui::Context::default();
        let mut app = app(game.clone(), &ctx);
        let position = column_center(destination);
        drag(&mut app, &ctx, column_center(from), position);
        frame(&mut app, &ctx, vec![button(position, false)]);
        let started_at = app.returning.as_ref().unwrap().started_at;
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(column_center(from))],
        );
        assert!(ctx.input(|input| input.time) - started_at < RETURN_SECONDS);
        assert!(app.returning.is_none());
        assert_eq!(app.engine.game(), &game);
        assert!(!app.engine.can_undo());
    }
}
