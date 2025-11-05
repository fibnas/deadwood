use std::collections::HashMap;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::{
    app::App,
    cards::Card,
    game::{PlayerId, RoundEndReason, TurnPhase},
    meld::{analyze_hand, MeldKind},
};

const RULES_TEXT: &str = r"GIN RUMMY RULES
----------------
Gin is a two-player card game where the goal is to form melds (runs or sets) while
minimising deadwood. Each turn you draw one card (stock or discard) and then
discard one card.

SETUP
  - Players: two
  - Deck: standard 52-card deck
  - Deal: 10 cards each, remainder becomes the face-down stock
  - Start: top stock card flipped to begin the discard pile; previous winner draws first

CARD VALUES
  - Ace = 1
  - 2-10 = face value
  - J/Q/K = 10

MELDS
  - Sets: three or four cards of the same rank
  - Runs: three or more consecutive cards of the same suit (aces are low)

TURN FLOW
  1. Draw from stock or take the top discard
  2. Optionally rearrange to build melds and reduce deadwood
  3. Discard one card to finish your turn

ENDING A ROUND
  - Knock when your deadwood is 10 or less (after discarding)
      - Opponent may lay off their deadwood onto your melds
  - Go Gin when all 10 cards form melds (opponent cannot lay off, +25 bonus)
  - Undercut occurs when the opponent's deadwood is <= the knocker's (opponent scores +25 plus the difference)

WINNING THE MATCH
  - First player to reach 100 points wins

Press Esc or ? to return to the game.";

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    if app.show_help() {
        draw_help_overlay(frame, app, frame.size());
        return;
    }

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(7),
        ])
        .split(frame.size());

    draw_header(frame, app, layout[0]);
    draw_opponent_hand(frame, app, layout[1]);
    draw_piles(frame, app, layout[2]);
    draw_player_section(frame, app, layout[3]);
}

fn draw_help_overlay(frame: &mut Frame<'_>, _app: &App, area: Rect) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(10),
            Constraint::Percentage(80),
            Constraint::Percentage(10),
        ])
        .split(area);

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(10),
            Constraint::Percentage(80),
            Constraint::Percentage(10),
        ])
        .split(vertical[1]);

    let popup_area = middle[1];
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title("Gin Rummy Reference (?/Esc to close)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(RULES_TEXT)
        .block(block)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, popup_area);
}

fn draw_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let scoreboard = &app.game.scoreboard;
    let phase_text = phase_description(app);
    let mut lines = vec![Line::from(vec![
        Span::raw(format!(
            "Score: You {} | Bot {} (Rounds played: {})",
            scoreboard.human, scoreboard.bot, scoreboard.rounds_played
        )),
        Span::raw(" "),
        Span::styled(
            "Rules: press ?",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        ),
    ])];
    lines.push(Line::from(format!(
        "Hands: You {} | Bot {} | Draws {}",
        scoreboard.human_hands_won, scoreboard.bot_hands_won, scoreboard.draws
    )));
    lines.push(Line::from(format!("Phase: {phase_text}")));

    if let Some(message) = app.status_message() {
        lines.push(Line::from(Span::styled(
            format!("Info: {message}"),
            Style::default().fg(Color::Cyan),
        )));
    }
    if let Some(err) = app.error_message() {
        lines.push(Line::from(Span::styled(
            format!("Error: {err}"),
            Style::default().fg(Color::Red),
        )));
    }

    if let Some(round) = app.game.pending_round.as_ref() {
        if let RoundEndReason::Knock {
            knocker, laid_off, ..
        } = &round.reason
        {
            let layoff_by = match knocker {
                PlayerId::Human => "Bot",
                PlayerId::Bot => "You",
            };
            let laid_off_label = format_card_list(laid_off);
            lines.push(Line::from(format!(
                "Layoffs by {layoff_by}: {laid_off_label}"
            )));
        }
    }

    let instructions = instructions_for_phase(app);
    lines.push(Line::from(instructions));

    let paragraph = Paragraph::new(lines)
        .block(Block::default().title("Status").borders(Borders::ALL))
        .alignment(Alignment::Left);
    frame.render_widget(paragraph, area);
}

