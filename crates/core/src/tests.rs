use crate::*;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

fn fixture(piles: &[(&[u8], usize)], waste: &[u8], foundations: [u8; 4]) -> Game {
    let tableau = std::array::from_fn(|column| {
        let (cards, hidden) = piles.get(column).copied().unwrap_or((&[], 0));
        Tableau {
            cards: cards.iter().copied().map(Card).collect(),
            hidden,
        }
    });
    let mut game = Game {
        draw_mode: DrawMode::One,
        stock: vec![],
        waste: waste.iter().copied().map(Card).collect(),
        foundations,
        tableau,
    };
    game.stock = (0..52)
        .map(Card)
        .filter(|card| {
            !game.waste.contains(card)
                && !game.tableau.iter().any(|pile| pile.cards.contains(card))
                && card.rank() > game.foundations[card.0 as usize / 13]
        })
        .collect();
    game.validate().unwrap();
    game
}

fn source(column: usize, index: usize) -> Source {
    Source::Tableau { column, index }
}

#[test]
fn draw_three_deals_in_order_recycles_and_undoes_as_one_action() {
    let game = Game::deal(17);
    assert_eq!(game.draw_mode(), DrawMode::Three);
    let mut engine = Engine::new(game.clone());
    engine.execute(Command::Draw).unwrap();
    assert_eq!(engine.game().stock_len(), 21);
    assert_eq!(
        engine.game().waste(),
        &[game.stock[23], game.stock[22], game.stock[21]]
    );
    assert_eq!(
        engine.game().movable_cards(Source::Waste).unwrap(),
        &[game.stock[21]]
    );
    assert!(engine.undo());
    assert_eq!(engine.game(), &game);
    for _ in 0..8 {
        engine.execute(Command::Draw).unwrap();
    }
    assert_eq!(engine.game().stock_len(), 0);
    assert_eq!(engine.game().waste().len(), 24);
    engine.execute(Command::Draw).unwrap();
    assert_eq!(engine.game(), &game);
}

#[test]
fn draw_three_handles_short_final_packets_and_exposes_the_previous_waste_card() {
    let mut game = fixture(&[], &[], [0; 4]);
    game.draw_mode = DrawMode::Three;
    game.waste = game.stock.drain(..50).collect();
    let expected = [game.stock[1], game.stock[0]];
    game.apply(Command::Draw).unwrap();
    assert_eq!(game.stock_len(), 0);
    assert_eq!(&game.waste[50..], &expected);
    game.validate().unwrap();

    let mut game = fixture(&[], &[12, 27, 0], [0; 4]);
    game.draw_mode = DrawMode::Three;
    game.apply(Command::Move {
        from: Source::Waste,
        to: Destination::Foundation(0),
    })
    .unwrap();
    assert_eq!(game.waste().last(), Some(&Card(27)));
    assert_eq!(game.movable_cards(Source::Waste).unwrap(), &[Card(27)]);
    game.validate().unwrap();
}

#[test]
fn urls_preserve_both_modes_and_legacy_links_keep_draw_one_rules() {
    for mode in [DrawMode::One, DrawMode::Three] {
        let game = Game::deal_with_mode(42, mode);
        assert!(game.encode().starts_with("v2:"));
        assert_eq!(Game::decode(&game.encode()), Ok(game));
    }
    let legacy = "v1:AAAAABgAAQACAQMCBAMFBAYFBwYZLSorKAMyLCMnAA8aChgcBSYbDhUdCwwvMBACJB8xExYeBykIFAYSCREzLiEiASANJQQX";
    let mut game = Game::decode(legacy).unwrap();
    assert_eq!(game.draw_mode(), DrawMode::One);
    game.apply(Command::Draw).unwrap();
    assert_eq!(game.stock_len(), 23);
    assert_eq!(game.waste().len(), 1);
    assert_eq!(Game::decode(&game.encode()), Ok(game));

    let encoded = Game::deal(42).encode();
    let mut bytes = URL_SAFE_NO_PAD.decode(&encoded[3..]).unwrap();
    for invalid in [0, 2, 4, 255] {
        bytes[0] = invalid;
        assert_eq!(
            Game::decode(&format!("v2:{}", URL_SAFE_NO_PAD.encode(&bytes))),
            Err(StateError::InvalidState)
        );
    }
}

