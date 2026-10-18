//! Shared SVG artwork and data-defined card compositions.

use graphics2d::{AssetId, Document, View};
use graphics2d_format::{Context, DeckBindings, LoadError};

pub const MANIFEST: &str = include_str!("../library.json");
pub const DECK: &str = include_str!("../deck.json");
include!(concat!(env!("OUT_DIR"), "/bundled.rs"));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

pub struct Deck {
    pub context: Context,
    pub bindings: DeckBindings,
}

impl Deck {
    pub fn bundled() -> Result<Self, LoadError> {
        Self::load(MANIFEST, DECK, |path| {
            BUNDLED
                .iter()
                .find(|(p, _)| *p == path)
                .map(|(_, text)| (*text).to_owned())
                .ok_or_else(|| LoadError(format!("missing bundled resource: {path}")))
        })
    }

    pub fn load(
        manifest: &str,
        deck: &str,
        read: impl FnMut(&str) -> Result<String, LoadError>,
    ) -> Result<Self, LoadError> {
        let context = Context::load(manifest, read)?;
        let bindings = DeckBindings::load(deck, &context)?;
        Ok(Self { context, bindings })
    }

    pub fn face(&self, rank: u8, suit: Suit) -> Option<&Document> {
        (1..=13).contains(&rank).then(|| {
            self.context
                .document(self.bindings.faces[suit as usize * 13 + rank as usize - 1])
        })
    }
    pub fn back(&self) -> &Document {
        self.context.document(self.bindings.back)
    }
    pub fn suit_asset(&self, suit: Suit) -> AssetId {
        self.bindings.suits[suit as usize]
    }
    pub fn rank_asset(&self, rank: u8) -> Option<AssetId> {
        (1..=13)
            .contains(&rank)
            .then(|| self.bindings.ranks[rank as usize - 1])
    }
    pub fn face_view(&self, rank: u8, suit: Suit) -> Option<View<'_>> {
        (1..=13).contains(&rank).then(|| {
            self.context
                .view(self.bindings.faces[suit as usize * 13 + rank as usize - 1])
        })
    }
    pub fn back_view(&self) -> View<'_> {
        self.context.view(self.bindings.back)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_of_spades_is_seven_ordered_instances_of_three_assets() {
        let deck = Deck::bundled().unwrap();
        let card = deck.face(2, Suit::Spades).unwrap();
        assert_eq!(card.instances.len(), 7);
        assert!(card.instances[0].style.is_some());
        assert_eq!(
            card.instances
                .iter()
                .filter(|instance| instance.asset == deck.suit_asset(Suit::Spades))
                .count(),
            4
        );
        assert_eq!(
            card.instances
                .iter()
                .filter(|instance| instance.asset == deck.rank_asset(2).unwrap())
                .count(),
            2
        );
        assert_eq!(card.instances[1].placement.matrix().x_axis.x, 1.0);
        assert_eq!(card.instances[3].placement.matrix().x_axis.x, -1.0);
    }

    #[test]
    fn every_face_has_the_expected_pips_unique_courts_and_in_bounds_instances() {
        let deck = Deck::bundled().unwrap();
        let assets = &deck.context.resources.assets;
        let mut courts = std::collections::HashSet::new();
        for suit in [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades] {
            for rank in 1..=13 {
                let card = deck.face(rank, suit).unwrap();
                let expected_suits = if rank < 11 { 2 + rank as usize } else { 4 };
                assert_eq!(
                    card.instances
                        .iter()
                        .filter(|instance| instance.asset == deck.suit_asset(suit))
                        .count(),
                    expected_suits
                );
                if rank >= 11 {
                    assert!(courts.insert(card.instances[5].asset));
                    let frame = &card.instances[8];
                    let frame_matrix = frame.placement.matrix();
                    let frame_min =
                        frame_matrix.transform_point2(assets[frame.asset.0].bounds.min());
                    let frame_max =
                        frame_matrix.transform_point2(assets[frame.asset.0].bounds.max());
                    for symbol in &card.instances[6..8] {
                        for point in assets[symbol.asset.0].bounds.corners() {
                            let point = symbol.placement.matrix().transform_point2(point);
                            assert!(
                                (point - frame_min).min_element() >= 20.0
                                    && (frame_max - point).min_element() >= 20.0,
                                "inner {suit:?} symbol touches the court frame"
                            );
                        }
                    }
                }
                for instance in &card.instances {
                    let asset = &assets[instance.asset.0];
                    for point in asset.bounds.corners() {
                        let point = instance.placement.matrix().transform_point2(point);
                        assert!(
                            point.x >= -0.01
                                && point.y >= -0.01
                                && point.x <= 750.01
                                && point.y <= 1050.01,
                            "{}: {point:?}",
                            asset.name
                        );
                    }
                }
                if (2..=10).contains(&rank) {
                    let mut centers = std::collections::HashSet::new();
                    for pip in &card.instances[5..] {
                        let point = pip
                            .placement
                            .matrix()
                            .transform_point2(assets[pip.asset.0].bounds.center());
                        assert!(centers.insert((point.x.round() as i32, point.y.round() as i32)));
                    }
                }
            }
        }
        assert_eq!(courts.len(), 12);
        assert!(deck.face(0, Suit::Clubs).is_none());
        assert!(deck.face(14, Suit::Clubs).is_none());
    }
}