fn draw_opponent_hand(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mut layoff_cards: Vec<Card> = Vec::new();
    let reveal_cards = if app.game.phase == TurnPhase::RoundOver {
        app.game.pending_round.as_ref().map(|round| {
            if let RoundEndReason::Knock {
                knocker, laid_off, ..
            } = &round.reason
            {
                if *knocker == PlayerId::Human {
                    layoff_cards = laid_off.clone();
                }
            }
            round.bot_hand.as_slice()
        })
    } else {
        None
    };

    let spans: Vec<Span> = if let Some(cards) = reveal_cards {
        let mut spans = Vec::new();
        for (idx, card) in cards.iter().enumerate() {
            if idx > 0 {
                spans.push(Span::raw(" "));
            }
            let was_laid_off = layoff_cards.contains(card);
            let mut rank_style = Style::default();
            if was_laid_off {
                rank_style = rank_style
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::UNDERLINED);
            }
            spans.push(Span::styled(card.rank.short_name().to_string(), rank_style));

            let mut suit_style = Style::default().fg(app.suit_color(card.suit));
            if was_laid_off {
                suit_style = suit_style.add_modifier(Modifier::UNDERLINED);
            }
            spans.push(Span::styled(card.suit.symbol().to_string(), suit_style));

            if was_laid_off {
                spans.push(Span::styled("*", Style::default().fg(Color::Yellow)));
            }
        }
        spans
    } else {
        app.game
            .bot
            .hand
            .iter()
            .map(|_| Span::raw(format!(" {} ", Card::face_down())))
            .collect()
    };
    let line = Line::from(spans);
    let paragraph = Paragraph::new(line)
        .block(Block::default().title("Opponent").borders(Borders::ALL))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn draw_piles(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let stock_size = app.game.stock.len();
    let stock_para = Paragraph::new(vec![
        Line::from("Stock pile"),
        Line::from(Span::styled(
            format!("Cards left: {stock_size}"),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(format!("Top: {}", Card::face_down())),
    ])
    .block(Block::default().title("Stock").borders(Borders::ALL))
    .alignment(Alignment::Center);

    let discard_para = Paragraph::new(vec![
        Line::from("Discard pile"),
        Line::from(Span::styled(
            format!("Cards: {}", app.game.discard.len()),
            Style::default().fg(Color::Yellow),
        )),
        if let Some(card) = app.game.discard.last() {
            Line::from(vec![
                Span::raw("Top: "),
                Span::styled(card.rank.short_name().to_string(), Style::default()),
                Span::styled(
                    card.suit.symbol().to_string(),
                    Style::default().fg(app.suit_color(card.suit)),
                ),
            ])
        } else {
            Line::from("Top: --")
        },
    ])
    .block(Block::default().title("Discard").borders(Borders::ALL))
    .alignment(Alignment::Center);

    frame.render_widget(stock_para, layout[0]);
    frame.render_widget(discard_para, layout[1]);
}

fn draw_player_section(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    draw_player_hand(frame, app, layout[0]);
    draw_player_details(frame, app, layout[1]);
}

fn draw_player_hand(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let recent_draw = app.recent_draw();
    let hand_slice = if app.game.phase == TurnPhase::RoundOver {
        app.game
            .pending_round
            .as_ref()
            .map(|round| round.human_hand.as_slice())
            .unwrap_or_else(|| app.game.human.hand.as_slice())
    } else {
        app.game.human.hand.as_slice()
    };

    let layoff_cards: Vec<Card> = if app.game.phase == TurnPhase::RoundOver {
        app.game
            .pending_round
            .as_ref()
            .and_then(|round| match &round.reason {
                RoundEndReason::Knock {
                    knocker, laid_off, ..
                } if *knocker == PlayerId::Bot => Some(laid_off.clone()),
                _ => None,
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    if hand_slice.is_empty() {
        let paragraph = Paragraph::new("Your hand is empty.")
            .block(Block::default().title("You").borders(Borders::ALL))
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    let analysis = analyze_hand(hand_slice);
    let mut card_membership: HashMap<Card, MeldKind> = HashMap::new();
    for meld in &analysis.melds {
        for &card in &meld.cards {
            card_membership.insert(card, meld.kind);
        }
    }

    let selection_style = Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD);

    let mut spans: Vec<Span> = Vec::new();

    for (idx, card) in hand_slice.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::raw(" "));
        }

        let is_selected = idx == app.selection;
        let was_laid_off = layoff_cards.contains(card);
        let is_recent = Some(*card) == recent_draw;
        if is_selected {
            spans.push(Span::styled("[", selection_style));
        }

        let bracket_kind = if app.auto_brackets() {
            card_membership.get(card).copied()
        } else {
            None
        };
        let (open_char, close_char, bracket_style) = match bracket_kind {
            Some(MeldKind::Run) => (Some('('), Some(')'), Style::default().fg(Color::Cyan)),
            Some(MeldKind::Set) => (Some('{'), Some('}'), Style::default().fg(Color::Yellow)),
            None => (None, None, Style::default()),
        };

        if let Some(open) = open_char {
            spans.push(Span::styled(open.to_string(), bracket_style));
        }

        let mut rank_style = Style::default();
        if is_recent {
            rank_style = rank_style.bg(Color::DarkGray);
        }
        if was_laid_off {
            rank_style = rank_style.add_modifier(Modifier::UNDERLINED);
        }
        if is_selected {
            rank_style = rank_style.fg(Color::Green).add_modifier(Modifier::BOLD);
            if was_laid_off {
                rank_style = rank_style.add_modifier(Modifier::UNDERLINED);
            }
        }
        spans.push(Span::styled(card.rank.short_name().to_string(), rank_style));

        let mut suit_style = Style::default().fg(app.suit_color(card.suit));
        if is_recent {
            suit_style = suit_style.bg(Color::DarkGray);
        }
        if was_laid_off {
            suit_style = suit_style.add_modifier(Modifier::UNDERLINED);
        }
        if is_selected {
            suit_style = suit_style.add_modifier(Modifier::BOLD);
            if was_laid_off {
                suit_style = suit_style.add_modifier(Modifier::UNDERLINED);
            }
        }
        spans.push(Span::styled(card.suit.symbol().to_string(), suit_style));

        if let Some(close) = close_char {
            spans.push(Span::styled(close.to_string(), bracket_style));
        }

        if is_selected {
            spans.push(Span::styled("]", selection_style));
        }

        if was_laid_off {
            spans.push(Span::styled("*", Style::default().fg(Color::Yellow)));
        }
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line)
        .block(Block::default().title("Your Hand").borders(Borders::ALL))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}
fn draw_player_details(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mut lines = Vec::new();
    if let TurnPhase::AwaitDiscard = app.game.phase {
        let knock_status = if app.knock_intent() { "ON" } else { "OFF" };
        lines.push(Line::from(format!("Knock intent: {knock_status}")));
    }

    let hand_slice = if app.game.phase == TurnPhase::RoundOver {
        app.game
            .pending_round
            .as_ref()
            .map(|round| round.human_hand.as_slice())
            .unwrap_or_else(|| app.game.human.hand.as_slice())
    } else {
        app.game.human.hand.as_slice()
    };

    let analysis = analyze_hand(hand_slice);
    lines.push(Line::from(format!(
        "Deadwood: {} ({} cards)",
        analysis.deadwood_value,
        analysis.deadwood.len()
    )));

    let paragraph = Paragraph::new(lines)
        .block(Block::default().title("Details").borders(Borders::ALL))
        .alignment(Alignment::Left);
    frame.render_widget(paragraph, area);
}

fn phase_description(app: &App) -> String {
    if app.exit_prompt_active() {
        return "Exit requested: choose Y to save, N to quit without saving, Esc to cancel."
            .to_string();
    }
    if app.show_help() {
        return "Rules reference open. Press Esc or ? to close.".to_string();
    }
    match app.game.phase {
        TurnPhase::RoundOver => "Round complete. Press Enter to continue.".to_string(),
        TurnPhase::AwaitDraw => match app.game.current_player {
            PlayerId::Human => "Your turn: draw from stock [S] or discard [D].".to_string(),
            PlayerId::Bot => "Bot drawing...".to_string(),
        },
        TurnPhase::AwaitDiscard => match app.game.current_player {
            PlayerId::Human => "Your turn: choose a card to discard.".to_string(),
            PlayerId::Bot => "Bot deciding on a discard...".to_string(),
        },
    }
}

fn instructions_for_phase(app: &App) -> String {
    if app.exit_prompt_active() {
        return "Controls: Y=save & quit, N=quit without saving, Esc=cancel.".to_string();
    }
    if app.show_help() {
        return "Controls: Esc/?=close rules.".to_string();
    }
    match app.game.phase {
        TurnPhase::RoundOver => "Controls: Enter/N=next round, ?=rules, Q=quit.".to_string(),
        TurnPhase::AwaitDraw => "Controls: S=stock, D=discard, ?=rules, Q=quit.".to_string(),
        TurnPhase::AwaitDiscard => {
            "Controls: ←/→ move, Enter=discard, K=toggle knock, ?=rules, Q=quit.".to_string()
        }
    }
}

fn format_card_list(cards: &[Card]) -> String {
    if cards.is_empty() {
        "none".to_string()
    } else {
        cards
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }
}
