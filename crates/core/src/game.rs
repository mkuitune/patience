use rand::{SeedableRng, seq::SliceRandom};
use rand_chacha::ChaCha8Rng;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    pub const ALL: [Self; 4] = [Self::Clubs, Self::Diamonds, Self::Hearts, Self::Spades];

    pub fn symbol(self) -> &'static str {
        match self {
            Self::Clubs => "♣",
            Self::Diamonds => "♦",
            Self::Hearts => "♥",
            Self::Spades => "♠",
        }
    }

    pub fn is_red(self) -> bool {
        matches!(self, Self::Diamonds | Self::Hearts)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Card(pub(crate) u8);

impl Card {
    pub fn rank(self) -> u8 {
        self.0 % 13 + 1
    }

    pub fn suit(self) -> Suit {
        Suit::ALL[self.0 as usize / 13]
    }

    pub fn rank_label(self) -> String {
        match self.rank() {
            1 => "A".into(),
            11 => "J".into(),
            12 => "Q".into(),
            13 => "K".into(),
            n => n.to_string(),
        }
    }

    pub(crate) fn follows(self, lower: Self) -> bool {
        lower.rank() == self.rank() + 1 && lower.suit().is_red() != self.suit().is_red()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tableau {
    pub(crate) cards: Vec<Card>,
    pub(crate) hidden: usize,
}

impl Tableau {
    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    pub fn hidden(&self) -> usize {
        self.hidden
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub(crate) stock: Vec<Card>,
    pub(crate) waste: Vec<Card>,
    pub(crate) foundations: [u8; 4],
    pub(crate) foundation_suits: [u8; 4],
    pub(crate) tableau: [Tableau; 7],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Waste,
    Foundation(usize),
    Tableau { column: usize, index: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    Foundation(usize),
    Tableau(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Draw,
    Move { from: Source, to: Destination },
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum MoveError {
    #[error("There is no playable card here.")]
    InvalidSource,
    #[error("That destination does not exist.")]
    InvalidDestination,
    #[error("Choose a different column.")]
    SameColumn,
    #[error("Only a king can fill an empty column.")]
    KingRequired,
    #[error("Tableau cards must alternate red and black.")]
    SameColor,
    #[error("Place the card on a card exactly one rank higher.")]
    WrongRank,
    #[error("Move only one card at a time to a foundation.")]
    FoundationSingleCard,
    #[error("Choose the foundation matching the card's suit.")]
    FoundationWrongSuit,
    #[error("Build foundations from ace to king, without skipping ranks.")]
    FoundationWrongRank,
    #[error("The stock and waste are empty.")]
    EmptyStock,
}

impl Game {
    pub fn deal(seed: u64) -> Self {
        let mut deck: Vec<_> = (0..52).map(Card).collect();
        deck.shuffle(&mut ChaCha8Rng::seed_from_u64(seed));
        let mut tableau: [Tableau; 7] = std::array::from_fn(|_| Tableau::default());
        for row in 0..7 {
            for pile in tableau.iter_mut().skip(row) {
                pile.cards
                    .push(deck.pop().expect("a deal uses 28 of 52 cards"));
            }
        }
        for (column, pile) in tableau.iter_mut().enumerate() {
            pile.hidden = column;
        }
        Self {
            stock: deck,
            waste: vec![],
            foundations: [0; 4],
            foundation_suits: [0, 1, 2, 3],
            tableau,
        }
    }

    pub fn stock_len(&self) -> usize {
        self.stock.len()
    }

    pub fn waste(&self) -> &[Card] {
        &self.waste
    }

    pub fn tableau(&self) -> &[Tableau; 7] {
        &self.tableau
    }

    pub fn foundation(&self, slot: usize) -> Option<Card> {
        let suit = *self.foundation_suits.get(slot)?;
        let rank = self.foundations[suit as usize];
        (rank > 0).then(|| Card(suit * 13 + rank - 1))
    }

    pub fn foundation_count(&self) -> u8 {
        self.foundations.iter().sum()
    }

    pub fn is_won(&self) -> bool {
        self.foundations == [13; 4]
    }

    pub fn movable_cards(&self, source: Source) -> Option<&[Card]> {
        match source {
            Source::Waste => self.waste.last().map(std::slice::from_ref),
            Source::Foundation(_) => None,
            Source::Tableau { column, index } => {
                let pile = self.tableau.get(column)?;
                if index < pile.hidden || index >= pile.cards.len() {
                    return None;
                }
                let cards = &pile.cards[index..];
                cards
                    .windows(2)
                    .all(|pair| pair[1].follows(pair[0]))
                    .then_some(cards)
            }
        }
    }

    fn moving(&self, source: Source) -> Option<Vec<Card>> {
        if let Source::Foundation(suit) = source {
            self.foundation(suit).map(|card| vec![card])
        } else {
            self.movable_cards(source).map(<[Card]>::to_vec)
        }
    }

    pub fn can_move(&self, from: Source, to: Destination) -> bool {
        self.assess_move(from, to).is_ok()
    }

    /// Lists all currently legal card moves and the stock action, when available.
    pub fn legal_moves(&self) -> Vec<Command> {
        let mut sources = vec![Source::Waste];
        sources.extend((0..4).map(Source::Foundation));
        for (column, pile) in self.tableau.iter().enumerate() {
            sources.extend(
                (pile.hidden..pile.cards.len()).map(|index| Source::Tableau { column, index }),
            );
        }
        let mut moves = Vec::new();
        for from in sources {
            for to in (0..4)
                .map(Destination::Foundation)
                .chain((0..7).map(Destination::Tableau))
            {
                if self.can_move(from, to) {
                    moves.push(Command::Move { from, to });
                }
            }
        }
        if !self.stock.is_empty() || !self.waste.is_empty() {
            moves.push(Command::Draw);
        }
        moves
    }

    /// Omits redundant relocations and guides aces to the leftmost empty foundation.
    pub fn suggested_moves(&self) -> Vec<Command> {
        self.legal_moves()
            .into_iter()
            .filter(|command| match command {
                Command::Move {
                    from: Source::Foundation(slot),
                    ..
                } if self.foundation(*slot).is_some_and(|card| card.rank() == 1) => false,
                Command::Move {
                    from: Source::Tableau { index: 0, .. },
                    to: Destination::Tableau(column),
                } => !self.tableau[*column].cards.is_empty(),
                Command::Move {
                    to: Destination::Foundation(slot),
                    ..
                } if self.foundation(*slot).is_none() => {
                    Some(*slot) == (0..4).find(|&slot| self.foundation(slot).is_none())
                }
                _ => true,
            })
            .collect()
    }

    /// Assesses a proposed move without changing the board.
    pub fn assess_move(&self, from: Source, to: Destination) -> Result<(), MoveError> {
        let cards = self.moving(from).ok_or(MoveError::InvalidSource)?;
        let card = cards[0];
        match to {
            Destination::Foundation(slot) => {
                if slot >= 4 {
                    return Err(MoveError::InvalidDestination);
                }
                if cards.len() != 1 {
                    return Err(MoveError::FoundationSingleCard);
                }
                let rank = if let Some(top) = self.foundation(slot) {
                    if card.suit() != top.suit() {
                        return Err(MoveError::FoundationWrongSuit);
                    }
                    top.rank()
                } else {
                    0
                };
                if card.rank() != rank + 1 {
                    return Err(MoveError::FoundationWrongRank);
                }
            }
            Destination::Tableau(column) => {
                if matches!(from, Source::Tableau { column: source, .. } if source == column) {
                    return Err(MoveError::SameColumn);
                }
                let pile = self
                    .tableau
                    .get(column)
                    .ok_or(MoveError::InvalidDestination)?;
                if let Some(&lower) = pile.cards.last() {
                    if card.suit().is_red() == lower.suit().is_red() {
                        return Err(MoveError::SameColor);
                    }
                    if card.rank() + 1 != lower.rank() {
                        return Err(MoveError::WrongRank);
                    }
                } else if card.rank() != 13 {
                    return Err(MoveError::KingRequired);
                }
            }
        }
        Ok(())
    }

    /// Produces the next state, leaving this state unchanged on success or failure.
    pub fn updated(&self, command: Command) -> Result<Self, MoveError> {
        let mut next = self.clone();
        next.apply(command)?;
        Ok(next)
    }

    pub fn apply(&mut self, command: Command) -> Result<(), MoveError> {
        match command {
            Command::Draw => {
                if !self.stock.is_empty() {
                    for _ in 0..3.min(self.stock.len()) {
                        self.waste.push(
                            self.stock
                                .pop()
                                .expect("draw count is bounded by stock length"),
                        );
                    }
                } else if !self.waste.is_empty() {
                    // Reverse to preserve the original draw order on each pass.
                    self.stock = self.waste.drain(..).rev().collect();
                } else {
                    return Err(MoveError::EmptyStock);
                }
            }
            Command::Move { from, to } => {
                self.assess_move(from, to)?;
                let cards = self.moving(from).ok_or(MoveError::InvalidSource)?;
                match from {
                    Source::Waste => {
                        self.waste.pop();
                    }
                    Source::Foundation(slot) => {
                        self.foundations[self.foundation_suits[slot] as usize] -= 1;
                    }
                    Source::Tableau { column, index } => {
                        let pile = &mut self.tableau[column];
                        pile.cards.truncate(index);
                        pile.hidden = pile.hidden.min(pile.cards.len().saturating_sub(1));
                    }
                }
                match to {
                    Destination::Foundation(slot) => {
                        let suit = cards[0].0 as usize / 13;
                        let previous_slot = self
                            .foundation_suits
                            .iter()
                            .position(|&s| s as usize == suit)
                            .unwrap();
                        self.foundation_suits.swap(slot, previous_slot);
                        self.foundations[suit] += 1;
                    }
                    Destination::Tableau(column) => self.tableau[column].cards.extend(cards),
                }
            }
        }
        Ok(())
    }
}
