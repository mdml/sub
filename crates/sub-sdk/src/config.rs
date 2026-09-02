//! Discovery and parsing for the beta-minimum `sub.toml` configuration.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::delegation::Harness;

/// Configuration for one supported harness.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessConfig {
    /// Absolute or user-selected path to the harness executable.
    pub binary: PathBuf,
    /// Harness-native default model.
    pub model: Option<String>,
    /// Harness-native default permission mode.
    pub permission_mode: Option<String>,
}

/// Supported harness entries in `sub.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessConfigs {
    /// Claude Code configuration.
    pub claude: Option<HarnessConfig>,
    /// Codex configuration.
    pub codex: Option<HarnessConfig>,
    /// Cursor Agent configuration.
    pub cursor: Option<HarnessConfig>,
}

/// The complete beta-minimum `sub.toml` shape.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SubConfig {
    /// Override for the implementation-private state directory.
    pub state_dir: Option<PathBuf>,
    /// Per-harness launch defaults.
    pub harnesses: HarnessConfigs,
}

impl SubConfig {
    /// Return the entry for a supported harness.
    #[must_use]
    pub const fn harness(&self, harness: Harness) -> Option<&HarnessConfig> {
        match harness {
            Harness::Claude => self.harnesses.claude.as_ref(),
            Harness::Codex => self.harnesses.codex.as_ref(),
            Harness::CursorAgent => self.harnesses.cursor.as_ref(),
        }
    }
}

/// A parsed configuration and the path at which it was sought.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    /// Parsed configuration, or an empty configuration when the file is absent.
    pub config: SubConfig,
    /// Discovered configuration path.
    pub path: PathBuf,
    /// Whether a file existed at the discovered path.
    pub exists: bool,
}

/// Configuration discovery or parsing failure.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// No explicit path was supplied and no user configuration home can be found.
    #[error("cannot locate sub.toml: HOME and XDG_CONFIG_HOME are unset; set SUB_CONFIG")]
    NoConfigHome,
    /// Reading the selected file failed.
    #[error("cannot read {path}: {source}")]
    Read {
        /// Selected configuration path.
        path: PathBuf,
        /// Filesystem error.
        source: io::Error,
    },
    /// Parsing the selected file failed.
    #[error("cannot parse {path}: {source}")]
    Parse {
        /// Selected configuration path.
        path: PathBuf,
        /// TOML parse error.
        source: toml::de::Error,
    },
}

/// Discover and load `sub.toml` from the process environment.
///
/// `SUB_CONFIG` selects an exact path. Otherwise discovery uses
/// `$XDG_CONFIG_HOME/sub/sub.toml`, falling back to `$HOME/.config/sub/sub.toml`.
/// A missing selected file is an empty configuration.
///
/// # Errors
///
/// Returns an error when no path can be discovered, or an existing file cannot
/// be read or parsed.
pub fn load() -> Result<LoadedConfig, ConfigError> {
    load_from(
        env::var_os("SUB_CONFIG").as_deref(),
        env::var_os("XDG_CONFIG_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
    )
}

/// Discover and load configuration from explicit environment values.
///
/// This pure discovery boundary lets tests exercise every location without
/// changing process-global environment variables.
///
/// # Errors
///
/// Returns [`ConfigError`] under the same conditions as [`load`].
pub fn load_from(
    override_path: Option<&std::ffi::OsStr>,
    xdg_config_home: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
) -> Result<LoadedConfig, ConfigError> {
    let path = discover_path(override_path, xdg_config_home, home)?;
    load_path(path)
}

fn discover_path(
    override_path: Option<&std::ffi::OsStr>,
    xdg_config_home: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
) -> Result<PathBuf, ConfigError> {
    if let Some(path) = override_path {
        return Ok(PathBuf::from(path));
    }
    if let Some(root) = xdg_config_home {
        return Ok(Path::new(root).join("sub/sub.toml"));
    }
    home.map(|root| Path::new(root).join(".config/sub/sub.toml"))
        .ok_or(ConfigError::NoConfigHome)
}

fn load_path(path: PathBuf) -> Result<LoadedConfig, ConfigError> {
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(LoadedConfig {
                config: SubConfig::default(),
                path,
                exists: false,
            });
        }
        Err(source) => return Err(ConfigError::Read { path, source }),
    };
    let config = toml::from_str(&contents).map_err(|source| ConfigError::Parse {
        path: path.clone(),
        source,
    })?;
    Ok(LoadedConfig {
        config,
        path,
        exists: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_precedence_and_missing_file() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let explicit = root.path().join("chosen.toml");
        let loaded = load_from(
            Some(explicit.as_os_str()),
            Some(root.path().as_os_str()),
            Some(root.path().as_os_str()),
        )
        .unwrap_or_else(|error| panic!("load: {error}"));
        assert_eq!(loaded.path, explicit);
        assert!(!loaded.exists);

        let xdg = load_from(None, Some(root.path().as_os_str()), None)
            .unwrap_or_else(|error| panic!("xdg: {error}"));
        assert_eq!(xdg.path, root.path().join("sub/sub.toml"));
        let home = load_from(None, None, Some(root.path().as_os_str()))
            .unwrap_or_else(|error| panic!("home: {error}"));
        assert_eq!(home.path, root.path().join(".config/sub/sub.toml"));
        assert!(matches!(
            load_from(None, None, None),
            Err(ConfigError::NoConfigHome)
        ));
    }

    #[test]
    fn parses_only_beta_minimum_shape() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let path = root.path().join("sub.toml");
        fs::write(
            &path,
            "state_dir = '/tmp/state'\n[harnesses.codex]\nbinary = '/bin/codex'\nmodel = 'gpt-test'\npermission_mode = 'agent'\n[harnesses.cursor]\nbinary = '/bin/cursor-agent'\npermission_mode = 'agent'\n",
        )
        .unwrap_or_else(|error| panic!("write: {error}"));
        let loaded = load_from(Some(path.as_os_str()), None, None)
            .unwrap_or_else(|error| panic!("load: {error}"));
        let codex = loaded
            .config
            .harness(Harness::Codex)
            .unwrap_or_else(|| panic!("codex"));
        assert_eq!(codex.binary, PathBuf::from("/bin/codex"));
        assert_eq!(codex.model.as_deref(), Some("gpt-test"));
        assert_eq!(
            loaded
                .config
                .harness(Harness::CursorAgent)
                .map(|entry| entry.binary.as_path()),
            Some(Path::new("/bin/cursor-agent"))
        );
        assert_eq!(loaded.config.state_dir, Some(PathBuf::from("/tmp/state")));

        fs::write(&path, "budgets = {}\n").unwrap_or_else(|error| panic!("write: {error}"));
        assert!(matches!(
            load_from(Some(path.as_os_str()), None, None),
            Err(ConfigError::Parse { .. })
        ));

        fs::remove_file(&path).unwrap_or_else(|error| panic!("remove: {error}"));
        fs::create_dir_all(&path).unwrap_or_else(|error| panic!("directory: {error}"));
        assert!(matches!(
            load_from(Some(path.as_os_str()), None, None),
            Err(ConfigError::Read { .. })
        ));
    }
}
