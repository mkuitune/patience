use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use thiserror::Error;

use crate::{Card, Game, Tableau};

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum StateError {
    #[error("This link uses an unsupported game version.")]
    UnsupportedVersion,
    #[error("This game link is damaged or contains an invalid deal.")]
    InvalidState,
}

impl Game {
    /// Encodes the complete board as a versioned URL fragment, without the `#`.
    pub fn encode(&self) -> String {
        let mut bytes = Vec::with_capacity(72);
        bytes.extend(self.foundations);
        bytes.push(self.stock.len() as u8);
        bytes.push(self.waste.len() as u8);
        for pile in &self.tableau {
            bytes.push(pile.cards.len() as u8);
            bytes.push(pile.hidden as u8);
        }
        bytes.extend(self.stock.iter().chain(&self.waste).map(|card| card.0));
        for pile in &self.tableau {
            bytes.extend(pile.cards.iter().map(|card| card.0));
        }
        format!("v1:{}", URL_SAFE_NO_PAD.encode(bytes))
    }

    /// Restores a board after checking its card inventory and pile invariants.
    pub fn decode(fragment: &str) -> Result<Self, StateError> {
        let fragment = fragment.strip_prefix('#').unwrap_or(fragment);
        let payload = fragment
            .strip_prefix("v1:")
            .ok_or(StateError::UnsupportedVersion)?;
        if payload.len() > 96 {
            return Err(StateError::InvalidState);
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| StateError::InvalidState)?;
        if bytes.len() < 20 {
            return Err(StateError::InvalidState);
        }
        let foundations: [u8; 4] = bytes[..4].try_into().unwrap();
        if foundations.iter().any(|&rank| rank > 13) {
            return Err(StateError::InvalidState);
        }
        let mut cursor = 20;
        let mut take = |len: usize| -> Result<Vec<Card>, StateError> {
            let end = cursor + len;
            let data = bytes.get(cursor..end).ok_or(StateError::InvalidState)?;
            cursor = end;
            Ok(data.iter().copied().map(Card).collect())
        };
        let stock = take(bytes[4] as usize)?;
        let waste = take(bytes[5] as usize)?;
        let mut tableau: [Tableau; 7] = std::array::from_fn(|_| Tableau::default());
        for (column, pile) in tableau.iter_mut().enumerate() {
            pile.cards = take(bytes[6 + column * 2] as usize)?;
            pile.hidden = bytes[7 + column * 2] as usize;
        }
        if cursor != bytes.len() {
            return Err(StateError::InvalidState);
        }
        let game = Self {
            stock,
            waste,
            foundations,
            tableau,
        };
        game.validate()?;
        Ok(game)
    }

    pub(crate) fn validate(&self) -> Result<(), StateError> {
        let mut seen = [false; 52];
        let mut mark = |card: Card| -> Result<(), StateError> {
            let slot = seen
                .get_mut(card.0 as usize)
                .ok_or(StateError::InvalidState)?;
            if std::mem::replace(slot, true) {
                return Err(StateError::InvalidState);
            }
            Ok(())
        };
        for (suit, &rank) in self.foundations.iter().enumerate() {
            if rank > 13 {
                return Err(StateError::InvalidState);
            }
            for offset in 0..rank {
                mark(Card(suit as u8 * 13 + offset))?;
            }
        }
        for &card in self.stock.iter().chain(&self.waste) {
            mark(card)?;
        }
        for pile in &self.tableau {
            for &card in &pile.cards {
                mark(card)?;
            }
            if (pile.cards.is_empty() && pile.hidden != 0)
                || (!pile.cards.is_empty() && pile.hidden >= pile.cards.len())
                || !pile.cards[pile.hidden..]
                    .windows(2)
                    .all(|pair| pair[1].follows(pair[0]))
            {
                return Err(StateError::InvalidState);
            }
        }
        if !seen.into_iter().all(|present| present) {
            return Err(StateError::InvalidState);
        }
        Ok(())
    }
}
