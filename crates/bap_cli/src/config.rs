//! `bap.json` project configuration.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Per-service runtime config; `package.json` stays untouched.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BapConfig {
    /// Declares intent only today; the value is validated but advisory.
    #[serde(default)]
    pub runtime: Option<String>,
    /// Pins `.js` classification for the whole project.
    #[serde(default)]
    pub module_format: Option<ModuleFormat>,
    /// Bare specifier -> local file, resolved from the config directory.
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

/// The module format a project pins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleFormat {
    Esm,
    Cjs,
}

impl BapConfig {
    /// Loads `bap.json` from `directory`, returning the default when absent.
    pub fn load(directory: &Path) -> Result<Self, String> {
        let path = directory.join("bap.json");
        if !path.is_file() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        serde_json::from_str(&text).map_err(|error| format!("invalid {}: {error}", path.display()))
    }

    /// The directory whose relative alias targets resolve against.
    pub fn base_directory(&self) -> PathBuf {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
}
