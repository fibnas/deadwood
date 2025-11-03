use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    bot::{BotDifficulty, take_turn},
    game::{ActionOutcome, DrawSource, Game, PlayerId, TurnPhase},
};

pub struct App {
    should_quit: bool,
    pub game: Game,
    pub selection: usize,
    message: Option<String>,
    error: Option<String>,
    knock_intent: bool,
    bot_difficulty: BotDifficulty,
}

impl App {
    pub fn new() -> Result<Self> {
        Ok(Self {
            should_quit: false,
            game: Game::new().context("failed to initialise game")?,
            selection: 0,
            message: None,
            error: None,
            knock_intent: false,
            bot_difficulty: BotDifficulty::Challenging,
        })
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn status_message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn reset_messages(&mut self) {
        self.message = None;
        self.error = None;
    }

    pub fn update(&mut self) -> Result<()> {
        if self.game.phase == TurnPhase::RoundOver {
            return Ok(());
        }

        while self.game.phase != TurnPhase::RoundOver && self.game.current_player == PlayerId::Bot {
            match take_turn(&mut self.game, self.bot_difficulty)? {
                ActionOutcome::Continue => {
                    if self.game.current_player != PlayerId::Bot {
                        break;
                    }
                }
                ActionOutcome::RoundEnded => {
                    self.on_round_end();
                    break;
                }
            }
        }
        Ok(())
    }

    pub fn handle_key(&mut self, key_event: KeyEvent) -> Result<()> {
        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
                return Ok(());
            }
            _ => {}
        }

        self.error = None;

        if self.game.phase == TurnPhase::RoundOver {
            match key_event.code {
                KeyCode::Enter | KeyCode::Char('n') => {
                    self.reset_messages();
                    self.game.start_next_round()?;
                    self.selection = 0;
                    self.knock_intent = false;
                    self.message = Some("New round started.".to_string());
                    self.update()?;
                }
                _ => {}
            }
            return Ok(());
        }

        if self.game.current_player != PlayerId::Human {
            return Ok(());
        }

        match self.game.phase {
            TurnPhase::AwaitDraw => self.handle_draw_phase(key_event)?,
            TurnPhase::AwaitDiscard => self.handle_discard_phase(key_event)?,
            TurnPhase::RoundOver => {}
        }

        Ok(())
    }

    fn handle_draw_phase(&mut self, key_event: KeyEvent) -> Result<()> {
        match key_event.code {
            KeyCode::Char(c) if c.eq_ignore_ascii_case(&'s') => {
                self.execute_draw(DrawSource::Stock)?
            }
            KeyCode::Char(c) if c.eq_ignore_ascii_case(&'d') => {
                self.execute_draw(DrawSource::Discard)?
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_discard_phase(&mut self, key_event: KeyEvent) -> Result<()> {
        match key_event.code {
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('H') => self.move_selection_left(),
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => self.move_selection_right(),
            KeyCode::Char(c) if c.eq_ignore_ascii_case(&'k') => self.toggle_knock(),
            KeyCode::Enter | KeyCode::Char(' ') => self.execute_discard()?,
            _ => {}
        }
        Ok(())
    }

    fn execute_draw(&mut self, source: DrawSource) -> Result<()> {
        match self.game.draw(PlayerId::Human, source) {
            Ok(ActionOutcome::Continue) => {
                self.selection = self.game.human.hand.len().saturating_sub(1);
                self.knock_intent = false;
            }
            Ok(ActionOutcome::RoundEnded) => self.on_round_end(),
            Err(err) => self.error = Some(err.to_string()),
        }
        Ok(())
    }

    fn execute_discard(&mut self) -> Result<()> {
        if self.game.human.hand.is_empty() {
            return Ok(());
        }

        let index = self.selection.min(self.game.human.hand.len() - 1);
        match self.game.discard(PlayerId::Human, index, self.knock_intent) {
            Ok(ActionOutcome::Continue) => {
                self.selection = 0;
                self.knock_intent = false;
                self.update()?;
            }
            Ok(ActionOutcome::RoundEnded) => self.on_round_end(),
            Err(err) => {
                self.error = Some(err.to_string());
                self.knock_intent = false;
            }
        }
        Ok(())
    }

    fn move_selection_left(&mut self) {
        if self.game.human.hand.is_empty() {
            return;
        }
        if self.selection == 0 {
            self.selection = self.game.human.hand.len() - 1;
        } else {
            self.selection -= 1;
        }
    }

    fn move_selection_right(&mut self) {
        if self.game.human.hand.is_empty() {
            return;
        }
        self.selection = (self.selection + 1) % self.game.human.hand.len();
    }

    fn toggle_knock(&mut self) {
        self.knock_intent = !self.knock_intent;
    }

    fn on_round_end(&mut self) {
        if let Some(result) = self.game.pending_round.clone() {
            let summary = format!(
                "{} | Score: You {} - Bot {}",
                result, self.game.scoreboard.human, self.game.scoreboard.bot
            );
            self.message = Some(summary);
        }
        self.selection = 0;
        self.knock_intent = false;
    }

    pub fn knock_intent(&self) -> bool {
        self.knock_intent
    }
}
