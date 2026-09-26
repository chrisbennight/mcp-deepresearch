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
    if args.len() == 4 && args[1] == "serve" {
        return serve(&args[2], std::path::Path::new(&args[3])).await;
    }
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

async fn serve(mode: &str, root: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    use mcp_deepresearch::{
        lifecycle::{Assignments, Research, Worker},
        mcp::{self, ServiceConfig},
    };
    use restate_sdk::prelude::{Endpoint, HttpServer};
    use std::time::Duration;
    let worker = match mode {
        "fixture" => Worker::Fixture {
            delay: Duration::from_millis(
                std::env::var("DEEPRESEARCH_FIXTURE_DELAY_MS")
                    .unwrap_or_else(|_| "0".into())
                    .parse()?,
            ),
            clarification: std::env::var("DEEPRESEARCH_FIXTURE_CLARIFICATION").as_deref()
                == Ok("true"),
        },
        "live" => Worker::Codex(CodexRuntime::from_environment(root.join("workers"))?),
        _ => return Err("serve mode must be fixture or live".into()),
    };
    let config = ServiceConfig {
        restate_ingress: std::env::var("DEEPRESEARCH_RESTATE_INGRESS")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".into()),
        principal: std::env::var("DEEPRESEARCH_PRINCIPAL")?,
        bearer_current: std::env::var("DEEPRESEARCH_INBOUND_TOKEN")?,
        bearer_previous: std::env::var("DEEPRESEARCH_INBOUND_TOKEN_PREVIOUS").ok(),
        allowed_hosts: std::env::var("DEEPRESEARCH_ALLOWED_HOSTS")
            .unwrap_or_else(|_| "localhost,127.0.0.1".into())
            .split(',')
            .map(str::trim)
            .map(str::to_owned)
            .collect(),
    };
    let cancel = CancellationToken::new();
    let router = mcp::router(config, cancel.clone())?;
    let mcp_listener = tokio::net::TcpListener::bind(
        std::env::var("DEEPRESEARCH_MCP_LISTEN").unwrap_or_else(|_| "127.0.0.1:8088".into()),
    )
    .await?;
    let restate_listener = tokio::net::TcpListener::bind(
        std::env::var("DEEPRESEARCH_WORKFLOW_LISTEN").unwrap_or_else(|_| "127.0.0.1:9080".into()),
    )
    .await?;
    let endpoint = Endpoint::builder()
        .bind(Research::new(worker.clone()))
        .bind(Assignments::new(worker.clone()))
        .build();
    let stop = cancel.clone();
    let http = tokio::spawn(async move {
        axum::serve(mcp_listener, router)
            .with_graceful_shutdown(stop.cancelled_owned())
            .await
    });
    let stop = cancel.clone();
    let workflows = tokio::spawn(async move {
        HttpServer::new(endpoint)
            .serve_with_cancel(restate_listener, stop.cancelled_owned())
            .await;
    });
    let mut termination =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! { result=tokio::signal::ctrl_c()=>result?,_=termination.recv()=>() }
    cancel.cancel();
    if let Worker::Codex(worker) = worker {
        worker.shutdown().await?;
    }
    http.await??;
    workflows.await?;
    Ok(())
}
