use axum::{Router, routing::get};
use mcp_deepresearch::sources::{AssignmentSources, SourceConfig};
use rmcp::{
    ClientServiceExt, ErrorData, RoleServer, ServerHandler,
    model::*,
    service::{ClientLifecycleMode, RequestContext},
    transport::{
        StreamableHttpClientTransport,
        streamable_http_client::StreamableHttpClientTransportConfig,
        streamable_http_server::{
            StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
        },
    },
};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
use tokio_util::sync::CancellationToken;

const PAPER: &str = r#"{"url":"https://example.org/paper","text":"Actual retrieved text.","document":{"uri":"mcp-file://fixture/underlying-document"}}"#;

#[derive(Clone)]
struct Gateway {
    origin: String,
    calls: Arc<AtomicU32>,
    discovery: Option<Arc<tokio::sync::Notify>>,
}
impl ServerHandler for Gateway {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::V_2026_07_28)
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        if let Some(started) = &self.discovery {
            started.notify_one();
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
        let tools = ["search", "read", "admin"].map(|name| serde_json::from_value(json!({"name":name,"inputSchema":{"type":"object"},"annotations":{"readOnlyHint":name!="admin","destructiveHint":false,"idempotentHint":true,"openWorldHint":true}})).unwrap());
        Ok(ListToolsResult::with_all_items(tools.into()))
    }
    async fn call_tool(
        &self,
        params: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(
            context
                .meta
                .get("io.modelcontextprotocol/clientCapabilities")
                .unwrap()["files"]["download"],
            true
        );
        let value = match params.name.as_ref() {
            "search"
                if params
                    .arguments
                    .as_ref()
                    .and_then(|a| a.get("oversized"))
                    .and_then(serde_json::Value::as_bool)
                    == Some(true) =>
            {
                json!({"text":"x".repeat(16*1024*1024+1)})
            }
            "search" => {
                json!({"results":[{"url":"https://example.org/paper","snippet":"Search lead only"}]})
            }
            "read"
                if params
                    .arguments
                    .as_ref()
                    .and_then(|a| a.get("nested"))
                    .and_then(serde_json::Value::as_bool)
                    == Some(true) =>
            {
                json!({"documents":[{"file":{"uri":"mcp-file://fixture/nested"}}]})
            }
            "read" => {
                json!({"result":{"file":{"uri":"mcp-file://fixture/paper","size":PAPER.len()},"attribution":"Fixture document attribution."}})
            }
            _ => panic!("non-source operation escaped the allowlist"),
        };
        let mut result = CallToolResult::structured(value);
        if params.name == "search" {
            result.content.push(ContentBlock::text(
                "Distinct text supplied alongside structured metadata.",
            ));
        }
        Ok(result.into())
    }
    async fn on_custom_request(
        &self,
        request: CustomRequest,
        _: RequestContext<RoleServer>,
    ) -> Result<CustomResult, ErrorData> {
        assert_eq!(request.method, "files/authorizeDownload");
        Ok(CustomResult::new(
            json!({"file":{"uri":"mcp-file://fixture/paper","size":PAPER.len()},"download":{"transport":"http","method":"GET","url":format!("{}/paper",self.origin),"headers":{"x-transfer-test":"host-only"}}}),
        ))
    }
}

