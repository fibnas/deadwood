use std::collections::{BTreeSet, HashSet};

use itertools::Itertools;

use crate::cards::{Card, Rank};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MeldKind {
    Set,
    Run,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meld {
    pub kind: MeldKind,
    pub cards: Vec<Card>,
}

impl Meld {
    pub fn new(kind: MeldKind, cards: Vec<Card>) -> Self {
        let mut cards = cards;
        cards.sort();
        Self { kind, cards }
    }

    pub fn contains(&self, card: Card) -> bool {
        self.cards.contains(&card)
    }

    pub fn can_layoff(&self, card: Card) -> bool {
        match self.kind {
            MeldKind::Set => self.cards.first().map(|c| c.rank) == Some(card.rank),
            MeldKind::Run => {
                if self.cards.is_empty() {
                    return false;
                }
                if self.cards[0].suit != card.suit {
                    return false;
                }
                let mut sorted = self.cards.clone();
                sorted.sort();
                let min_rank = sorted.first().unwrap().rank as i32;
                let max_rank = sorted.last().unwrap().rank as i32;
                let card_rank = card.rank as i32;
                card_rank == min_rank - 1 || card_rank == max_rank + 1
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeldAnalysis {
    pub melds: Vec<Meld>,
    pub deadwood: Vec<Card>,
    pub deadwood_value: u32,
}

impl MeldAnalysis {
    fn new(melds: Vec<Meld>, deadwood: Vec<Card>) -> Self {
        let deadwood_value = deadwood.iter().map(|c| c.rank.value() as u32).sum();
        Self {
            melds,
            deadwood,
            deadwood_value,
        }
    }
}

pub fn analyze_hand(cards: &[Card]) -> MeldAnalysis {
    let mut sorted = cards.to_vec();
    sorted.sort();
    let candidates = generate_candidates(&sorted);
    let mut best = MeldAnalysis::new(vec![], sorted.clone());
    search_candidates(&sorted, &candidates, &mut vec![], &mut vec![], &mut best);
    best
}

fn generate_candidates(cards: &[Card]) -> Vec<Meld> {
    let mut candidates = Vec::new();
    candidates.extend(generate_sets(cards));
    candidates.extend(generate_runs(cards));

    dedup_melds(candidates)
}

fn generate_sets(cards: &[Card]) -> Vec<Meld> {
    cards
        .iter()
        .cloned()
        .into_group_map_by(|card| card.rank)
        .into_iter()
        .flat_map(|(_, group)| {
            (3..=group.len()).flat_map(move |size| {
                group
                    .iter()
                    .cloned()
                    .combinations(size)
                    .map(|combo| Meld::new(MeldKind::Set, combo))
                    .collect::<Vec<_>>()
            })
        })
        .collect()
}

fn generate_runs(cards: &[Card]) -> Vec<Meld> {
    let mut runs = Vec::new();
    let by_suit = cards.iter().cloned().into_group_map_by(|card| card.suit);

    for (_suit, mut suited_cards) in by_suit {
        suited_cards.sort();
        let unique_cards: Vec<Card> = suited_cards
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        let mut start = 0;
        while start < unique_cards.len() {
            let mut end = start + 1;
            while end < unique_cards.len()
                && ranks_are_consecutive(unique_cards[end - 1].rank, unique_cards[end].rank)
            {
                end += 1;
            }

            for len in 3..=(end - start) {
                for window_start in start..=(end - len) {
                    let window = unique_cards[window_start..window_start + len].to_vec();
                    runs.push(Meld::new(MeldKind::Run, window));
                }
            }

            start = end;
        }
    }

    runs
}

fn ranks_are_consecutive(prev: Rank, next: Rank) -> bool {
    (prev as i32) + 1 == (next as i32)
}

fn dedup_melds(melds: Vec<Meld>) -> Vec<Meld> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for meld in melds {
        if seen.insert(meld.cards.clone()) {
            result.push(meld);
        }
    }
    result
}

fn search_candidates(
    remaining: &[Card],
    candidates: &[Meld],
    current_melds: &mut Vec<Meld>,
    deadwood: &mut Vec<Card>,
    best: &mut MeldAnalysis,
) {
    if remaining.is_empty() {
        let analysis = MeldAnalysis::new(current_melds.clone(), deadwood.clone());
        if analysis.deadwood_value < best.deadwood_value
            || (analysis.deadwood_value == best.deadwood_value
                && analysis.melds.len() > best.melds.len())
        {
            *best = analysis;
        }
        return;
    }

    let card = remaining[0];
    let rest = &remaining[1..];

    deadwood.push(card);
    search_candidates(rest, candidates, current_melds, deadwood, best);
    deadwood.pop();

    for meld in candidates.iter().filter(|m| m.contains(card)) {
        if meld.cards.iter().all(|c| remaining.contains(c))
            && meld.cards.iter().all(|c| !deadwood.contains(c))
            && meld
                .cards
                .iter()
                .all(|c| current_melds.iter().all(|m| !m.contains(*c)))
        {
            current_melds.push(meld.clone());
            let mut reduced: Vec<Card> = remaining
                .iter()
                .filter(|c| !meld.cards.contains(c))
                .cloned()
                .collect();
            reduced.sort();
            search_candidates(&reduced, candidates, current_melds, deadwood, best);
            current_melds.pop();
        }
    }
}

pub fn layoff_cards(deadwood: &[Card], knocker_melds: &[Meld]) -> (Vec<Card>, Vec<Card>) {
    let mut remaining = Vec::new();
    let mut laid_off = Vec::new();
    let mut expanded_melds = knocker_melds.to_vec();

    'outer: for card in deadwood {
        for meld in expanded_melds.iter_mut() {
            if meld.can_layoff(*card) {
                laid_off.push(*card);
                meld.cards.push(*card);
                meld.cards.sort();
                continue 'outer;
            }
        }
        remaining.push(*card);
    }

    (remaining, laid_off)
}
