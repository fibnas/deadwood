# Deadwood

Deadwood is a terminal user interface (TUI) implementation of Gin Rummy written in Rust. It delivers a full single–player experience against an algorithmic bot, complete with proper Gin scoring, knock/deadwood rules, and a responsive Crossterm/Ratatui front end.

## Highlights

- **Playable Gin Rummy**: Standard 52-card deck, 10-card hands, draw/discard flow, knock/Gin/undercut scoring, and deadwood tracking.
- **Responsive TUI**: Crossterm + Ratatui interface shows both hands (opponent face-down), stock/discard piles, scoreboard, contextual controls, and live deadwood totals.
- **Bot Opponent**: Deterministic heuristics with difficulty setting baked into the game core; the AI evaluates meld potential, knock opportunities, and throws in occasional randomness on easier settings.
- **Pure Rust**: No external game logic crates—cards, deck building, meld search, deadwood analysis, and scoring are all homegrown and unit-test friendly.

## Getting Started

### Prerequisites

- Rust toolchain (1.75 or newer recommended)
- Cargo (bundled with Rustup)
- A terminal capable of ANSI/Crossterm control sequences

### Installation

Clone the repository and build the project:

```bash
git clone https://github.com/<your-org>/deadwood.git
cd deadwood
cargo build
```

### Running the Game

```bash
cargo run
```

The game launches directly into the TUI. Resize the terminal as needed; Ratatui adapts to larger viewports.

## Controls

Game controls change depending on the current phase, and the status panel always reminds you what to press.

| Phase                | Keys                                                                 |
| -------------------- | -------------------------------------------------------------------- |
| Menu / Round over    | `Enter` or `n` – start next round · `q` / `Esc` – quit               |
| Draw phase           | `s` – draw from stock · `d` – draw from discard · `q` / `Esc` – quit |
| Discard / knock phase| `←`/`→` or `h`/`l` – move selector · `Enter`/`Space` – discard · `k` – toggle knock intent · `q` / `Esc` – quit |

## Rules & Scoring

Deadwood follows standard Gin Rummy rules:

- Players are dealt 10 cards. The non-dealer starts after the first discard is revealed.
- Each turn consists of drawing (stock or discard) and discarding one card.
- A player may **knock** when their deadwood total is 10 or fewer points. Picking a discard does not oblige you to knock.
- A player gets **Gin** when they knock with zero deadwood and receives an additional 25-point bonus.
- If the opponent’s deadwood (after laying off any legal cards) is **less than or equal** to the knocker’s, an **undercut** occurs; the opponent wins the hand and receives the difference plus a 25-point bonus.
- A round is a draw if the stock pile drops to two cards.

Deadwood values: Ace = 1, 2–9 = face value, 10/J/Q/K = 10.

### Meld Detection & Layoffs

The engine searches all valid combinations of runs (same suit, sequential ranks) and sets (same rank). When a knock occurs, the opponent is allowed to lay off deadwood onto the knocker’s melds, extending runs or sets when legal. These mechanics are handled automatically and reflected in the round summary.

## Bot Behavior

The bot evaluates both drawing sources, simulates discard outcomes, and will knock based on configurable difficulty thresholds (default: `Challenging`). On the easier setting it occasionally injects randomness to appear less perfect. All logic lives in `src/bot.rs`.

## Project Layout

```
src/
 ├─ main.rs      # Terminal bootstrap, event loop
 ├─ app.rs       # App state machine, input handling, round orchestration
 ├─ ui.rs        # Ratatui rendering functions
 ├─ cards.rs     # Card, rank, suit types and helpers
 ├─ meld.rs      # Meld detection, deadwood analysis, layoff logic
 ├─ game.rs      # Core Gin Rummy rules, scoring, turn phases
 └─ bot.rs       # Bot strategy and difficulty helpers
```

## Roadmap Ideas

1. **Multiple Difficulties**: Expose difficulty selection in the UI (currently hard-coded).
2. **Enhanced Bot**: Monte Carlo or minimax-style simulations, bluffing, and discard inference.
3. **Replay / History**: Persist game logs and provide round-by-round review.
4. **Multiplayer**: Hot-seat or networked play with proper turn synchronization.
5. **Tests & Benchmarks**: Lightweight property tests for meld search and scoring, plus performance benchmarks for deadwood analysis.

## Contributing

Issues and PRs are welcome. Please format with `cargo fmt` and ensure `cargo check` passes before submitting.

## License

Deadwood is released under the [MIT License](LICENSE).
