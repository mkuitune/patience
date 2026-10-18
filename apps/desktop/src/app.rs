use crate::cards::Cards;
use crate::language::{self, Key, LANGUAGES, Language};
use crate::platform::Platform;
use crate::toolbar::{self, Icon};
use eframe::egui::{self, Color32, FontId, Rect, Sense, Stroke, StrokeKind};
use patience_core::{Card, Command, Destination, Engine, Game, Source, StateError};

const ROW_GAP: f32 = 28.0;
const BOARD_VERTICAL_PADDING: f32 = 16.0;
const FACE_UP_STEP: f32 = 0.18;
const FULL_COLUMN_HEIGHT_FACTOR: f32 = 1.4 + 12.0 * FACE_UP_STEP;

fn tableau_height_factor(game: &Game) -> f32 {
    game.tableau()
        .iter()
        .map(|pile| {
            pile.hidden() as f32 * 0.18
                + pile.cards().len().saturating_sub(pile.hidden() + 1) as f32 * FACE_UP_STEP
                + 1.4
        })
        .fold(1.4, f32::max)
}

fn board_widths(available: egui::Vec2, expanded: bool, game: &Game) -> (f32, f32, f32) {
    let width = available.x.max(if expanded { 91.0 } else { 650.0 });
    let width_limit = ((width - 84.0) / 7.0).max(1.0);
    let card = if expanded {
        let column_height = tableau_height_factor(game).max(FULL_COLUMN_HEIGHT_FACTOR);
        let height_limit =
            ((available.y - ROW_GAP - BOARD_VERTICAL_PADDING) / (1.4 + column_height)).max(1.0);
        width_limit.min(height_limit)
    } else {
        width_limit.clamp(74.0, 108.0)
    };
    (width, card, card * 7.0 + 84.0)
}