#[tokio::test]
async fn sources_use_current_discovery_host_file_transfer_and_call_budget() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let calls = Arc::new(AtomicU32::new(0));
    let gateway = Gateway {
        origin: origin.clone(),
        calls: calls.clone(),
        discovery: None,
    };
    let stop = CancellationToken::new();
    let service = StreamableHttpService::new(
        move || Ok(gateway.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true)
            .with_cancellation_token(stop.clone()),
    );
    let downloads = Arc::new(AtomicU32::new(0));
    let served = downloads.clone();
    let router = Router::new().nest_service("/mcp", service).route(
        "/paper",
        get(move |headers: axum::http::HeaderMap| {
            let served = served.clone();
            async move {
                assert_eq!(headers.get("x-transfer-test").unwrap(), "host-only");
                if served.fetch_add(1, Ordering::SeqCst) == 0 {
                    (
                        axum::http::StatusCode::BAD_GATEWAY,
                        "temporary fixture failure",
                    )
                } else {
                    (axum::http::StatusCode::OK, PAPER)
                }
            }
        }),
    );
    let directory =
        std::env::temp_dir().join(format!("research-source-test-{}", uuid::Uuid::new_v4()));
    let server = tokio::spawn(axum::serve(listener, router).into_future());
    let source = AssignmentSources::start(
        SourceConfig {
            endpoint: format!("{origin}/mcp"),
            token: None,
            tools: vec!["search".into(), "read".into()],
            file_origins: vec![],
        },
        5,
        stop.clone(),
        directory.clone(),
    )
    .await
    .unwrap();
    let transport = StreamableHttpClientTransport::with_client(
        reqwest::Client::new(),
        StreamableHttpClientTransportConfig::with_uri(source.endpoint.clone())
            .auth_header(source.token.clone()),
    );
    let client = ()
        .serve_with_lifecycle(
            transport,
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            },
        )
        .await
        .unwrap();
    let names: Vec<_> = client
        .list_all_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();
    assert_eq!(names, vec!["search", "read", "read_source_material"]);
    assert!(
        client
            .call_tool(CallToolRequestParams::new("admin"))
            .await
            .is_err()
    );
    let search = client
        .call_tool(CallToolRequestParams::new("search"))
        .await
        .unwrap();
    assert!(
        search.structured_content.as_ref().unwrap()["text"]
            .as_str()
            .unwrap()
            .contains("Search lead only")
    );
    let mixed = search.structured_content.as_ref().unwrap()["text"]
        .as_str()
        .unwrap();
    assert!(mixed.contains("Distinct text supplied alongside structured metadata."));
    let read = client
        .call_tool(CallToolRequestParams::new("read"))
        .await
        .unwrap();
    let pending = read.structured_content.unwrap();
    assert_eq!(pending["operation_status"], "succeeded");
    assert_eq!(pending["delivery_status"], "failed");
    let recovered = client
        .call_tool(
            CallToolRequestParams::new("read_source_material").with_arguments(
                json!({"material_id":pending["material_id"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_ne!(recovered.is_error, Some(true), "{recovered:?}");
    let recovered = recovered.structured_content.unwrap();
    assert!(
        recovered["delivery_limitations"][0]
            .as_str()
            .unwrap()
            .contains("not downloaded")
    );
    let text = recovered.to_string();
    assert!(text.contains("Actual retrieved text"));
    assert!(text.contains("Fixture document attribution."));
    assert!(!text.contains("host-only"));
    assert_eq!(downloads.load(Ordering::SeqCst), 2);
    let oversized = client
        .call_tool(
            CallToolRequestParams::new("search")
                .with_arguments(json!({"oversized":true}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    assert_eq!(oversized.is_error, Some(true));
    assert!(
        oversized.structured_content.unwrap()["error"]
            .as_str()
            .unwrap()
            .contains("ingestion limit")
    );
    let nested = client
        .call_tool(
            CallToolRequestParams::new("read")
                .with_arguments(json!({"nested":true}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    assert!(
        nested.structured_content.unwrap()["delivery_limitations"][0]
            .as_str()
            .unwrap()
            .contains("not downloaded")
    );
    let exhausted = client
        .call_tool(CallToolRequestParams::new("search"))
        .await
        .unwrap();
    assert_eq!(exhausted.is_error, Some(true));
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    client.cancel().await.unwrap();
    drop(source);
    stop.cancel();
    server.abort();
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn cancelling_stalled_tool_discovery_returns_without_waiting_for_timeout() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let discovery = Arc::new(tokio::sync::Notify::new());
    let gateway = Gateway {
        origin: origin.clone(),
        calls: Arc::new(AtomicU32::new(0)),
        discovery: Some(discovery.clone()),
    };
    let stop = CancellationToken::new();
    let service = StreamableHttpService::new(
        move || Ok(gateway.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true)
            .with_cancellation_token(stop.clone()),
    );
    let server = tokio::spawn(
        axum::serve(listener, Router::new().nest_service("/mcp", service)).into_future(),
    );
    let directory =
        std::env::temp_dir().join(format!("research-discovery-test-{}", uuid::Uuid::new_v4()));
    let cancel = stop.child_token();
    let start_cancel = cancel.clone();
    let root = directory.clone();
    let starting = tokio::spawn(async move {
        AssignmentSources::start(
            SourceConfig {
                endpoint: format!("{origin}/mcp"),
                token: None,
                tools: vec!["search".into()],
                file_origins: vec![],
            },
            1,
            start_cancel,
            root,
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(3), discovery.notified())
        .await
        .unwrap();
    cancel.cancel();
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(1), starting)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        outcome,
        Err(mcp_deepresearch::runtime::RuntimeError::Cancelled)
    ));
    stop.cancel();
    server.abort();
    std::fs::remove_dir_all(directory).unwrap();
}
