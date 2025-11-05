use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::Color;

use crate::{
    bot::{take_turn, BotDifficulty},
    cards::{Card, Suit},
    config::{Config, ConfigLoadOutcome},
    game::{ActionOutcome, DrawSource, Game, PlayerId, TurnPhase},
    storage::{self, Paths, RoundSummary, SessionData},
};

const EXIT_PROMPT_MESSAGE: &str =
    "Save session stats before quitting? (Y=save, N=exit, Esc=cancel).";
const MAX_ROUND_HISTORY: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExitPrompt {
    SaveBeforeQuit,
}

pub struct App {
    should_quit: bool,
    pub game: Game,
    pub selection: usize,
    message: Option<String>,
    error: Option<String>,
    knock_intent: bool,
    bot_difficulty: BotDifficulty,
    config: Config,
    paths: Paths,
    exit_prompt: Option<ExitPrompt>,
    round_history: Vec<RoundSummary>,
    recent_draw: Option<Card>,
    show_help: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        let game = Game::new().context("failed to initialise game")?;
        let paths = Paths::new().context("failed to prepare application directories")?;
        let ConfigLoadOutcome {
            config,
            created,
            warnings,
        } = Config::load_or_create(paths.config_file()).context("failed to load configuration")?;

        let mut session_data: Option<SessionData> = None;
        let mut session_errors = Vec::new();
        if config.persist_stats() {
            match storage::load_session(paths.session_file()) {
                Ok(Some(data)) => session_data = Some(data),
                Ok(None) => {}
                Err(err) => session_errors.push(format!("Failed to load session data: {err}")),
            }
        }

        let mut app = Self {
            should_quit: false,
            game,
            selection: 0,
            message: None,
            error: None,
            knock_intent: false,
            bot_difficulty: BotDifficulty::Challenging,
            config,
            paths,
            exit_prompt: None,
            round_history: Vec::new(),
            recent_draw: None,
            show_help: false,
        };

        let mut info_messages = Vec::new();
        if created {
            info_messages.push(format!(
                "Created default config at {}.",
                app.paths.config_file().display()
            ));
        }

        if let Some(data) = session_data {
            app.game.scoreboard = data.scoreboard;
            let mut history = data.round_history;
            if history.len() > MAX_ROUND_HISTORY {
                let start = history.len() - MAX_ROUND_HISTORY;
                history = history.split_off(start);
            }
            app.round_history = history;
            info_messages.push(format!(
                "Loaded session data ({} rounds).",
                app.game.scoreboard.rounds_played
            ));
        }

        if app.game.scoreboard.rounds_played == 0 {
            let draw = app.game.opening_draw();
            app.game
                .restart_with_starting_player(draw.starter)
                .context("failed to apply opening draw")?;
            app.recent_draw = None;
            let starter_label = match draw.starter {
                PlayerId::Human => "You",
                PlayerId::Bot => "Bot",
            };
            info_messages.push(format!(
                "Opening draw: You drew {}, Bot drew {}. {starter_label} will begin.",
                draw.human_card, draw.bot_card
            ));
        }

        if !info_messages.is_empty() {
            app.message = Some(info_messages.join(" "));
        }

        let mut collected_errors = session_errors;
        if !warnings.is_empty() {
            collected_errors.extend(warnings);
        }
        if !collected_errors.is_empty() {
            app.error = Some(collected_errors.join(" "));
        }

        Ok(app)
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
        if self.show_help {
            return Ok(());
        }