pub struct PatienceApp {
    cards: Cards,
    table_texture: egui::TextureHandle,
    engine: Engine,
    waste_fan_count: usize,
    undo_waste_fan_count: Option<usize>,
    selected: Option<Source>,
    drag: Option<Drag>,
    returning: Option<ReturnAnimation>,
    teach: bool,
    expanded_layout: bool,
    mobile_layout: bool,
    toolbar_height: f32,
    rejected: Option<RejectedDrop>,
    message_is_rule: bool,
    message: String,
    load_error: Option<StateError>,
    language: &'static Language,
    settings_open: bool,
    help_open: bool,
    help_document: crate::help::HelpDocument,
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
const REJECTION_SECONDS: f64 = 0.20;

struct RejectedDrop {
    destination: Destination,
    started_at: Option<f64>,
}

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

fn same_pile(from: Source, to: Destination) -> bool {
    match (from, to) {
        (Source::Tableau { column, .. }, Destination::Tableau(target)) => column == target,
        (Source::Foundation(suit), Destination::Foundation(target)) => suit == target,
        _ => false,
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
    pub fn new(cc: &eframe::CreationContext<'_>, mobile_layout: bool) -> Self {
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
        let language = language::by_code(&platform.language().unwrap_or_default());
        platform.set_title(&cc.egui_ctx, language.title, language.code);
        let fragment = platform.fragment();
        let restored = if fragment.is_empty() {
            Ok(Game::deal(rand::random()))
        } else {
            Game::decode(&fragment)
        };
        let (game, load_error) = match restored {
            Ok(game) => (game, None),
            Err(error) => (Game::deal(rand::random()), Some(error)),
        };
        let mut app = Self {
            cards: Cards::new(),
            table_texture: crate::table::load(&cc.egui_ctx),
            engine: Engine::new(game),
            waste_fan_count: 0,
            undo_waste_fan_count: None,
            selected: None,
            drag: None,
            returning: None,
            teach: false,
            expanded_layout: true,
            mobile_layout,
            toolbar_height: toolbar::BUTTON_HEIGHT + 24.0,
            rejected: None,
            message_is_rule: false,
            message: String::new(),
            load_error,
            language,
            settings_open: false,
            help_open: false,
            help_document: crate::help::HelpDocument::default(),
            platform,
        };
        if app.load_error.is_none() {
            app.save_url();
        }
        app
    }

    fn control(&mut self, ui: &mut egui::Ui, icon: Icon, show_label: bool) {
        let (key, selected, enabled) = match icon {
            Icon::NewGame => (Key::NewGame, None, true),
            Icon::Undo => (
                Key::Undo,
                None,
                self.load_error.is_none() && self.engine.can_undo(),
            ),
            Icon::Help => (Key::Help, None, true),
            Icon::Teach => (Key::Teach, Some(self.teach), true),
            Icon::Settings => (Key::Settings, None, true),
            Icon::Expand | Icon::Collapse => (
                if self.expanded_layout {
                    Key::CenterLayout
                } else {
                    Key::ExpandLayout
                },
                Some(self.expanded_layout),
                true,
            ),
            Icon::Close => unreachable!("Close belongs to a dialog"),
        };
        let label = self.language.text(key);
        let response = toolbar::button(ui, label, icon, show_label, selected, enabled);
        let response = if !show_label && (!matches!(icon, Icon::Undo) || self.teach) {
            response.on_hover_text(label)
        } else {
            response
        };
        if !response.clicked() {
            return;
        }
        match icon {
            Icon::NewGame => {
                self.engine = Engine::new(Game::deal(rand::random()));
                self.waste_fan_count = 0;
                self.undo_waste_fan_count = None;
                self.rejected = None;
                self.selected = None;
                self.drag = None;
                self.returning = None;
                self.load_error = None;
                self.message.clear();
                self.save_url();
            }
            Icon::Undo => {
                self.undo();
            }
            Icon::Help => {
                self.help_open = true;
                self.settings_open = false;
                self.drag = None;
                self.selected = None;
                self.returning = None;
            }
            Icon::Teach => {
                self.teach = !self.teach;
                self.rejected = None;
                if self.message_is_rule {
                    self.message.clear();
                }
            }
            Icon::Settings => {
                self.settings_open = true;
                self.help_open = false;
                self.drag = None;
                self.selected = None;
            }
            Icon::Expand | Icon::Collapse => {
                self.expanded_layout = !self.expanded_layout;
                self.selected = None;
                self.drag = None;
                self.returning = None;
                self.rejected = None;
                ui.ctx().request_repaint();
            }
            Icon::Close => unreachable!("Close belongs to a dialog"),
        }
    }

    fn navigation(&mut self, ctx: &egui::Context) {
        let landscape =
            self.mobile_layout && ctx.content_rect().width() > ctx.content_rect().height();
        let frame = egui::Frame::new()
            .fill(Color32::TRANSPARENT)
            .inner_margin(egui::Margin::symmetric(12, 12));
        if landscape {
            self.toolbar_height = 0.0;
            let controls_width = ctx.fonts_mut(|fonts| {
                [Key::NewGame, Key::Help]
                    .into_iter()
                    .map(|key| {
                        fonts
                            .layout_no_wrap(
                                self.language.text(key).to_owned(),
                                FontId::proportional(14.0),
                                Color32::WHITE,
                            )
                            .size()
                            .x
                            + 48.0
                    })
                    .fold(toolbar::BUTTON_HEIGHT, f32::max)
            });
            egui::SidePanel::left("mobile_controls")
                .resizable(false)
                .exact_width(controls_width + 24.0)
                .show_separator_line(false)
                .frame(frame)
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 10.0;
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for icon in [
                                Icon::Undo,
                                Icon::NewGame,
                                Icon::Help,
                                Icon::Teach,
                                Icon::Settings,
                            ] {
                                self.control(ui, icon, matches!(icon, Icon::NewGame | Icon::Help));
                            }
                        });
                });
            return;
        }
        let toolbar_panel = egui::TopBottomPanel::top("toolbar")
            .show_separator_line(false)
            .frame(frame)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
                let (_, _, playing_width) = board_widths(
                    egui::vec2(
                        ui.available_width(),
                        (ctx.content_rect().height() - self.toolbar_height - 16.0).max(0.0),
                    ),
                    self.expanded_layout || self.mobile_layout,
                    self.engine.game(),
                );
                // Keep controls usable when a short viewport makes the fitted cards narrower.
                let width = playing_width
                    .max(if self.expanded_layout { 450.0 } else { 0.0 })
                    .min(ui.available_width());
                let origin =
                    ui.cursor().min + egui::vec2((ui.available_width() - width) * 0.5, 0.0);
                let rect = Rect::from_min_size(origin, egui::vec2(width, toolbar::BUTTON_HEIGHT));
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), toolbar::BUTTON_HEIGHT),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            self.control(ui, Icon::Settings, false);
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center)
                                    .with_main_wrap(true),
                                |ui| {
                                    self.control(ui, Icon::NewGame, !self.mobile_layout);
                                    self.control(ui, Icon::Undo, false);
                                    self.control(ui, Icon::Help, !self.mobile_layout);
                                    self.control(ui, Icon::Teach, !self.mobile_layout);
                                    if !self.mobile_layout {
                                        self.control(
                                            ui,
                                            if self.expanded_layout {
                                                Icon::Collapse
                                            } else {
                                                Icon::Expand
                                            },
                                            false,
                                        );
                                    }
                                    if self.engine.game().is_won() {
                                        ui.strong(self.language.text(Key::Won));
                                    }
                                },
                            );
                        },
                    );
                });
            });
        let toolbar_height = toolbar_panel.response.rect.height();
        if (toolbar_height - self.toolbar_height).abs() > 0.1 {
            self.toolbar_height = toolbar_height;
            ctx.request_repaint();
        }
    }

    fn save_url(&mut self) {
        if let Err(error) = self.platform.save(&self.engine.game().encode()) {
            self.message_is_rule = false;
            self.message = format!("{}: {error}", self.language.text(Key::SaveError));
        }
    }

    fn execute(&mut self, command: Command) -> bool {
        if matches!(command, Command::Move { from, to } if same_pile(from, to)) {
            self.selected = None;
            self.drag = None;
            self.returning = None;
            self.rejected = None;
            if self.message_is_rule {
                self.message.clear();
            }
            return false;
        }
        let waste_before = self.engine.game().waste().len();
        match self.engine.execute(command) {
            Ok(_) => {
                self.undo_waste_fan_count = Some(self.waste_fan_count);
                self.waste_fan_count = match command {
                    Command::Draw => self
                        .engine
                        .game()
                        .waste()
                        .len()
                        .saturating_sub(waste_before),
                    Command::Move {
                        from: Source::Waste,
                        ..
                    } => self.waste_fan_count.saturating_sub(1),
                    _ => self.waste_fan_count,
                };
                self.selected = None;
                self.drag = None;
                self.returning = None;
                self.rejected = None;
                self.message.clear();
                self.save_url();
                true
            }
            Err(error) => {
                self.message_is_rule = true;
                self.message = self.language.move_error(error).into();
                if let Command::Move { to, .. } = command {
                    self.rejected = Some(RejectedDrop {
                        destination: to,
                        started_at: None,
                    });
                }
                false
            }
        }
    }

    fn undo(&mut self) -> bool {
        if !self.engine.undo() {
            return false;
        }
        self.waste_fan_count = self.undo_waste_fan_count.take().unwrap_or(0);
        self.rejected = None;
        self.selected = None;
        self.drag = None;
        self.returning = None;
        self.message.clear();
        self.save_url();
        true
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
                        self.message_is_rule = true;
                        self.message = self.language.text(Key::CannotHome).into();
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

    fn board(&mut self, ui: &mut egui::Ui, viewport_height: f32) {
        let (width, w, playing_width) = board_widths(
            egui::vec2(ui.available_width(), viewport_height),
            self.expanded_layout || self.mobile_layout,
            self.engine.game(),
        );
        let h = w * 1.4;
        let front_step = w * FACE_UP_STEP;
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
        let (board, _) = ui.allocate_exact_size(
            egui::vec2(width, h + ROW_GAP + BOARD_VERTICAL_PADDING + max_height),
            Sense::hover(),
        );
        let origin = board.min + egui::vec2((width - playing_width) * 0.5, 8.0);
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
            self.cards.paint(ui, stock, None, false);
        }
        let stock_response = ui.interact(stock, ui.id().with("stock"), Sense::click());
        let stock_response = if self.teach {
            stock_response.on_hover_text(self.language.text(Key::Draw))
        } else {
            stock_response
        };
        if stock_response.clicked() {
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
                Color32::from_black_alpha(80),
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
        slot(ui, waste, "");
        let visible = game.waste().len().min(self.waste_fan_count.max(1));
        for (index, &value) in game
            .waste()
            .iter()
            .skip(game.waste().len() - visible)
            .enumerate()
        {
            let target = waste.translate(egui::vec2(index as f32 * w * 0.24, 0.0));
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
        for suit in 0..4 {
            let target = at(3 + suit, 0.0);
            slot(ui, target, "");
            self.cards.paint_rank_marker(ui, target, 1, 42);
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
            let empty = at(column, h + ROW_GAP);
            slot(ui, empty, "");
            self.cards.paint_rank_marker(ui, empty, 13, 42);
            if pile.cards().is_empty() {
                let response = ui.interact(empty, ui.id().with(("empty", column)), Sense::click());
                if response.clicked() {
                    click = Some(Click::Empty(Destination::Tableau(column)));
                }
                targets.push((Destination::Tableau(column), response, empty));
            }
            let mut y = h + ROW_GAP;
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
                self.rejected = None;
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
                    last.map(|(rect, _, _, _)| at(column, h + ROW_GAP).union(*rect))
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
                self.cards.paint(ui, rect, value, selected);
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
                self.cards.paint(ui, rect, Some(value), false);
            }
            ui.ctx().request_repaint();
        }
        let candidate = targets
            .iter()
            .rev()
            .find(|(_, response, _)| response.contains_pointer());
        if self.teach {
            for command in game.suggested_moves() {
                let endpoints = match command {
                    Command::Move { from, to } => {
                        let from = sources
                            .iter()
                            .find(|(source, _, _)| *source == from)
                            .map(|(_, response, _)| response.rect);
                        let to = targets
                            .iter()
                            .rev()
                            .find(|(destination, _, _)| *destination == to)
                            .map(|(_, _, rect)| *rect);
                        from.zip(to)
                    }
                    Command::Draw => {
                        click_marker(ui.painter(), stock);
                        None
                    }
                };
                if let Some((from, to)) = endpoints {
                    let over_cards = matches!(
                        command,
                        Command::Move {
                            from: Source::Waste,
                            to: Destination::Foundation(_),
                        }
                    );
                    move_arrow(ui.painter(), from, to, over_cards);
                }
            }
        }
        if self.teach
            && let (Some(from), Some((to, _, rect))) = (self.selected, candidate)
            && !same_pile(from, *to)
        {
            let assessment = game.assess_move(from, *to);
            let color = if assessment.is_ok() {
                Color32::from_rgb(80, 255, 125)
            } else {
                Color32::from_rgb(255, 80, 80)
            };
            ui.painter().rect_stroke(
                rect.expand(3.0),
                crate::cards::corner_radius(*rect) + 3.0,
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
                            ui.colored_label(color, self.language.move_error(reason));
                        });
                    });
            }
        }
        if let Some(rejected) = &mut self.rejected {
            let now = ui.input(|input| input.time);
            let start = *rejected.started_at.get_or_insert(now);
            let progress = ((now - start) / REJECTION_SECONDS).clamp(0.0, 1.0) as f32;
            if progress >= 1.0 {
                self.rejected = None;
            } else if !self.teach {
                if let Some((_, _, rect)) = targets
                    .iter()
                    .rev()
                    .find(|(to, _, _)| *to == rejected.destination)
                {
                    ui.painter().rect_stroke(
                        rect.expand(3.0),
                        crate::cards::corner_radius(*rect) + 3.0,
                        Stroke::new(
                            3.0,
                            Color32::from_rgba_unmultiplied(
                                255,
                                65,
                                65,
                                (255.0 * (1.0 - progress)) as u8,
                            ),
                        ),
                        StrokeKind::Outside,
                    );
                }
                ui.ctx().request_repaint();
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
                    self.cards.paint(ui, ghost, Some(value), true);
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
        crate::table::paint(ctx, &self.table_texture);
        self.cards.begin_frame(ctx);
        if let Some(fragment) = self.platform.changed_fragment() {
            match Game::decode(&fragment) {
                Ok(game) => {
                    self.engine = Engine::new(game);
                    self.waste_fan_count = 0;
                    self.undo_waste_fan_count = None;
                    self.rejected = None;
                    self.selected = None;
                    self.drag = None;
                    self.returning = None;
                    self.load_error = None;
                    self.message.clear();
                }
                Err(error) => {
                    self.rejected = None;
                    self.returning = None;
                    self.load_error = Some(error);
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
        if !self.settings_open
            && !self.help_open
            && self.load_error.is_none()
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z))
        {
            self.undo();
        }
        self.navigation(ctx);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::TRANSPARENT)
                    .inner_margin(egui::Margin {
                        left: 12,
                        right: 12,
                        top: 4,
                        bottom: 12,
                    }),
            )
            .show(ctx, |ui| {
                if !self.message.is_empty() && !self.message_is_rule {
                    ui.colored_label(Color32::WHITE, &self.message);
                }
                if let Some(error) = &self.cards.error {
                    ui.colored_label(Color32::WHITE, error);
                }
                if let Some(error) = &self.load_error {
                    ui.colored_label(Color32::WHITE, self.language.state_error(*error));
                    ui.label(self.language.text(Key::InvalidLinkHelp));
                } else {
                    let viewport_height = ui.available_height();
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.board(ui, viewport_height));
                }
            });
        if self.help_open && self.help_document.show(ctx, self.language) {
            self.help_open = false;
        }
        if self.settings_open {
            let modal = egui::Modal::new(egui::Id::new("settings"))
                .frame(toolbar::panel_frame())
                .backdrop_color(Color32::from_black_alpha(45))
                .show(ctx, |ui| {
                    toolbar::settings_style(ui);
                    ui.set_width(
                        300.0_f32
                            .min(ui.ctx().content_rect().width() - 72.0)
                            .max(160.0),
                    );
                    ui.heading(egui::RichText::new(self.language.text(Key::Settings)).size(23.0));
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new(self.language.text(Key::Language))
                            .color(Color32::from_white_alpha(165)),
                    );
                    egui::ComboBox::from_id_salt("language")
                        .width(ui.available_width())
                        .selected_text(self.language.name)
                        .show_ui(ui, |ui| {
                            for language in LANGUAGES {
                                if ui
                                    .selectable_label(
                                        self.language.code == language.code,
                                        language.name,
                                    )
                                    .clicked()
                                {
                                    self.language = language;
                                    self.platform.set_title(ctx, language.title, language.code);
                                    if self.message_is_rule {
                                        self.message.clear();
                                    }
                                    if let Err(error) = self.platform.save_language(language.code) {
                                        self.message_is_rule = false;
                                        self.message =
                                            format!("{}: {error}", language.text(Key::SaveError));
                                    }
                                }
                            }
                        });
                    ui.add_space(12.0);
                    if toolbar::button(
                        ui,
                        self.language.text(Key::Close),
                        Icon::Close,
                        true,
                        None,
                        true,
                    )
                    .clicked()
                    {
                        ui.close();
                    }
                });
            if modal.should_close() {
                self.settings_open = false;
            }
        }
    }
}