#[test]
fn deals_have_the_standard_layout_and_complete_inventory() {
    for seed in 0..100 {
        let game = Game::deal(seed);
        game.validate().unwrap();
        assert_eq!(game.stock_len(), 24);
        for (column, pile) in game.tableau().iter().enumerate() {
            assert_eq!(pile.cards().len(), column + 1);
            assert_eq!(pile.hidden(), column);
        }
        assert_eq!(Game::decode(&game.encode()).unwrap(), game);
        assert_eq!(game.encode().len(), 101);
    }
    assert_eq!(Game::deal(42), Game::deal(42));
    assert_ne!(Game::deal(42), Game::deal(43));
}

#[test]
fn recycling_restores_stock_order_and_undo_restores_the_previous_state() {
    let game = Game::deal_with_mode(17, DrawMode::One);
    let stock = game.stock.clone();
    let mut engine = Engine::new(game.clone());
    for _ in 0..24 {
        engine.execute(Command::Draw).unwrap();
    }
    assert_eq!(
        engine.game().waste,
        stock.iter().rev().copied().collect::<Vec<_>>()
    );
    let before_recycle = engine.game().clone();
    engine.execute(Command::Draw).unwrap();
    assert_eq!(engine.game(), &game);
    assert!(engine.undo());
    assert_eq!(engine.game(), &before_recycle);
}

#[test]
fn moving_a_sequence_reveals_the_next_card_and_preserves_order() {
    // 7♣ is hidden under 6♦, 5♣; 7♠ accepts that sequence.
    let game = fixture(&[(&[6, 18, 4], 1), (&[45], 0)], &[], [0; 4]);
    let from = source(0, 1);
    let to = Destination::Tableau(1);
    assert_eq!(game.assess_move(from, to), Ok(()));
    let next = game.updated(Command::Move { from, to }).unwrap();
    assert_eq!(game.tableau[0].hidden, 1);
    assert_eq!(next.tableau[0].cards, [Card(6)]);
    assert_eq!(next.tableau[0].hidden, 0);
    assert_eq!(next.tableau[1].cards, [Card(45), Card(18), Card(4)]);
    next.validate().unwrap();
}

#[test]
fn assessment_reports_specific_reasons_without_mutation() {
    let game = fixture(&[(&[6], 0), (&[4], 0), (&[17], 0)], &[], [0; 4]);
    let unchanged = game.clone();
    assert_eq!(
        game.assess_move(source(1, 0), Destination::Tableau(0)),
        Err(MoveError::SameColor)
    );
    assert_eq!(
        game.assess_move(source(2, 0), Destination::Tableau(0)),
        Err(MoveError::WrongRank)
    );
    assert_eq!(
        game.assess_move(source(0, 0), Destination::Tableau(3)),
        Err(MoveError::KingRequired)
    );
    assert_eq!(
        game.assess_move(source(0, 0), Destination::Tableau(0)),
        Err(MoveError::SameColumn)
    );
    assert_eq!(
        game.assess_move(source(0, 0), Destination::Foundation(1)),
        Err(MoveError::FoundationWrongSuit)
    );
    assert_eq!(
        game.assess_move(source(0, 0), Destination::Foundation(0)),
        Err(MoveError::FoundationWrongRank)
    );
    assert_eq!(
        game.assess_move(source(7, 0), Destination::Tableau(0)),
        Err(MoveError::InvalidSource)
    );
    assert_eq!(
        game.assess_move(source(0, 0), Destination::Tableau(7)),
        Err(MoveError::InvalidDestination)
    );
    assert_eq!(game, unchanged);
}

#[test]
fn only_kings_fill_empty_columns_and_hidden_cards_cannot_move() {
    let game = fixture(&[(&[0, 12], 1)], &[], [0; 4]);
    assert_eq!(
        game.assess_move(source(0, 0), Destination::Tableau(1)),
        Err(MoveError::InvalidSource)
    );
    let next = game
        .updated(Command::Move {
            from: source(0, 1),
            to: Destination::Tableau(1),
        })
        .unwrap();
    assert_eq!(next.tableau[1].cards, [Card(12)]);
    assert_eq!(next.tableau[0].hidden, 0);
}

