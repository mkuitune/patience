//! Shared card artwork and ordered card compositions in a 750 by 1050 document.

use graphics2d::{
    AssetId, Bounds, Document, Handedness, Instance, Paint, Placement, SimplePlacement, SvgAsset,
    Vec2, View,
};

pub const CARD_BOUNDS: Bounds = Bounds::from_size(750.0, 1050.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    pub fn asset(self) -> AssetId {
        AssetId(1 + self as usize)
    }
    pub fn color(self) -> [u8; 4] {
        match self {
            Self::Diamonds | Self::Hearts => [204, 0, 0, 255],
            _ => [0, 0, 0, 255],
        }
    }
}

macro_rules! asset {
    ($file:literal, $width:literal, $height:literal) => {
        SvgAsset {
            name: $file,
            svg: include_str!(concat!("../", $file, ".svg")),
            bounds: Bounds::from_size($width, $height),
        }
    };
}

pub static ASSETS: &[SvgAsset] = &[
    asset!("CARD", 750.0, 1050.0),
    asset!("CLUB", 182.0, 195.0),
    asset!("DIAMOND", 142.0, 196.0),
    asset!("HEART", 174.0, 195.0),
    asset!("SPADE", 157.0, 195.0),
    asset!("A", 67.0, 106.0),
    asset!("2", 68.0, 106.0),
    asset!("3", 68.0, 106.0),
    asset!("4", 68.0, 106.0),
    asset!("5", 63.0, 106.0),
    asset!("6", 63.0, 106.0),
    asset!("7", 68.0, 106.0),
    asset!("8", 62.0, 106.0),
    asset!("9", 63.0, 106.0),
    asset!("10", 68.0, 106.0),
    asset!("J", 68.0, 106.0),
    asset!("Q", 68.0, 106.0),
    asset!("K", 68.0, 106.0),
    asset!("ROYAL_FRAME", 545.0, 856.0),
    asset!("JACK_CLUB", 532.0, 849.0),
    asset!("QUEEN_CLUB", 532.0, 849.0),
    asset!("KING_CLUB", 532.0, 860.0),
    asset!("JACK_DIAMOND", 543.0, 860.0),
    asset!("QUEEEN_DIAMOND", 543.0, 860.0),
    asset!("KING_DIAMOND", 532.0, 860.0),
    asset!("JACK_HEART", 532.0, 849.0),
    asset!("QUEEN_HEART", 531.0, 850.0),
    asset!("KING_HEART", 532.0, 860.0),
    asset!("JACK_SPADE", 532.0, 849.0),
    asset!("QUEEN_SPADE", 531.0, 849.0),
    asset!("KING_SPADE", 532.0, 860.0),
    asset!("JOKER_HAT", 225.0, 195.0),
    asset!("JOKER_TEXT", 69.0, 396.0),
];

pub fn rank_asset(rank: u8) -> Option<AssetId> {
    (1..=13)
        .contains(&rank)
        .then_some(AssetId(4 + rank as usize))
}

pub fn face(rank: u8, suit: Suit) -> Option<Document> {
    if !(1..=13).contains(&rank) {
        return None;
    }
    let mut document = Document::default();
    document.instances.push(Instance {
        asset: AssetId(0),
        placement: Placement::Simple(SimplePlacement::IDENTITY),
        paint: Paint::Original,
    });
    let ink = Paint::Solid(suit.color());
    let rank_asset = AssetId(4 + rank as usize);
    for inverted in [false, true] {
        let center = |point| {
            if inverted {
                CARD_BOUNDS.size() - point
            } else {
                point
            }
        };
        add_centered(
            &mut document,
            rank_asset,
            center(Vec2::new(58.0, 75.0)),
            106.0,
            inverted,
            ink,
        );
        add_centered(
            &mut document,
            suit.asset(),
            center(Vec2::new(58.0, 177.0)),
            70.0,
            inverted,
            ink,
        );
    }
    if rank >= 11 {
        let court = AssetId(19 + suit as usize * 3 + (rank - 11) as usize);
        add_centered(
            &mut document,
            court,
            Vec2::new(375.0, 525.0),
            845.0,
            false,
            Paint::Original,
        );
        add_centered(
            &mut document,
            suit.asset(),
            Vec2::new(190.0, 195.0),
            185.0,
            false,
            ink,
        );
        add_centered(
            &mut document,
            suit.asset(),
            Vec2::new(560.0, 855.0),
            185.0,
            true,
            ink,
        );
        add_centered(
            &mut document,
            AssetId(18),
            Vec2::new(375.0, 525.0),
            856.0,
            false,
            Paint::Original,
        );
    } else if rank == 1 {
        add_centered(
            &mut document,
            suit.asset(),
            Vec2::new(375.0, 525.0),
            195.0,
            false,
            ink,
        );
    } else {
        for &(x, y) in pip_layout(rank) {
            add_centered(
                &mut document,
                suit.asset(),
                Vec2::new(375.0 + x * 155.0, 525.0 + y * 385.0),
                195.0,
                y > 0.0,
                ink,
            );
        }
    }
    Some(document)
}