fn slot(ui: &egui::Ui, rect: Rect, label: &str) {
    ui.painter().rect(
        rect,
        crate::cards::corner_radius(rect),
        Color32::from_black_alpha(22),
        Stroke::new(1.0, Color32::from_white_alpha(90)),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(28.0),
        Color32::from_white_alpha(130),
    );
}

fn click_marker(painter: &egui::Painter, target: Rect) {
    let tip = target.right_top() + egui::vec2(-24.0, 20.0);
    let ink = Color32::from_rgb(255, 222, 146);
    let cursor = [
        (0.0, 0.0),
        (0.0, 20.0),
        (5.0, 15.0),
        (10.0, 24.0),
        (14.0, 22.0),
        (9.0, 13.0),
        (17.0, 12.0),
        (0.0, 0.0),
    ]
    .map(|(x, y)| tip + egui::vec2(x, y))
    .to_vec();
    painter.add(egui::Shape::line(
        cursor.clone(),
        Stroke::new(4.0, Color32::from_black_alpha(120)),
    ));
    painter.add(egui::Shape::line(cursor, Stroke::new(1.6, ink)));
    for (start, end) in [
        ((-6.0, -3.0), (-12.0, -6.0)),
        ((0.0, -7.0), (0.0, -13.0)),
        ((6.0, -4.0), (11.0, -9.0)),
    ] {
        painter.line_segment(
            [
                tip + egui::vec2(start.0, start.1),
                tip + egui::vec2(end.0, end.1),
            ],
            Stroke::new(1.6, ink),
        );
    }
}

