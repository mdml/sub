use super::*;

/// Linux refuses to exec a file while any process holds it open for writing (`ETXTBSY`). A fixture written in this multi-threaded test process can leak its write descriptor into a child that a sibling test thread is forking, so fixtures must be executable as soon as the helper returns even while other threads spawn.
#[cfg(unix)]
#[test]
fn fixture_executables_run_while_sibling_threads_spawn() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let stop = Arc::new(AtomicBool::new(false));
    let spawners: Vec<_> = (0..4)
        .map(|_| {
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let _ = Command::new("true").status();
                }
            })
        })
        .collect();
    let outcome = std::panic::catch_unwind(|| {
        for index in 0..100 {
            let probe = root.path().join(format!("probe-{index}"));
            write_executable(&probe, "#!/bin/sh\nexit 0\n");
            let status = Command::new(&probe)
                .status()
                .unwrap_or_else(|error| panic!("exec fixture {index}: {error}"));
            assert!(status.success());
        }
    });
    stop.store(true, Ordering::Relaxed);
    for spawner in spawners {
        spawner
            .join()
            .unwrap_or_else(|_| panic!("spawner thread panicked"));
    }
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}
