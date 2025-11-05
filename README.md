# Deadwood

Deadwood is a terminal user interface (TUI) implementation of Gin Rummy written in Rust. It delivers a full single–player experience against an algorithmic bot, complete with proper Gin scoring, knock/deadwood rules, and a responsive Crossterm/Ratatui front end.

[![Crates.io](https://img.shields.io/crates/v/deadwood.svg)](https://crates.io/crates/deadwood)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org)

## Highlights

- **Playable Gin Rummy**: Standard 52-card deck, 10-card hands, draw/discard flow, knock/Gin/undercut scoring, and deadwood tracking.
- **Responsive TUI**: Crossterm + Ratatui interface shows both hands (opponent face-down), stock/discard piles, scoreboard (points plus hands won), contextual controls, and live deadwood totals.
- **Session Awareness**: Optional persistence keeps score/history between runs, highlights your most recent draw, and exposes simple configuration knobs.
- **Round Reveals**: When a hand ends the opponent's cards flip up, and the status panel lists any layoff cards so you can review how the knock resolved.
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
git clone https://github.com/fibnas/deadwood.git
cd deadwood
cargo build
```

### Running the Game

```bash
cargo run
```

The game launches directly into the TUI. Resize the terminal as needed; Ratatui adapts to larger viewports.

### Configuration & Persistence

On first launch Deadwood writes a config file to your OS config directory (for example `~/.config/deadwood/config.toml`). You can tweak these options:

- `persist_stats` – keep cumulative scores and the latest round summaries between runs (creates `session.json` alongside the config).
- `auto_brackets` – toggle automatic braces around detected melds/runs in your hand view.
- `[suit_colors]` – override suit colours with recognised names (`Red`, `Blue`, …), `#RRGGBB` hex strings, or `rgb(r,g,b)` values.

There is a starter template at [`config.example.toml`](config.example.toml); copy or adapt it for your setup.

## Controls

Game controls change depending on the current phase, and the status panel always reminds you what to press.

| Phase                | Keys                                                                 |
| -------------------- | -------------------------------------------------------------------- |
| Menu / Round over    | `Enter`/`n` – start next round · `?` – rules · `q`/`Esc` – quit       |
| Draw phase           | `s` – draw stock · `d` – draw discard · `?` – rules · `q`/`Esc` – quit |
| Discard / knock phase| `←`/`→` or `h`/`l` – move selector · `Enter`/`Space` – discard · `k` – toggle knock intent · `?` – rules · `q`/`Esc` – quit |

## Rules & Scoring

Deadwood follows standard Gin Rummy rules:

- Players are dealt 10 cards. The non-dealer starts after the first discard is revealed.
- The previous round's winner draws first; on a fresh game a quick high-card draw decides the opener.
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
