//! Unit tests for the shared ACP client layer.

#[path = "common/fake_binary.rs"]
mod fake_binary;

use std::time::Duration;

use fake_binary::fake_binary;

use sub_harness_fake::{FixtureSource, LoadedFixture, Scenario, ScenarioBehavior};
use sub_sdk::acp::{
    AcpClient, AcpClientConfig, AcpError, HarnessLaunch, PromptOptions, StopReason,
};
#[path = "acp_client/acp_client_config_defaults.rs"]
mod acp_client_config_defaults;
#[path = "acp_client/codex_fixture_is_recorded.rs"]
mod codex_fixture_is_recorded;
#[path = "acp_client/fake_client_prompt_turn_minimal.rs"]
mod fake_client_prompt_turn_minimal;
#[path = "acp_client/hang_scenario_deserializes.rs"]
mod hang_scenario_deserializes;
#[path = "acp_client/harness_launch_builder.rs"]
mod harness_launch_builder;
#[path = "acp_client/minimal_fixture_loads.rs"]
mod minimal_fixture_loads;
#[path = "acp_client/missing_agent_binary_fails.rs"]
mod missing_agent_binary_fails;
#[path = "acp_client/prompt_turn_times_out_on_hang.rs"]
mod prompt_turn_times_out_on_hang;
#[path = "acp_client/prompt_turn_without_timeout.rs"]
mod prompt_turn_without_timeout;
