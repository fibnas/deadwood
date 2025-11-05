use std::{fs, path::Path};

use anyhow::{Context, Result};
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use crate::cards::Suit;

#[derive(Debug, Clone)]
pub struct Config {
    persist_stats: bool,
    auto_brackets: bool,
    palette: SuitColorPalette,
}

#[derive(Debug, Clone)]
struct SuitColorPalette {
    hearts: Color,
    diamonds: Color,
    clubs: Color,
    spades: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigFile {
    #[serde(default = "default_persist_stats")]
    persist_stats: bool,
    #[serde(default = "default_auto_brackets")]
    auto_brackets: bool,
    #[serde(default)]
    suit_colors: SuitColorStrings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SuitColorStrings {
    hearts: String,
    diamonds: String,
    clubs: String,
    spades: String,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            persist_stats: default_persist_stats(),
            auto_brackets: default_auto_brackets(),
            suit_colors: SuitColorStrings::default(),
        }
    }
}

impl Default for SuitColorStrings {
    fn default() -> Self {
        Self {
            hearts: "Red".to_string(),
            diamonds: "Magenta".to_string(),
            clubs: "Green".to_string(),
            spades: "Blue".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct ConfigLoadOutcome {
    pub config: Config,
    pub created: bool,
    pub warnings: Vec<String>,
}

impl Config {
    pub fn load_or_create(path: &Path) -> Result<ConfigLoadOutcome> {
        let created;
        let mut warnings = Vec::new();
        let data = if path.exists() {
            created = false;
            let content = fs::read_to_string(path)
                .with_context(|| format!("failed to read config file at {}", path.display()))?;
            match toml::from_str::<ConfigFile>(&content) {
                Ok(parsed) => parsed,
                Err(err) => {
                    warnings.push(format!(
                        "Failed to parse config file at {}: {err}. Using defaults.",
                        path.display()
                    ));
                    ConfigFile::default()
                }
            }
        } else {
            created = true;
            let data = ConfigFile::default();
            let serialized =
                toml::to_string_pretty(&data).context("failed to serialise default config")?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create config directory at {}", parent.display())
                })?;
            }
            fs::write(path, serialized)
                .with_context(|| format!("failed to write default config to {}", path.display()))?;
            data
        };

        let (config, mut palette_warnings) = Config::from_file(data);
        warnings.append(&mut palette_warnings);
        Ok(ConfigLoadOutcome {
            config,
            created,
            warnings,
        })
    }

    fn from_file(data: ConfigFile) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let palette = SuitColorPalette::from_strings(&data.suit_colors, &mut warnings);
        (
            Self {
                persist_stats: data.persist_stats,
                auto_brackets: data.auto_brackets,
                palette,
            },
            warnings,
        )
    }

    pub fn persist_stats(&self) -> bool {
        self.persist_stats
    }

    pub fn auto_brackets(&self) -> bool {
        self.auto_brackets
    }

    pub fn suit_color(&self, suit: Suit) -> Color {
        self.palette.color(suit)
    }
}

impl SuitColorPalette {
    fn from_strings(strings: &SuitColorStrings, warnings: &mut Vec<String>) -> Self {
        Self {
            hearts: parse_color_with_default(&strings.hearts, Suit::Hearts, warnings),
            diamonds: parse_color_with_default(&strings.diamonds, Suit::Diamonds, warnings),
            clubs: parse_color_with_default(&strings.clubs, Suit::Clubs, warnings),
            spades: parse_color_with_default(&strings.spades, Suit::Spades, warnings),
        }
    }

    fn color(&self, suit: Suit) -> Color {
        match suit {
            Suit::Hearts => self.hearts,
            Suit::Diamonds => self.diamonds,
            Suit::Clubs => self.clubs,
            Suit::Spades => self.spades,
        }
    }
}

fn parse_color_with_default(value: &str, suit: Suit, warnings: &mut Vec<String>) -> Color {
    if let Some(color) = parse_color(value) {
        return color;
    }
    warnings.push(format!(
        "Unrecognised colour '{}' for {}. Using default.",
        value,
        suit_label(suit)
    ));
    default_color(suit)
}

fn parse_color(value: &str) -> Option<Color> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" | "purple" => Some(Color::Magenta),
        "cyan" | "teal" => Some(Color::Cyan),
        "white" => Some(Color::White),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" => Some(Color::DarkGray),
        "lightred" | "brightred" => Some(Color::LightRed),
        "lightgreen" | "brightgreen" => Some(Color::LightGreen),
        "lightyellow" | "brightyellow" => Some(Color::LightYellow),
        "lightblue" | "brightblue" => Some(Color::LightBlue),
        "lightmagenta" | "brightmagenta" | "lightpurple" | "brightpurple" => {
            Some(Color::LightMagenta)
        }
        "lightcyan" | "brightcyan" | "lightteal" | "brightteal" => Some(Color::LightCyan),
        "lightgrey" | "lightgray" => Some(Color::Gray),
        _ => {
            if let Some(hex) = lower.strip_prefix('#') {
                return parse_hex_color(hex);
            }
            if let Some(rgb) = lower.strip_prefix("rgb(") {
                return parse_rgb_function(rgb);
            }
            None
        }
    }
}

fn parse_hex_color(hex: &str) -> Option<Color> {
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

fn parse_rgb_function(value: &str) -> Option<Color> {
    let trimmed = value.trim_end_matches(')');
    let components: Vec<&str> = trimmed.split(',').collect();
    if components.len() != 3 {
        return None;
    }
    let parse_component = |part: &str| -> Option<u8> {
        let v = part.trim();
        if let Some(percent) = v.strip_suffix('%') {
            let percentage: f32 = percent.parse().ok()?;
            let clamped = percentage.clamp(0.0, 100.0);
            Some((clamped / 100.0 * 255.0).round() as u8)
        } else {
            v.parse().ok()
        }
    };
    let r = parse_component(components[0])?;
    let g = parse_component(components[1])?;
    let b = parse_component(components[2])?;
    Some(Color::Rgb(r, g, b))
}

fn default_color(suit: Suit) -> Color {
    match suit {
        Suit::Hearts => Color::Red,
        Suit::Diamonds => Color::Magenta,
        Suit::Clubs => Color::Green,
        Suit::Spades => Color::Blue,
    }
}

fn suit_label(suit: Suit) -> &'static str {
    match suit {
        Suit::Hearts => "hearts",
        Suit::Diamonds => "diamonds",
        Suit::Clubs => "clubs",
        Suit::Spades => "spades",
    }
}

fn default_persist_stats() -> bool {
    false
}

fn default_auto_brackets() -> bool {
    true
}
