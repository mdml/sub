//! Harness selection for the behavioral contract suite.

use std::env;
use std::path::PathBuf;

use sub_sdk::acp::HarnessLaunch;

use super::fake_binary;

/// Fake-harness scenarios used by the contract suite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeScenario {
    ReplayMinimal,
    ReplayCodex,
    Hang,
    DieMidStream,
    IgnoreCancel,
    CancelHonored,
    Malformed,
    PermissionRequest,
    ResumeRefused,
}

impl FakeScenario {
    fn name(self) -> &'static str {
        match self {
            Self::ReplayMinimal => "replay-minimal",
            Self::ReplayCodex => "replay-codex",
            Self::Hang => "hang",
            Self::DieMidStream => "die-mid-stream",
            Self::IgnoreCancel => "ignore-cancel",
            Self::CancelHonored => "cancel_honored",
            Self::Malformed => "malformed",
            Self::PermissionRequest => "permission-request",
            Self::ResumeRefused => "resume-refused",
        }
    }
}

/// Which harness the contract suite drives for this run.
#[derive(Debug, Clone)]
pub enum ContractHarness {
    Fake(FakeScenario),
    Real {
        #[allow(dead_code)]
        name: String,
        launch: HarnessLaunch,
    },
}

impl ContractHarness {
    /// Select fake or real harness based on the environment.
    #[must_use]
    pub fn select(default_fake: FakeScenario) -> Self {
        match env::var("SUB_CONTRACT_REAL_HARNESS") {
            Ok(name) if !name.is_empty() => Self::Real {
                name: name.clone(),
                launch: real_launch(&name),
            },
            _ => Self::Fake(default_fake),
        }
    }

    /// Command the SDK should spawn.
    #[must_use]
    pub fn launch(&self) -> HarnessLaunch {
        match self {
            Self::Fake(scenario) => fake_launch(*scenario),
            Self::Real { launch, .. } => launch.clone(),
        }
    }

    /// Real harness name, absent for a fake scenario.
    #[must_use]
    pub fn real_name(&self) -> Option<&str> {
        match self {
            Self::Fake(_) => None,
            Self::Real { name, .. } => Some(name),
        }
    }
}

/// Whether real-harness mode is enabled for this process.
#[must_use]
pub fn real_harness_enabled() -> bool {
    env::var("SUB_CONTRACT_REAL_HARNESS").is_ok_and(|value| !value.is_empty())
}

fn fake_launch(scenario: FakeScenario) -> HarnessLaunch {
    let binary = fake_binary::fake_binary();
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sub-harness-fake/fixtures");
    let scenarios = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sub-harness-fake/scenarios");

    HarnessLaunch::new(binary)
        .arg(scenario.name())
        .env(
            "SUB_FAKE_FIXTURES_DIR",
            fixtures.to_string_lossy().into_owned(),
        )
        .env(
            "SUB_FAKE_SCENARIOS_DIR",
            scenarios.to_string_lossy().into_owned(),
        )
}

fn real_launch(name: &str) -> HarnessLaunch {
    if let Ok(command) = env::var("SUB_CONTRACT_HARNESS_CMD")
        && !command.is_empty()
    {
        return parse_command_line(&command);
    }

    match name {
        "cursor-agent" => HarnessLaunch::new("cursor-agent").arg("acp"),
        "claude" | "codex" => panic!(
            "set SUB_CONTRACT_HARNESS_CMD to the bridge path printed by `sub bridge install {name}`"
        ),
        other => panic!("unknown SUB_CONTRACT_REAL_HARNESS={other:?}"),
    }
}

fn parse_command_line(command: &str) -> HarnessLaunch {
    let parts = shell_words::split(command)
        .unwrap_or_else(|error| panic!("parse SUB_CONTRACT_HARNESS_CMD: {error}"));
    let mut iter = parts.into_iter();
    let command = iter.next().unwrap_or_else(|| panic!("harness command"));
    let mut launch = HarnessLaunch::new(command);
    for arg in iter {
        launch = launch.arg(arg);
    }
    launch
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_scenario_names_match_files() {
        assert_eq!(FakeScenario::ReplayMinimal.name(), "replay-minimal");
        assert_eq!(FakeScenario::Malformed.name(), "malformed");
    }

    #[test]
    fn parse_command_line_splits_args() {
        let launch = parse_command_line("npx --yes pkg@1.0.0");
        assert_eq!(launch.command(), PathBuf::from("npx"));
        assert_eq!(launch.args(), &["--yes".to_owned(), "pkg@1.0.0".to_owned()]);
    }
}
