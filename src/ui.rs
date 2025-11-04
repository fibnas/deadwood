use std::collections::HashMap;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{
    app::App,
    cards::{Card, Suit},
    game::{PlayerId, TurnPhase},
    meld::{analyze_hand, MeldKind},
};

pub fn draw(frame: &mut Frame<'_>, app: &App) {
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

fn draw_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let scoreboard = &app.game.scoreboard;
    let phase_text = phase_description(app);
    let mut lines = vec![
        Line::from(format!(
            "Score: You {} | Bot {} (Rounds played: {})",
            scoreboard.human, scoreboard.bot, scoreboard.rounds_played
        )),
        Line::from(format!("Phase: {phase_text}")),
    ];

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

    let instructions = instructions_for_phase(app);
    lines.push(Line::from(instructions));

    let paragraph = Paragraph::new(lines)
        .block(Block::default().title("Status").borders(Borders::ALL))
        .alignment(Alignment::Left);
    frame.render_widget(paragraph, area);
}

fn draw_opponent_hand(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let hand = &app.game.bot.hand;
    let spans: Vec<Span> = hand
        .iter()
        .map(|_| Span::raw(format!(" {} ", Card::face_down())))
        .collect();
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
                    Style::default().fg(suit_color(card.suit)),
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
    let hand = &app.game.human.hand;

    if hand.is_empty() {
        let paragraph = Paragraph::new("Your hand is empty.")
            .block(Block::default().title("You").borders(Borders::ALL))
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    let analysis = analyze_hand(hand);
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

    for (idx, card) in hand.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::raw(" "));
        }

        let is_selected = idx == app.selection;
        if is_selected {
            spans.push(Span::styled("[", selection_style));
        }

        let bracket_kind = card_membership.get(card).copied();
        let (open_char, close_char, bracket_style) = match bracket_kind {
            Some(MeldKind::Run) => (Some('('), Some(')'), Style::default().fg(Color::Cyan)),
            Some(MeldKind::Set) => (Some('{'), Some('}'), Style::default().fg(Color::Yellow)),
            None => (None, None, Style::default()),
        };

        if let Some(open) = open_char {
            spans.push(Span::styled(open.to_string(), bracket_style));
        }

        let mut rank_style = Style::default();
        if is_selected {
            rank_style = rank_style.fg(Color::Green).add_modifier(Modifier::BOLD);
        }
        spans.push(Span::styled(card.rank.short_name().to_string(), rank_style));

        let mut suit_style = Style::default().fg(suit_color(card.suit));
        if is_selected {
            suit_style = suit_style.add_modifier(Modifier::BOLD);
        }
        spans.push(Span::styled(card.suit.symbol().to_string(), suit_style));

        if let Some(close) = close_char {
            spans.push(Span::styled(close.to_string(), bracket_style));
        }

        if is_selected {
            spans.push(Span::styled("]", selection_style));
        }
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line)
        .block(Block::default().title("Your Hand").borders(Borders::ALL))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn suit_color(suit: Suit) -> Color {
    match suit {
        Suit::Hearts => Color::Red,
        Suit::Diamonds => Color::Magenta,
        Suit::Clubs => Color::Green,
        Suit::Spades => Color::Blue,
    }
}

fn draw_player_details(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mut lines = Vec::new();
    if let TurnPhase::AwaitDiscard = app.game.phase {
        let knock_status = if app.knock_intent() { "ON" } else { "OFF" };
        lines.push(Line::from(format!("Knock intent: {knock_status}")));
    }

    let analysis = analyze_hand(&app.game.human.hand);
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
    match app.game.phase {
        TurnPhase::RoundOver => "Controls: Enter/N to start next round, Q to quit.".to_string(),
        TurnPhase::AwaitDraw => "Controls: S=draw stock, D=draw discard, Q=quit.".to_string(),
        TurnPhase::AwaitDiscard => {
            "Controls: ←/→ move, Enter=discard, K=toggle knock, Q=quit.".to_string()
        }
    }
}
