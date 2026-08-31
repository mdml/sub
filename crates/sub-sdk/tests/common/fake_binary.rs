//! Resolve the path to the `sub-harness-fake` test binary.

use std::env;
use std::path::{Path, PathBuf};

/// Path to the `sub-harness-fake` binary for integration tests.
///
/// # Panics
///
/// Panics when the binary cannot be located. Run `cargo build -p sub-harness-fake` first.
#[must_use]
pub fn fake_binary() -> PathBuf {
    if let Ok(path) = env::var("CARGO_BIN_EXE_sub_harness_fake") {
        return PathBuf::from(path);
    }

    if let Ok(exe) = env::current_exe()
        && let Some(path) = fake_binary_near_test_exe(&exe)
    {
        return path;
    }

    let workspace =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/sub-harness-fake");
    if workspace.is_file() {
        return workspace;
    }

    panic!("sub-harness-fake binary not found; run `cargo build -p sub-harness-fake`");
}

fn fake_binary_near_test_exe(exe: &Path) -> Option<PathBuf> {
    let debug_dir = exe.parent()?.parent()?;
    let direct = debug_dir.join("sub-harness-fake");
    if direct.is_file() {
        return Some(direct);
    }

    // `cargo llvm-cov` runs tests from `llvm-cov-target/debug/deps/` but builds
    // dependency binaries under `cargo-target/debug/`.
    let sibling = debug_dir
        .parent()?
        .parent()?
        .join("debug")
        .join("sub-harness-fake");
    if sibling.is_file() {
        return Some(sibling);
    }

    None
}
