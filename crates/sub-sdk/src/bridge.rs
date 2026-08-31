//! Pinned npm bridge installation and integrity verification.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Immutable identity and executable name for one npm ACP bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeSpec {
    /// npm package name.
    pub package: &'static str,
    /// Exact npm package version.
    pub version: &'static str,
    /// Executable installed in `node_modules/.bin`.
    pub binary: &'static str,
    /// Harness name used by the install command.
    pub harness: &'static str,
}

/// Manifest written after a complete bridge install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeManifest {
    /// npm package name.
    pub package: String,
    /// Exact npm package version.
    pub version: String,
    /// Unix timestamp at installation.
    pub installed_at_unix_seconds: u64,
    /// SHA-256 of relative paths, file bytes, and symlink targets in the installed tree.
    pub integrity_sha256: String,
}

/// Bridge installation failure.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    /// A filesystem operation failed.
    #[error("bridge filesystem operation failed: {0}")]
    Io(#[from] io::Error),
    /// npm returned failure.
    #[error("npm failed to install {package}@{version} (exit {status})")]
    Npm {
        /// Package being installed.
        package: String,
        /// Version being installed.
        version: String,
        /// Process status.
        status: std::process::ExitStatus,
    },
    /// The stored manifest is invalid.
    #[error("bridge manifest is invalid: {0}")]
    Manifest(#[from] serde_json::Error),
    /// The bridge is absent or does not match its declared pin.
    #[error("{reason}; run `sub bridge install {harness}`")]
    NotInstalled {
        /// Harness whose bridge is required.
        harness: String,
        /// Validation failure.
        reason: String,
    },
}

/// Install a pinned npm bridge under the state directory.
///
/// # Errors
///
/// Returns an error when npm or an atomic filesystem operation fails.
pub fn install(state_dir: &Path, spec: BridgeSpec) -> Result<PathBuf, BridgeError> {
    install_with_npm(state_dir, spec, Path::new("npm"))
}

fn install_with_npm(
    state_dir: &Path,
    spec: BridgeSpec,
    npm: &Path,
) -> Result<PathBuf, BridgeError> {
    let root = bridge_dir(state_dir, spec);
    let parent = root.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "bridge directory has no parent",
        )
    })?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".install-{}-{}", spec.version, std::process::id()));
    if temp.exists() {
        fs::remove_dir_all(&temp)?;
    }
    fs::create_dir_all(&temp)?;

    let package_pin = format!("{}@{}", spec.package, spec.version);
    let status = Command::new(npm)
        .args([
            "install",
            "--prefix",
            temp.to_string_lossy().as_ref(),
            "--ignore-scripts",
            "--no-package-lock",
            "--no-save",
            &package_pin,
        ])
        .status()?;
    if !status.success() {
        return Err(BridgeError::Npm {
            package: spec.package.to_owned(),
            version: spec.version.to_owned(),
            status,
        });
    }

    let manifest = BridgeManifest {
        package: spec.package.to_owned(),
        version: spec.version.to_owned(),
        installed_at_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        integrity_sha256: hash_tree(&temp)?,
    };
    fs::write(
        temp.join("sub-bridge-manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::rename(&temp, &root)?;
    verify(state_dir, spec)
}

/// Resolve a bridge executable only when its manifest and integrity hash match.
///
/// # Errors
///
/// Returns [`BridgeError::NotInstalled`] with the exact repair command for any mismatch.
pub fn verify(state_dir: &Path, spec: BridgeSpec) -> Result<PathBuf, BridgeError> {
    let root = bridge_dir(state_dir, spec);
    let manifest_path = root.join("sub-bridge-manifest.json");
    let fail = |reason: String| BridgeError::NotInstalled {
        harness: spec.harness.to_owned(),
        reason,
    };
    let bytes = fs::read(&manifest_path)
        .map_err(|error| fail(format!("bridge manifest missing: {error}")))?;
    let manifest: BridgeManifest =
        serde_json::from_slice(&bytes).map_err(|error| fail(error.to_string()))?;
    if manifest.package != spec.package || manifest.version != spec.version {
        return Err(fail(format!(
            "bridge pin mismatch (found {}@{}, require {}@{})",
            manifest.package, manifest.version, spec.package, spec.version
        )));
    }
    let actual = hash_tree(&root).map_err(|error| fail(error.to_string()))?;
    if actual != manifest.integrity_sha256 {
        return Err(fail("bridge integrity hash mismatch".to_owned()));
    }
    let binary = root.join("node_modules/.bin").join(spec.binary);
    if !binary.is_file() {
        return Err(fail(format!(
            "bridge executable missing: {}",
            binary.display()
        )));
    }
    Ok(binary)
}

fn bridge_dir(state_dir: &Path, spec: BridgeSpec) -> PathBuf {
    let package_dir = spec.package.replace(['@', '/'], "_");
    state_dir
        .join("bridges")
        .join(package_dir)
        .join(spec.version)
}

fn hash_tree(root: &Path) -> io::Result<String> {
    let mut files = Vec::new();
    collect_entries(root, &mut files)?;
    files.sort();
    let mut digest = Sha256::new();
    for path in files {
        let relative = path.strip_prefix(root).map_err(io::Error::other)?;
        if relative == Path::new("sub-bridge-manifest.json") {
            continue;
        }
        digest.update(relative.to_string_lossy().as_bytes());
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            digest.update(b"symlink\0");
            digest.update(fs::read_link(&path)?.to_string_lossy().as_bytes());
        } else if metadata.is_file() {
            digest.update(b"file\0");
            let mut file = fs::File::open(path)?;
            let mut buffer = [0_u8; 16 * 1024];
            loop {
                let count = file.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                digest.update(&buffer[..count]);
            }
        }
    }
    Ok(hex::encode(digest.finalize()))
}

