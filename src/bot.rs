use anyhow::Result;
use rand::seq::SliceRandom;

use crate::{
    game::{ActionOutcome, DrawSource, Game, PlayerId, TurnPhase},
    meld::analyze_hand,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotDifficulty {
    Easy,
    Challenging,
}

impl BotDifficulty {
    fn knock_threshold(self) -> u32 {
        match self {
            BotDifficulty::Easy => 6,
            BotDifficulty::Challenging => 10,
        }
    }
}

pub fn take_turn(game: &mut Game, difficulty: BotDifficulty) -> Result<ActionOutcome> {
    loop {
        match game.phase {
            TurnPhase::AwaitDraw if game.current_player == PlayerId::Bot => {
                let source = choose_draw_source(game, difficulty);
                let outcome = game.draw(PlayerId::Bot, source)?;
                match outcome {
                    ActionOutcome::Continue => continue,
                    ActionOutcome::RoundEnded => return Ok(ActionOutcome::RoundEnded),
                }
            }
            TurnPhase::AwaitDiscard if game.current_player == PlayerId::Bot => {
                let (index, knock) = choose_discard(game, difficulty);
                let outcome = game.discard(PlayerId::Bot, index, knock)?;
                return Ok(outcome);
            }
            _ => return Ok(ActionOutcome::Continue),
        }
    }
}

fn choose_draw_source(game: &Game, _difficulty: BotDifficulty) -> DrawSource {
    if game.discard.is_empty() {
        return DrawSource::Stock;
    }

    let mut hypothetical = game.bot.hand.clone();
    let top_discard = *game.discard.last().unwrap();
    let current_score = analyze_hand(&hypothetical).deadwood_value;
    hypothetical.push(top_discard);
    let score_with_discard = analyze_hand(&hypothetical).deadwood_value;

    if score_with_discard <= current_score {
        DrawSource::Discard
    } else {
        DrawSource::Stock
    }
}

fn choose_discard(game: &Game, difficulty: BotDifficulty) -> (usize, bool) {
    let mut best_index = 0;
    let mut best_deadwood = u32::MAX;
    let mut best_card_value = 0;

    for (idx, _card) in game.bot.hand.iter().enumerate() {
        let mut hypothetical = game.bot.hand.clone();
        let removed = hypothetical.remove(idx);
        let analysis = analyze_hand(&hypothetical);
        let deadwood_with_discard = analysis.deadwood_value;

        if deadwood_with_discard < best_deadwood
            || (deadwood_with_discard == best_deadwood && removed.rank.value() > best_card_value)
        {
            best_deadwood = deadwood_with_discard;
            best_index = idx;
            best_card_value = removed.rank.value();
        }
    }

    let mut knock = false;
    if best_deadwood <= difficulty.knock_threshold() {
        let hypothetical = {
            let mut hand = game.bot.hand.clone();
            hand.remove(best_index);
            hand
        };
        let analysis = analyze_hand(&hypothetical);
        if analysis.deadwood_value <= 10 {
            knock = true;
        }
    }

    if difficulty == BotDifficulty::Easy && rand::random::<f32>() < 0.2 {
        let mut rng = rand::thread_rng();
        let random_index = (0..game.bot.hand.len())
            .collect::<Vec<_>>()
            .choose(&mut rng)
            .copied();
        if let Some(idx) = random_index {
            return (idx, false);
        }
    }

    (best_index, knock)
}
