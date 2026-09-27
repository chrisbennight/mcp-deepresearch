use mcp_deepresearch::research::TraceContext;
use mcp_deepresearch::sources::{AssignmentSources, SourceConfig};
use rmcp::{
    ClientServiceExt,
    model::*,
    service::ClientLifecycleMode,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn corpus_boundary_authenticates_and_exposes_only_selected_documents() {
    let root = std::env::temp_dir().join(format!("corpus-contract-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let docs = root.join("docs.jsonl");
    std::fs::write(
        &docs,
        "{\"id\":\"included\",\"text\":\"Salt water can accelerate galvanic corrosion.\"}\n",
    )
    .unwrap();
    let token = uuid::Uuid::new_v4().to_string();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_mcp-deepresearch"))
        .args(["corpus", docs.to_str().unwrap(), "0"])
        .env("DEEPRESEARCH_SOURCE_TOKEN", &token)
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let line = tokio::time::timeout(std::time::Duration::from_secs(10), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let endpoint = line.strip_prefix("Corpus MCP listening on ").unwrap();
    assert_eq!(
        reqwest::Client::new()
            .post(endpoint)
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    let stop = CancellationToken::new();
    let source = AssignmentSources::start(
        SourceConfig {
            endpoint: endpoint.into(),
            token: Some(token),
            tools: vec!["corpus_search".into(), "corpus_read".into()],
            file_origins: vec![],
            local_materials: vec![],
            trace_context: TraceContext::default(),
        },
        10,
        stop.clone(),
        root.join("materials"),
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
    let search = client
        .call_tool(
            CallToolRequestParams::new("corpus_search").with_arguments(
                serde_json::json!({"query":"galvanic"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert!(serde_json::to_string(&search).unwrap().contains("included"));
    let read = client
        .call_tool(
            CallToolRequestParams::new("corpus_read").with_arguments(
                serde_json::json!({"id":"included"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert!(serde_json::to_string(&read).unwrap().contains("Salt water"));
    assert!(
        client
            .call_tool(
                CallToolRequestParams::new("corpus_read").with_arguments(
                    serde_json::json!({"id":"missing"})
                        .as_object()
                        .unwrap()
                        .clone()
                )
            )
            .await
            .unwrap()
            .is_error
            == Some(true)
    );
    assert!(
        client
            .call_tool(CallToolRequestParams::new("kagi_search_fetch"))
            .await
            .is_err()
    );
    client.cancel().await.unwrap();
    stop.cancel();
    child.kill().await.unwrap();
    child.wait().await.unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