pub fn view(document: &Document) -> View<'_> {
    View::new(document, CARD_BOUNDS)
}

pub fn back() -> Document {
    let mut document = Document::default();
    add_centered(
        &mut document,
        AssetId(0),
        CARD_BOUNDS.center(),
        1050.0,
        false,
        Paint::Original,
    );
    add_centered(
        &mut document,
        AssetId(0),
        CARD_BOUNDS.center(),
        1008.0,
        false,
        Paint::Solid([35, 65, 125, 255]),
    );
    add_centered(
        &mut document,
        Suit::Spades.asset(),
        CARD_BOUNDS.center(),
        260.0,
        false,
        Paint::Solid([255, 255, 255, 100]),
    );
    document
}

fn add_centered(
    document: &mut Document,
    asset: AssetId,
    center: Vec2,
    height: f32,
    inverted: bool,
    paint: Paint,
) {
    let scale = height / ASSETS[asset.0].bounds.size().y;
    let x_direction = if inverted { -Vec2::X } else { Vec2::X };
    let origin = center - ASSETS[asset.0].bounds.center() * scale * x_direction.x;
    let placement = SimplePlacement::new(origin, x_direction, scale, Handedness::Positive)
        .expect("finite card layout");
    document.instances.push(Instance {
        asset,
        placement: Placement::Simple(placement),
        paint,
    });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_of_spades_is_seven_ordered_instances_of_three_assets() {
        let card = face(2, Suit::Spades).unwrap();
        assert_eq!(card.instances.len(), 7);
        assert_eq!(card.instances[0].asset, AssetId(0));
        assert_eq!(
            card.instances
                .iter()
                .filter(|instance| instance.asset == Suit::Spades.asset())
                .count(),
            4
        );
        assert_eq!(
            card.instances
                .iter()
                .filter(|instance| instance.asset == rank_asset(2).unwrap())
                .count(),
            2
        );
        assert_eq!(card.instances[1].placement.matrix().x_axis.x, 1.0);
        assert_eq!(card.instances[3].placement.matrix().x_axis.x, -1.0);
    }

    #[test]
    fn every_face_has_the_expected_pips_unique_courts_and_in_bounds_instances() {
        let mut courts = std::collections::HashSet::new();
        for suit in [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades] {
            for rank in 1..=13 {
                let card = face(rank, suit).unwrap();
                let expected_suits = if rank < 11 { 2 + rank as usize } else { 4 };
                assert_eq!(
                    card.instances
                        .iter()
                        .filter(|instance| instance.asset == suit.asset())
                        .count(),
                    expected_suits
                );
                if rank >= 11 {
                    assert!(courts.insert(card.instances[5].asset));
                }
                for instance in &card.instances {
                    let asset = &ASSETS[instance.asset.0];
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
                            .transform_point2(ASSETS[pip.asset.0].bounds.center());
                        assert!(centers.insert((point.x.round() as i32, point.y.round() as i32)));
                    }
                }
            }
        }
        assert_eq!(courts.len(), 12);
        assert!(face(0, Suit::Clubs).is_none());
        assert!(face(14, Suit::Clubs).is_none());
    }
}
