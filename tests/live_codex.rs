//! Opt-in provider compatibility check. This consumes an authenticated account.
use mcp_deepresearch::{
    codex::{CodexConfig, CodexRuntime},
    controller::Controller,
    research::*,
    runtime::{self, AgentRuntime},
    sources::SourceConfig,
    workspace::Workspace,
};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::*,
    service::RequestContext,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct Source(Arc<AtomicU32>);
impl ServerHandler for Source {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::V_2026_07_28)
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(vec![serde_json::from_value(json!({"name":"read_fixture","description":"Read the invented fixture observation for this compatibility test.","inputSchema":{"type":"object","properties":{},"additionalProperties":false},"annotations":{"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}})).unwrap()]))
    }
    async fn call_tool(
        &self,
        _: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(CallToolResult::structured(json!({"url":"https://example.org/fixture-observation","title":"Invented observation","text":"The fixture observation contains exactly 17 blue stones. This is invented test data, not an observation about the real world."})).into())
    }
}

#[tokio::test]
#[ignore = "consumes a real Codex account; set DEEPRESEARCH_CODEX_HOME and optionally DEEPRESEARCH_MODEL"]
async fn actual_codex_reads_current_mcp_and_returns_structured_evidence() {
    let home =
        std::env::var_os("DEEPRESEARCH_CODEX_HOME").expect("explicit authenticated home required");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/mcp", listener.local_addr().unwrap());
    let calls = Arc::new(AtomicU32::new(0));
    let source = Source(calls.clone());
    let stop = CancellationToken::new();
    let service = StreamableHttpService::new(
        move || Ok(source.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true)
            .with_cancellation_token(stop.clone()),
    );
    let server = tokio::spawn(
        axum::serve(listener, axum::Router::new().nest_service("/mcp", service))
            .with_graceful_shutdown(stop.clone().cancelled_owned())
            .into_future(),
    );
    let root = std::env::temp_dir().join(format!(
        "research-live-compatibility-{}",
        ResearchId::default()
    ));
    let worker = CodexRuntime::new(CodexConfig {
        executable: std::env::var_os("DEEPRESEARCH_CODEX_EXECUTABLE")
            .unwrap_or_else(|| "codex".into())
            .into(),
        auth_home: home.into(),
        work_root: root.join("workers"),
        sources: Some(SourceConfig {
            endpoint,
            token: None,
            tools: vec!["read_fixture".into()],
            file_origins: Vec::new(),
            local_materials: Vec::new(),
            trace_context: TraceContext::default(),
        }),
        model: std::env::var("DEEPRESEARCH_MODEL").ok(),
        reasoning_effort: None,
        max_workers: 1,
    })
    .unwrap();
    let request = serde_json::from_value(json!({"objective":"Read the fixture tool and report how many blue stones it describes. Clearly label this invented test data. Cite the retrieved source.","limits":{"wall_seconds":120,"max_tool_calls":4}})).unwrap();
    let mut controller = Controller::new(
        Workspace::new("test".into(), request).unwrap(),
        runtime::unix_seconds(),
    );
    let mut assignment = controller.assignment(runtime::unix_seconds()).unwrap();
    assignment.kind = AssignmentKind::CompleteResearch;
    assignment.focus = "Read the tool and return the final cited answer in draft, with the actual source excerpt, in this one session.".into();
    let outcome = worker.execute(assignment, CancellationToken::new()).await;
    worker.shutdown().await.unwrap();
    stop.cancel();
    server.await.unwrap().unwrap();
    let result = outcome
        .expect("provider compatibility check failed; no raw credential-bearing logs are retained");
    assert!(
        calls.load(Ordering::SeqCst) > 0,
        "model did not call the source tool"
    );
    assert!(result.draft.as_ref().is_some_and(|d| d.contains("17")));
    controller.workspace.apply(result).unwrap();
    assert!(
        controller
            .workspace
            .sources
            .values()
            .any(|s| s.excerpt.contains("17"))
    );
    std::fs::remove_dir_all(root).unwrap();
}