#[test]
fn foundations_require_single_cards_and_allow_moving_back_to_tableau() {
    let game = fixture(&[(&[1, 13], 0)], &[], [0; 4]);
    assert_eq!(
        game.assess_move(source(0, 0), Destination::Foundation(0)),
        Err(MoveError::FoundationSingleCard)
    );
    let game = fixture(&[(&[14], 0)], &[0], [0; 4]);
    let next = game
        .updated(Command::Move {
            from: Source::Waste,
            to: Destination::Foundation(0),
        })
        .unwrap();
    assert_eq!(next.foundation(0), Some(Card(0)));
    let returned = next
        .updated(Command::Move {
            from: Source::Foundation(0),
            to: Destination::Tableau(0),
        })
        .unwrap();
    assert_eq!(returned.foundations[0], 0);
    assert_eq!(returned.tableau[0].cards, [Card(14), Card(0)]);
    returned.validate().unwrap();
}

#[test]
fn rejected_commands_do_not_change_state_or_create_undo_entries() {
    let game = Game::deal(1);
    let mut engine = Engine::new(game.clone());
    assert!(
        engine
            .execute(Command::Move {
                from: Source::Waste,
                to: Destination::Tableau(0)
            })
            .is_err()
    );
    assert_eq!(engine.game(), &game);
    assert!(!engine.can_undo());
    assert!(!engine.undo());
}

#[test]
fn winning_state_round_trips_and_empty_stock_rejects_draw() {
    let game = fixture(&[], &[], [13; 4]);
    assert!(game.is_won());
    assert_eq!(game.foundation_count(), 52);
    assert_eq!(game.updated(Command::Draw), Err(MoveError::EmptyStock));
    assert_eq!(Game::decode(&format!("#{}", game.encode())), Ok(game));
}

#[test]
fn malformed_links_are_rejected_without_panicking() {
    for input in ["", "v3:abc", "v1:!", "v1:", "#v1:AAAA"] {
        assert!(Game::decode(input).is_err());
    }
    let game = Game::deal(8);
    let original = URL_SAFE_NO_PAD.decode(&game.encode()[3..]).unwrap()[1..].to_vec();
    let encoded = |bytes: &[u8]| format!("v1:{}", URL_SAFE_NO_PAD.encode(bytes));
    for (index, value) in [(0, 14), (4, 255), (7, 255), (20, 52), (20, original[21])] {
        let mut corrupt = original.clone();
        corrupt[index] = value;
        assert!(Game::decode(&encoded(&corrupt)).is_err(), "byte {index}");
    }
    for len in 0..original.len() {
        assert!(Game::decode(&encoded(&original[..len])).is_err());
    }
    for len in 0..100 {
        for byte in [0, 1, 13, 51, 52, 255] {
            assert!(Game::decode(&encoded(&vec![byte; len])).is_err());
        }
    }
}

#[test]
fn legal_play_preserves_inventory_and_serialization() {
    for seed in 0..16 {
        let mut game = Game::deal(seed);
        for step in 0..150 {
            let sources = std::iter::once(Source::Waste)
                .chain((0..4).map(Source::Foundation))
                .chain(game.tableau.iter().enumerate().flat_map(|(column, pile)| {
                    (pile.hidden..pile.cards.len()).map(move |index| source(column, index))
                }));
            let mut moves = vec![];
            for from in sources {
                for to in (0..4)
                    .map(Destination::Foundation)
                    .chain((0..7).map(Destination::Tableau))
                {
                    if game.can_move(from, to) {
                        moves.push(Command::Move { from, to });
                    }
                }
            }
            let command = if moves.is_empty() || step % 3 == 0 {
                Command::Draw
            } else {
                moves[(seed as usize + step * 17) % moves.len()]
            };
            if let Ok(next) = game.updated(command) {
                game = next;
            }
            game.validate().unwrap();
            assert!(game.encode().len() <= 101);
            assert_eq!(Game::decode(&game.encode()).unwrap(), game);
        }
    }
}