fn collect_entries(directory: &Path, entries: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.is_dir() {
            collect_entries(&path, entries)?;
        } else {
            entries.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: BridgeSpec = BridgeSpec {
        package: "@example/bridge",
        version: "1.2.3",
        binary: "example",
        harness: "example",
    };

    #[test]
    fn verify_missing_names_install_action() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let error = verify(root.path(), SPEC)
            .err()
            .unwrap_or_else(|| panic!("missing bridge should fail"));
        assert!(error.to_string().contains("sub bridge install example"));
    }

    #[test]
    fn tree_hash_changes_with_content() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        fs::write(root.path().join("file"), "one").unwrap_or_else(|error| panic!("write: {error}"));
        let first = hash_tree(root.path()).unwrap_or_else(|error| panic!("hash: {error}"));
        fs::write(root.path().join("file"), "two").unwrap_or_else(|error| panic!("write: {error}"));
        let second = hash_tree(root.path()).unwrap_or_else(|error| panic!("hash: {error}"));
        assert_ne!(first, second);
    }

    #[cfg(unix)]
    #[test]
    fn fake_npm_install_writes_and_verifies_manifest() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let npm = root.path().join("npm");
        fs::write(
            &npm,
            "#!/bin/sh\nwhile [ \"$1\" != \"--prefix\" ]; do shift; done\nshift\nprefix=$1\nmkdir -p \"$prefix/node_modules/.bin\"\nprintf '#!/bin/sh\\n' > \"$prefix/node_modules/.bin/example\"\nchmod +x \"$prefix/node_modules/.bin/example\"\n",
        ).unwrap_or_else(|error| panic!("script: {error}"));
        fs::set_permissions(&npm, fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|error| panic!("permissions: {error}"));
        let stale = bridge_dir(root.path(), SPEC)
            .parent()
            .unwrap_or_else(|| panic!("bridge parent"))
            .join(format!(".install-{}-{}", SPEC.version, std::process::id()));
        fs::create_dir_all(&stale).unwrap_or_else(|error| panic!("stale: {error}"));
        let binary = install_with_npm(root.path(), SPEC, &npm)
            .unwrap_or_else(|error| panic!("install: {error}"));
        assert!(binary.is_file());
        assert_eq!(
            verify(root.path(), SPEC).unwrap_or_else(|error| panic!("verify: {error}")),
            binary
        );
        fs::write(&binary, "tampered").unwrap_or_else(|error| panic!("tamper: {error}"));
        assert!(matches!(
            verify(root.path(), SPEC),
            Err(BridgeError::NotInstalled { .. })
        ));
        assert!(install_with_npm(root.path(), SPEC, &npm).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn fake_npm_failure_is_reported() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let npm = root.path().join("npm-fail");
        fs::write(&npm, "#!/bin/sh\nexit 9\n").unwrap_or_else(|error| panic!("script: {error}"));
        fs::set_permissions(&npm, fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|error| panic!("permissions: {error}"));
        assert!(matches!(
            install_with_npm(root.path(), SPEC, &npm),
            Err(BridgeError::Npm { .. })
        ));
    }

    #[test]
    fn verify_rejects_pin_and_binary_mismatches() {
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let install = bridge_dir(root.path(), SPEC);
        fs::create_dir_all(&install).unwrap_or_else(|error| panic!("mkdir: {error}"));
        let wrong = BridgeManifest {
            package: "wrong".to_owned(),
            version: "0".to_owned(),
            installed_at_unix_seconds: 0,
            integrity_sha256: String::new(),
        };
        fs::write(
            install.join("sub-bridge-manifest.json"),
            serde_json::to_vec(&wrong).unwrap_or_else(|error| panic!("json: {error}")),
        )
        .unwrap_or_else(|error| panic!("write: {error}"));
        assert!(
            verify(root.path(), SPEC)
                .err()
                .unwrap_or_else(|| panic!("pin error"))
                .to_string()
                .contains("pin mismatch")
        );

        let valid = BridgeManifest {
            package: SPEC.package.to_owned(),
            version: SPEC.version.to_owned(),
            installed_at_unix_seconds: 0,
            integrity_sha256: hash_tree(&install).unwrap_or_else(|error| panic!("hash: {error}")),
        };
        fs::write(
            install.join("sub-bridge-manifest.json"),
            serde_json::to_vec(&valid).unwrap_or_else(|error| panic!("json: {error}")),
        )
        .unwrap_or_else(|error| panic!("write: {error}"));
        assert!(
            verify(root.path(), SPEC)
                .err()
                .unwrap_or_else(|| panic!("binary error"))
                .to_string()
                .contains("executable missing")
        );
    }

    #[cfg(unix)]
    #[test]
    fn tree_hash_includes_symlink_targets() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        symlink("first", root.path().join("link"))
            .unwrap_or_else(|error| panic!("symlink: {error}"));
        let first = hash_tree(root.path()).unwrap_or_else(|error| panic!("hash: {error}"));
        fs::remove_file(root.path().join("link")).unwrap_or_else(|error| panic!("remove: {error}"));
        symlink("second", root.path().join("link"))
            .unwrap_or_else(|error| panic!("symlink: {error}"));
        assert_ne!(
            first,
            hash_tree(root.path()).unwrap_or_else(|error| panic!("hash: {error}"))
        );
    }
}
