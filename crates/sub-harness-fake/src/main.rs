//! Fake ACP harness process for tests and local development.

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    if let Err(error) = sub_harness_fake::run_from_env().await {
        eprintln!("sub-harness-fake: {error}");
        std::process::exit(1);
    }
}
