use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::game::Scoreboard;

#[derive(Debug, Clone)]
pub struct Paths {
    config_file: PathBuf,
    session_file: PathBuf,
}

impl Paths {
    pub fn new() -> Result<Self> {
        let root = resolve_app_root()?;
        let config_file = root.join("config.toml");
        let session_file = root.join("session.json");
        Ok(Self {
            config_file,
            session_file,
        })
    }

    pub fn config_file(&self) -> &Path {
        &self.config_file
    }

    pub fn session_file(&self) -> &Path {
        &self.session_file
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionData {
    pub scoreboard: Scoreboard,
    #[serde(default)]
    pub round_history: Vec<RoundSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoundSummary {
    pub round_number: u32,
    pub description: String,
}

impl SessionData {
    pub fn new(scoreboard: Scoreboard, round_history: Vec<RoundSummary>) -> Self {
        Self {
            scoreboard,
            round_history,
        }
    }
}

pub fn load_session(path: &Path) -> Result<Option<SessionData>> {
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read session data at {}", path.display()))?;
    let session = serde_json::from_str::<SessionData>(&contents)
        .with_context(|| format!("failed to parse session data at {}", path.display()))?;
    Ok(Some(session))
}

pub fn save_session(path: &Path, data: &SessionData) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("failed to ensure session directory at {}", parent.display())
        })?;
    }
    let contents =
        serde_json::to_string_pretty(data).context("failed to serialise session data")?;
    fs::write(path, contents)
        .with_context(|| format!("failed to write session data to {}", path.display()))
}

fn resolve_app_root() -> Result<PathBuf> {
    if let Some(mut dir) = dirs::config_dir() {
        dir.push("deadwood");
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create config directory at {}", dir.display()))?;
        return Ok(dir);
    }

    let mut dir = env::current_dir().context("failed to resolve current directory")?;
    dir.push(".deadwood");
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create fallback directory at {}", dir.display()))?;
    Ok(dir)
}