fn move_arrow(painter: &egui::Painter, from: Rect, to: Rect, over_cards: bool) {
    let start = from.center();
    let end = to.center();
    let direction = if end.y > start.y + 40.0 { 1.0 } else { -1.0 };
    let mut bend = (start.distance(end) * 0.22).clamp(24.0, 90.0);
    if over_cards {
        bend = bend.min(from.height().min(to.height()) * 0.4);
    }
    let control1 = start + egui::vec2(0.0, direction * bend);
    let tangent = (end - control1).normalized();
    let control2 = end - tangent * bend;
    let points = [start, control1, control2, end];
    let normal = egui::vec2(-tangent.y, tangent.x);
    let head = vec![
        end - tangent * 8.0 + normal * 4.0,
        end,
        end - tangent * 8.0 - normal * 4.0,
    ];
    let ink = Color32::from_rgba_unmultiplied(255, 222, 146, 220);
    for stroke in [
        Stroke::new(4.0, Color32::from_black_alpha(95)),
        Stroke::new(1.6, ink),
    ] {
        painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
            points,
            false,
            Color32::TRANSPARENT,
            stroke,
        ));
        painter.add(egui::Shape::line(head.clone(), stroke));
    }
    painter.circle_filled(start, 4.5, Color32::from_black_alpha(95));
    painter.circle_filled(start, 3.0, ink);
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn mobile_controls_rotate_to_the_side_and_keep_undo_first() {
        let ctx = egui::Context::default();
        let original = Game::deal(7);
        let mut app = app(original.clone(), &ctx);
        app.mobile_layout = true;
        app.teach = true;
        app.execute(Command::Draw);
        let drawn = app.engine.game().clone();
        let render = |app: &mut PatienceApp, size, events| {
            let mut available = Rect::NOTHING;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |ctx| {
                    app.navigation(ctx);
                    available = ctx.available_rect();
                },
            );
            (available, output)
        };
        let (portrait, output) = render(&mut app, egui::vec2(390.0, 844.0), vec![]);
        assert!(portrait.top() > 0.0);
        assert_eq!(portrait.left(), 0.0);
        for label in ["New game", "Help", "Teach"] {
            assert!(!output.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == label)
            }));
        }
        let size = egui::vec2(844.0, 390.0);
        render(&mut app, size, vec![]);
        let (landscape, output) = render(&mut app, size, vec![]);
        assert_eq!(landscape.top(), 0.0);
        assert!(landscape.left() > toolbar::BUTTON_HEIGHT + 24.0);
        assert_eq!(landscape.height(), size.y);
        for label in ["New game", "Help"] {
            assert!(output.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == label)
            }));
        }
        let (_, with_sidebar, _) = board_widths(
            landscape.size() - egui::vec2(24.0, 16.0),
            true,
            app.engine.game(),
        );
        let (_, with_topbar, _) = board_widths(
            size - egui::vec2(24.0, toolbar::BUTTON_HEIGHT + 40.0),
            true,
            app.engine.game(),
        );
        assert!(with_sidebar > with_topbar);
        assert_eq!(app.engine.game(), &drawn);
        assert!(app.teach);
        let undo = egui::pos2(33.0, 33.0);
        render(&mut app, size, vec![egui::Event::PointerMoved(undo)]);
        render(&mut app, size, vec![button(undo, true)]);
        render(&mut app, size, vec![button(undo, false)]);
        assert_eq!(app.engine.game(), &original);
        assert!(!app.engine.can_undo());
    }

    #[test]
    fn expanded_layout_reserves_thirteen_cards_and_fits_taller_columns() {
        let mut game = Game::deal(7);
        for _ in 0..120 {
            for size in [
                egui::vec2(1800.0, 800.0),
                egui::vec2(800.0, 500.0),
                egui::vec2(360.0, 650.0),
            ] {
                let (_, width, playing_width) = board_widths(size, true, &game);
                assert!(playing_width <= size.x + 0.01);
                let column_height = tableau_height_factor(&game).max(FULL_COLUMN_HEIGHT_FACTOR);
                let height = (1.4 + column_height) * width + ROW_GAP + BOARD_VERTICAL_PADDING;
                assert!(height <= size.y + 0.01, "{height} exceeds {}", size.y);
            }
            let Some(command) = game.legal_moves().into_iter().next() else {
                break;
            };
            game.apply(command).unwrap();
        }
    }

    fn app(game: Game, ctx: &egui::Context) -> PatienceApp {
        PatienceApp {
            cards: Cards::new(),
            table_texture: crate::table::load(ctx),
            engine: Engine::new(game),
            waste_fan_count: 0,
            undo_waste_fan_count: None,
            selected: None,
            drag: None,
            returning: None,
            teach: false,
            expanded_layout: false,
            mobile_layout: false,
            toolbar_height: toolbar::BUTTON_HEIGHT + 24.0,
            rejected: None,
            message_is_rule: false,
            message: String::new(),
            load_error: None,
            language: language::by_code("en"),
            settings_open: false,
            help_open: false,
            help_document: crate::help::HelpDocument::default(),
            platform: Platform::new(ctx),
        }
    }

    fn frame(
        app: &mut PatienceApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000.0, 900.0),
            )),
            events,
            focused: true,
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let height = ui.available_height();
                    app.board(ui, height);
                });
        })
    }

    fn button(position: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn waste_fan_shrinks_without_refilling_and_undo_restores_it() {
        let (game, moves) = (0..10_000)
            .find_map(|seed| {
                let mut game = Game::deal(seed);
                game.apply(Command::Draw).unwrap();
                let before_draw = game.clone();
                game.apply(Command::Draw).unwrap();
                let mut moves = Vec::new();
                for _ in 0..3 {
                    let command = game.legal_moves().into_iter().find(|command| {
                        matches!(
                            command,
                            Command::Move {
                                from: Source::Waste,
                                ..
                            }
                        )
                    })?;
                    game.apply(command).unwrap();
                    moves.push(command);
                }
                Some((before_draw, moves))
            })
            .expect("a deal with three playable waste cards");
        let ctx = egui::Context::default();
        let mut app = app(game, &ctx);
        assert!(app.execute(Command::Draw));
        assert_eq!(app.waste_fan_count, 3);
        for (command, remaining) in moves.into_iter().zip([2, 1, 0]) {
            assert!(app.execute(command));
            assert_eq!(app.waste_fan_count, remaining);
        }
        assert_eq!(app.engine.game().waste().len(), 3);
        assert!(app.undo());
        assert_eq!(app.waste_fan_count, 1);
        assert_eq!(app.engine.game().waste().len(), 4);
        assert!(app.execute(Command::Draw));
        assert_eq!(app.waste_fan_count, 3);
        assert!(app.undo());
        assert_eq!(app.waste_fan_count, 1);
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

    fn has_outline(output: &egui::FullOutput, color: Color32) -> bool {
        output.shapes.iter().any(
            |shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.stroke.color == color),
        )
    }

    #[test]
    fn returning_to_the_original_column_is_neutral_and_preserves_undo() {
        let (game, source, to) = playable_pair(true);
        let Destination::Tableau(column) = to else {
            unreachable!()
        };
        let game = game.updated(Command::Move { from: source, to }).unwrap();
        for help in [false, true] {
            let ctx = egui::Context::default();
            let mut app = app(game.clone(), &ctx);
            app.teach = help;
            app.engine.execute(Command::Draw).unwrap();
            let before = app.engine.game().clone();
            let start = column_center(column) + egui::vec2(0.0, 38.88);
            let target = column_center(column) - egui::vec2(0.0, 65.6);
            drag(&mut app, &ctx, start, target);
            let preview = frame(&mut app, &ctx, vec![]);
            assert!(!has_outline(&preview, Color32::from_rgb(255, 80, 80)));
            assert!(!has_outline(&preview, Color32::from_rgb(80, 255, 125)));
            frame(&mut app, &ctx, vec![button(target, false)]);
            assert!(app.message.is_empty());
            assert!(app.rejected.is_none());
            assert_eq!(app.engine.game(), &before);
            assert!(app.engine.undo());
            assert_eq!(app.engine.game(), &game);
        }
    }

    #[test]
    fn teach_controls_live_highlights_and_quiet_mode_flashes_only_after_release() {
        let green = Color32::from_rgb(80, 255, 125);
        for help in [false, true] {
            let (game, source, to) = playable_pair(true);
            let Source::Tableau { column: from, .. } = source else {
                unreachable!()
            };
            let Destination::Tableau(destination) = to else {
                unreachable!()
            };
            let ctx = egui::Context::default();
            let mut app = app(game, &ctx);
            app.teach = help;
            drag(
                &mut app,
                &ctx,
                column_center(from),
                column_center(destination),
            );
            let output = frame(&mut app, &ctx, vec![]);
            assert_eq!(has_outline(&output, green), help);
        }

        let (game, source, to) = playable_pair(false);
        let Source::Tableau { column: from, .. } = source else {
            unreachable!()
        };
        let Destination::Tableau(destination) = to else {
            unreachable!()
        };
        let ctx = egui::Context::default();
        let mut app = app(game.clone(), &ctx);
        app.teach = false;
        let position = column_center(destination);
        drag(&mut app, &ctx, column_center(from), position);
        assert!(app.rejected.is_none());
        let preview = frame(&mut app, &ctx, vec![]);
        assert!(!has_outline(&preview, Color32::from_rgb(255, 80, 80)));
        frame(&mut app, &ctx, vec![button(position, false)]);
        assert!(app.returning.is_some());
        assert!(app.rejected.is_some());
        let flash = frame(&mut app, &ctx, vec![]);
        assert!(has_outline(&flash, Color32::from_rgb(255, 65, 65)));
        for _ in 0..20 {
            frame(&mut app, &ctx, vec![]);
        }
        assert!(app.rejected.is_none());
        assert!(app.returning.is_none());
        assert_eq!(app.engine.game(), &game);
        assert!(!app.engine.can_undo());
    }

    #[test]
    fn teach_draws_every_legal_move_and_does_not_intercept_card_drags() {
        let (game, source, to) = playable_pair(true);
        let expected = game.updated(Command::Move { from: source, to }).unwrap();
        let count = game
            .suggested_moves()
            .iter()
            .filter(|command| matches!(command, Command::Move { .. }))
            .count();
        let ctx = egui::Context::default();
        let mut app = app(game.clone(), &ctx);
        app.teach = true;
        frame(&mut app, &ctx, vec![]);
        let output = frame(&mut app, &ctx, vec![]);
        let curves = output
            .shapes
            .iter()
            .filter(|shape| matches!(shape.shape, egui::Shape::CubicBezier(_)))
            .count();
        assert_eq!(curves, count * 2);
        assert_eq!(app.engine.game(), &game);
        let Source::Tableau { column: from, .. } = source else {
            unreachable!()
        };
        let Destination::Tableau(destination) = to else {
            unreachable!()
        };
        let position = column_center(destination);
        drag(&mut app, &ctx, column_center(from), position);
        frame(&mut app, &ctx, vec![button(position, false)]);
        assert_eq!(app.engine.game(), &expected);
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
