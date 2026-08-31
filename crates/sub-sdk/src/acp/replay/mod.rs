//! Fake-harness replay support in the shared ACP layer.
//!
//! Adapters must not use this module; it exists so `sub-harness-fake` can speak
//! ACP v1 over stdio without taking a direct dependency on ACP schema crates.

mod fixture;
mod scenario;
mod server;

pub use fixture::{FixtureManifest, FixtureSource, LoadedFixture, RecordedEvent};
pub use scenario::{Scenario, ScenarioBehavior};
pub use server::run_stdio;