        if self.exit_prompt.is_some() || self.game.phase == TurnPhase::RoundOver {
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
        if self.show_help {
            match key_event.code {
                KeyCode::Esc | KeyCode::Char('?') => {
                    self.show_help = false;
                    self.message = Some("Returned to the game.".to_string());
                }
                _ => {}
            }
            return Ok(());
        }

        if self.process_exit_prompt(key_event)? {
            return Ok(());
        }

        if let KeyCode::Char('?') = key_event.code {
            self.show_help = true;
            self.message = Some("Gin Rummy rules open. Press Esc or ? to close.".to_string());
            return Ok(());
        }

        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.request_exit()?;
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
                    self.recent_draw = None;
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

    fn process_exit_prompt(&mut self, key_event: KeyEvent) -> Result<bool> {
        if self.exit_prompt.is_none() {
            return Ok(false);
        }

        match key_event.code {
            KeyCode::Char(c) if c.eq_ignore_ascii_case(&'y') => {
                self.save_and_quit()?;
            }
            KeyCode::Char(c) if c.eq_ignore_ascii_case(&'n') => {
                self.exit_prompt = None;
                self.should_quit = true;
            }
            KeyCode::Esc => {
                self.exit_prompt = None;
                self.message = Some("Exit cancelled.".to_string());
            }
            _ => {}
        }

        Ok(true)
    }

    fn request_exit(&mut self) -> Result<()> {
        if self.config.persist_stats() {
            self.save_and_quit()?;
        } else {
            self.exit_prompt = Some(ExitPrompt::SaveBeforeQuit);
            self.message = Some(EXIT_PROMPT_MESSAGE.to_string());
        }
        Ok(())
    }

    fn save_and_quit(&mut self) -> Result<()> {
        match self.save_session_data() {
            Ok(_) => {
                self.exit_prompt = None;
                self.should_quit = true;
            }
            Err(err) => {
                self.error = Some(format!("Failed to save session data: {err}"));
                self.exit_prompt = Some(ExitPrompt::SaveBeforeQuit);
                self.message = Some(EXIT_PROMPT_MESSAGE.to_string());
            }
        }
        Ok(())
    }

    fn save_session_data(&mut self) -> Result<()> {
        let data = SessionData::new(self.game.scoreboard.clone(), self.round_history.clone());
        storage::save_session(self.paths.session_file(), &data)
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
        let previous_hand = self.game.human.hand.clone();
        match self.game.draw(PlayerId::Human, source) {
            Ok(ActionOutcome::Continue) => {
                self.selection = self.game.human.hand.len().saturating_sub(1);
                self.knock_intent = false;
                let drawn_card = self
                    .game
                    .human
                    .hand
                    .iter()
                    .copied()
                    .find(|card| !previous_hand.contains(card));
                self.recent_draw = drawn_card;
            }
            Ok(ActionOutcome::RoundEnded) => {
                self.recent_draw = None;
                self.on_round_end();
            }
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
                self.recent_draw = None;
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
            let sb = &self.game.scoreboard;
            let summary = format!(
                "Round {}: {} | Score: You {} - Bot {} | Hands: You {} Bot {} Draws {}",
                sb.rounds_played,
                result,
                sb.human,
                sb.bot,
                sb.human_hands_won,
                sb.bot_hands_won,
                sb.draws
            );
            self.message = Some(summary.clone());
            self.record_round(summary);
        }
        self.selection = 0;
        self.knock_intent = false;
        self.recent_draw = None;
    }

    fn record_round(&mut self, summary: String) {
        let entry = RoundSummary {
            round_number: self.game.scoreboard.rounds_played,
            description: summary,
        };
        self.round_history.push(entry);
        if self.round_history.len() > MAX_ROUND_HISTORY {
            self.round_history.remove(0);
        }
    }

    pub fn knock_intent(&self) -> bool {
        self.knock_intent
    }

    pub fn suit_color(&self, suit: Suit) -> Color {
        self.config.suit_color(suit)
    }

    pub fn auto_brackets(&self) -> bool {
        self.config.auto_brackets()
    }

    pub fn exit_prompt_active(&self) -> bool {
        self.exit_prompt.is_some()
    }

    pub fn recent_draw(&self) -> Option<Card> {
        self.recent_draw
    }

    pub fn show_help(&self) -> bool {
        self.show_help
    }
}
