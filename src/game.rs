use std::fmt::{Display, Formatter};

use anyhow::{anyhow, Result};
use rand::{seq::SliceRandom, thread_rng};
use serde::{Deserialize, Serialize};

use crate::{
    cards::{Card, Rank, Suit, HAND_SIZE},
    meld::{analyze_hand, layoff_cards},
};

const BIG_GIN_BONUS: i32 = 31;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerId {
    Human,
    Bot,
}

impl PlayerId {
    pub fn other(self) -> Self {
        match self {
            PlayerId::Human => PlayerId::Bot,
            PlayerId::Bot => PlayerId::Human,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawSource {
    Stock,
    Discard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnPhase {
    AwaitDraw,
    AwaitDiscard,
    RoundOver,
}

#[derive(Debug, Clone)]
pub struct Player {
    pub hand: Vec<Card>,
}

impl Player {
    pub fn new() -> Self {
        Self { hand: Vec::new() }
    }

    pub fn sort_hand(&mut self) {
        self.hand.sort();
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Scoreboard {
    pub human: i32,
    pub bot: i32,
    pub rounds_played: u32,
    pub human_hands_won: u32,
    pub bot_hands_won: u32,
    pub draws: u32,
}

#[derive(Debug, Clone)]
pub enum RoundEndReason {
    Knock {
        knocker: PlayerId,
        knocker_deadwood: u32,
        opponent_deadwood: u32,
        laid_off: Vec<Card>,
        gin: bool,
        undercut: bool,
    },
    BigGin {
        player: PlayerId,
        opponent_deadwood: u32,
        bonus: i32,
    },
    StockDepleted,
}

#[derive(Debug, Clone)]
pub struct RoundResult {
    pub winner: Option<PlayerId>,
    pub points_awarded: i32,
    pub reason: RoundEndReason,
    pub human_hand: Vec<Card>,
    pub bot_hand: Vec<Card>,
}

impl Display for RoundResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = |id: PlayerId| match id {
            PlayerId::Human => "You",
            PlayerId::Bot => "Bot",
        };

        match &self.reason {
            RoundEndReason::Knock {
                knocker,
                knocker_deadwood,
                opponent_deadwood,
                laid_off,
                gin,
                undercut,
            } => {
                let winner_name = self.winner.map(name).unwrap_or("Nobody");
                let knocker_name = name(*knocker);
                let layoff_label = describe_layoffs(laid_off);

                if *gin {
                    write!(
                        f,
                        "{knocker_name} got Gin and scores {} points (opponent deadwood {}, laid off {}).",
                        self.points_awarded,
                        opponent_deadwood,
                        layoff_label
                    )
                } else if *undercut {
                    write!(
                        f,
                        "Undercut! {winner_name} scores {} points (knocker deadwood {}, opponent deadwood {}, laid off {}).",
                        self.points_awarded, knocker_deadwood, opponent_deadwood, layoff_label
                    )
                } else {
                    write!(
                        f,
                        "{knocker_name} knocked and scores {} points (deadwood diff {} vs {}, laid off {}).",
                        self.points_awarded,
                        opponent_deadwood,
                        knocker_deadwood,
                        layoff_label
                    )
                }
            }
            RoundEndReason::BigGin {
                player,
                opponent_deadwood,
                bonus,
            } => write!(
                f,
                "{} hit Big Gin and scores {} points (opponent deadwood {}, bonus {}).",
                name(*player),
                self.points_awarded,
                opponent_deadwood,
                bonus
            ),
            RoundEndReason::StockDepleted => write!(f, "Round ended in a draw: stock depleted."),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Game {
    pub human: Player,
    pub bot: Player,
    pub stock: Vec<Card>,
    pub discard: Vec<Card>,
    pub dealer: PlayerId,
    pub current_player: PlayerId,
    pub phase: TurnPhase,
    pub scoreboard: Scoreboard,
    pub pending_round: Option<RoundResult>,
    pub last_round_winner: Option<PlayerId>,
}

impl Game {
    pub fn new() -> Result<Self> {
        let mut game = Self {
            human: Player::new(),
            bot: Player::new(),
            stock: Vec::new(),
            discard: Vec::new(),
            dealer: PlayerId::Bot,
            current_player: PlayerId::Human,
            phase: TurnPhase::AwaitDraw,
            scoreboard: Scoreboard::default(),
            pending_round: None,
            last_round_winner: None,
        };

        game.start_round()?;
        Ok(game)
    }

    pub fn start_round(&mut self) -> Result<()> {
        self.human.hand.clear();
        self.bot.hand.clear();
        self.stock = build_deck();
        self.discard.clear();

        let mut rng = thread_rng();
        self.stock.shuffle(&mut rng);

        for _ in 0..HAND_SIZE {
            let human_card = self.draw_from_stock()?;
            self.human.hand.push(human_card);
            let bot_card = self.draw_from_stock()?;
            self.bot.hand.push(bot_card);
        }
        self.human.sort_hand();
        self.bot.sort_hand();

        let starter = self.draw_from_stock()?;
        self.discard.push(starter);
        self.current_player = self.dealer.other();
        self.phase = TurnPhase::AwaitDraw;
        self.pending_round = None;

        Ok(())
    }

    pub fn restart_with_starting_player(&mut self, starter: PlayerId) -> Result<()> {
        self.dealer = starter.other();
        self.start_round()
    }

    fn draw_from_stock(&mut self) -> Result<Card> {
        self.stock
            .pop()
            .ok_or_else(|| anyhow!("stock pile is empty"))
    }

    pub fn draw(&mut self, player: PlayerId, source: DrawSource) -> Result<ActionOutcome> {
        if self.phase != TurnPhase::AwaitDraw {
            return Err(anyhow!("not expecting a draw"));
        }
        if self.current_player != player {
            return Err(anyhow!("not this player's turn"));
        }
        if source == DrawSource::Stock && self.stock.len() <= 2 {
            let result = RoundResult {
                winner: None,
                points_awarded: 0,
                reason: RoundEndReason::StockDepleted,
                human_hand: self.human.hand.clone(),
                bot_hand: self.bot.hand.clone(),
            };
            self.finish_round(result);
            return Ok(ActionOutcome::RoundEnded);
        }

        let card = match source {
            DrawSource::Stock => self.draw_from_stock()?,
            DrawSource::Discard => self
                .discard
                .pop()
                .ok_or_else(|| anyhow!("discard pile empty"))?,
        };

        {
            let player_ref = self.player_mut(player);
            player_ref.hand.push(card);
            player_ref.sort_hand();
        }

        if self.player(player).hand.len() == HAND_SIZE + 1 {
            let analysis = analyze_hand(&self.player(player).hand);
            if analysis.deadwood_value == 0 {
                let opponent = player.other();
                let opponent_analysis = analyze_hand(&self.player(opponent).hand);
                let opponent_deadwood_value: u32 = opponent_analysis
                    .deadwood
                    .iter()
                    .map(|c| c.rank.value() as u32)
                    .sum();
                let points = opponent_deadwood_value as i32 + BIG_GIN_BONUS;
                let result = RoundResult {
                    winner: Some(player),
                    points_awarded: points,
                    reason: RoundEndReason::BigGin {
                        player,
                        opponent_deadwood: opponent_deadwood_value,
                        bonus: BIG_GIN_BONUS,
                    },
                    human_hand: self.human.hand.clone(),
                    bot_hand: self.bot.hand.clone(),
                };
                self.finish_round(result);
                return Ok(ActionOutcome::RoundEnded);
            }
        }

        self.phase = TurnPhase::AwaitDiscard;
        Ok(ActionOutcome::Continue)
    }

    pub fn discard(
        &mut self,
        player: PlayerId,
        card_index: usize,
        declare_knock: bool,
    ) -> Result<ActionOutcome> {
        if self.phase != TurnPhase::AwaitDiscard {
            return Err(anyhow!("not expecting a discard"));
        }
        if self.current_player != player {
            return Err(anyhow!("not this player's turn"));
        }

        let card = {
            let player_ref = self.player_mut(player);
            if card_index >= player_ref.hand.len() {
                return Err(anyhow!("invalid card index"));
            }
            let card = player_ref.hand.remove(card_index);
            player_ref.sort_hand();
            card
        };
        self.discard.push(card);

        if declare_knock {
            let result = self.resolve_knock(player)?;
            self.finish_round(result);
            return Ok(ActionOutcome::RoundEnded);
        }

        self.advance_turn();
        Ok(ActionOutcome::Continue)
    }

    pub fn resolve_knock(&mut self, knocker: PlayerId) -> Result<RoundResult> {
        let opponent = knocker.other();
        let knocker_hand = self.player(knocker).hand.clone();
        let opponent_hand = self.player(opponent).hand.clone();

        let knocker_analysis = analyze_hand(&knocker_hand);
        if knocker_analysis.deadwood_value > 10 {
            return Err(anyhow!("deadwood too high to knock"));
        }

        let gin = knocker_analysis.deadwood_value == 0;
        let opponent_analysis = analyze_hand(&opponent_hand);

        let (opponent_deadwood_cards, laid_off) = if gin {
            (opponent_analysis.deadwood.clone(), Vec::new())
        } else {
            layoff_cards(&opponent_analysis.deadwood, &knocker_analysis.melds)
        };

        let opponent_deadwood_value: u32 = opponent_deadwood_cards
            .iter()
            .map(|c| c.rank.value() as u32)
            .sum();

        let mut winner = knocker;
        let mut points = opponent_deadwood_value as i32 - knocker_analysis.deadwood_value as i32;
        let mut undercut = false;

        if opponent_deadwood_value <= knocker_analysis.deadwood_value as u32 && !gin {
            winner = opponent;
            undercut = true;
            points = (knocker_analysis.deadwood_value as i32 - opponent_deadwood_value as i32) + 25;
        } else {
            if gin {
                points += 25;
            }
        }

        if winner == PlayerId::Bot && points < 0 {
            points = 0;
        }

        let result = RoundResult {
            winner: Some(winner),
            points_awarded: points.abs(),
            reason: RoundEndReason::Knock {
                knocker,
                knocker_deadwood: knocker_analysis.deadwood_value,
                opponent_deadwood: opponent_deadwood_value,
                laid_off,
                gin,
                undercut,
            },
            human_hand: self.human.hand.clone(),
            bot_hand: self.bot.hand.clone(),
        };

        Ok(result)
    }

    pub fn finish_round(&mut self, result: RoundResult) {
        match result.winner {
            Some(PlayerId::Human) => {
                self.scoreboard.human += result.points_awarded;
                self.scoreboard.human_hands_won += 1;
                self.dealer = PlayerId::Bot;
                self.last_round_winner = Some(PlayerId::Human);
            }
            Some(PlayerId::Bot) => {
                self.scoreboard.bot += result.points_awarded;
                self.scoreboard.bot_hands_won += 1;
                self.dealer = PlayerId::Human;
                self.last_round_winner = Some(PlayerId::Bot);
            }
            None => {
                self.scoreboard.draws += 1;
                self.dealer = self.dealer.other();
                self.last_round_winner = None;
            }
        }
        self.scoreboard.rounds_played += 1;
        self.phase = TurnPhase::RoundOver;
        self.pending_round = Some(result);
    }

    pub fn start_next_round(&mut self) -> Result<()> {
        if self.phase != TurnPhase::RoundOver {
            return Err(anyhow!("round still in progress"));
        }
        self.start_round()
    }

    fn advance_turn(&mut self) {
        self.current_player = self.current_player.other();
        self.phase = TurnPhase::AwaitDraw;
    }

    pub fn player(&self, id: PlayerId) -> &Player {
        match id {
            PlayerId::Human => &self.human,
            PlayerId::Bot => &self.bot,
        }
    }

    pub fn player_mut(&mut self, id: PlayerId) -> &mut Player {
        match id {
            PlayerId::Human => &mut self.human,
            PlayerId::Bot => &mut self.bot,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionOutcome {
    Continue,
    RoundEnded,
}

fn build_deck() -> Vec<Card> {
    let mut deck = Vec::with_capacity(52);
    for &suit in Suit::ALL.iter() {
        for &rank in Rank::ALL.iter() {
            deck.push(Card::new(rank, suit));
        }
    }
    deck
}

fn describe_layoffs(cards: &[Card]) -> String {
    if cards.is_empty() {
        return "none".to_string();
    }
    let labels: Vec<String> = cards.iter().map(|c| c.to_string()).collect();
    labels.join(" ")
}

#[derive(Debug, Clone, Copy)]
pub struct OpeningDrawResult {
    pub human_card: Card,
    pub bot_card: Card,
    pub starter: PlayerId,
}

impl Game {
    pub fn opening_draw(&self) -> OpeningDrawResult {
        let mut deck = build_deck();
        let mut rng = thread_rng();
        deck.shuffle(&mut rng);

        loop {
            if deck.len() < 2 {
                deck = build_deck();
                deck.shuffle(&mut rng);
            }
            let human_card = deck.pop().unwrap();
            let bot_card = deck.pop().unwrap();
            let human_val = human_card.rank.value();
            let bot_val = bot_card.rank.value();
            if human_val == bot_val {
                continue;
            }
            let starter = if human_val > bot_val {
                PlayerId::Human
            } else {
                PlayerId::Bot
            };
            return OpeningDrawResult {
                human_card,
                bot_card,
                starter,
            };
        }
    }
}
