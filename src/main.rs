use mcp_deepresearch::{
    codex::CodexRuntime,
    controller::Controller,
    research::*,
    runtime::{self, FixtureRuntime},
    workspace::{Workspace, WorkspaceStore},
};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 || !matches!(args[1].as_str(), "fixture" | "live") {
        eprintln!("Usage: mcp-deepresearch <fixture|live> <request.json> <workspace-directory>");
        std::process::exit(2);
    }
    let request: ResearchRequest = serde_json::from_slice(&std::fs::read(&args[2])?)?;
    let workspace = Workspace::new("local-operator".into(), request)?;
    let mut controller = Controller::new(workspace, runtime::unix_seconds());
    let cancel = CancellationToken::new();
    let signal = cancel.clone();
    let signal_task = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            signal.cancel();
        }
    });
    if args[1] == "live" {
        let worker =
            CodexRuntime::from_environment(std::path::Path::new(&args[3]).join("workers"))?;
        runtime::run(&worker, &mut controller, cancel).await;
    } else {
        runtime::run(&FixtureRuntime::default(), &mut controller, cancel).await;
    }
    signal_task.abort();
    WorkspaceStore::new(&args[3])?.save(&controller.workspace)?;
    println!("{}", controller.workspace.report());
    if controller.workspace.status != Status::Completed {
        std::process::exit(1);
    }
    Ok(())
}
