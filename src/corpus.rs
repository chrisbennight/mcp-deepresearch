//! Evaluation-only search/read over an operator-supplied, bounded document pool.
//! This is a lexical baseline, not a reproduction of a track's official retriever.
use axum::{
    Router,
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::*,
    service::RequestContext,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde::Deserialize;
use serde_json::json;
use std::{collections::HashSet, path::Path, sync::Arc};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Deserialize)]
pub struct Document {
    pub id: String,
    pub text: String,
}
#[derive(Clone)]
pub struct Corpus {
    documents: Arc<Vec<Document>>,
}
impl Corpus {
    pub fn new(documents: Vec<Document>) -> Result<Self, Box<dyn std::error::Error>> {
        let ids: HashSet<_> = documents.iter().map(|d| &d.id).collect();
        if documents.is_empty() || ids.len() != documents.len() {
            return Err("corpus must have distinct document IDs".into());
        }
        Ok(Self {
            documents: Arc::new(documents),
        })
    }
    pub fn search(&self, query: &str) -> Vec<serde_json::Value> {
        let words: HashSet<_> = query.split_whitespace().map(str::to_lowercase).collect();
        let mut matches: Vec<_> = self
            .documents
            .iter()
            .map(|d| {
                let lower = d.text.to_lowercase();
                let score = words
                    .iter()
                    .filter(|word| lower.contains(word.as_str()))
                    .count();
                (score, d)
            })
            .filter(|(score, _)| *score > 0)
            .collect();
        matches.sort_by(|(a, da), (b, db)| b.cmp(a).then_with(|| da.id.cmp(&db.id)));
        matches.into_iter().take(10).map(|(_, d)| json!({"id":d.id,"url":format!("corpus:{}",d.id),"snippet":d.text.chars().take(500).collect::<String>()})).collect()
    }
}
impl ServerHandler for Corpus {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::V_2026_07_28)
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = [("corpus_search", "query", "Search only this benchmark's configured document pool."), ("corpus_read", "id", "Read a document by its returned corpus ID; offset is a character offset, default 0.")].map(|(name, field, description)| {
            serde_json::from_value(json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":{field:{"type":"string"},"offset":{"type":"integer","minimum":0}},"required":[field]},"annotations":{"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}})).expect("static tool schema")
        });
        Ok(ListToolsResult::with_all_items(tools.into()))
    }
    async fn call_tool(
        &self,
        params: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let args = params.arguments.unwrap_or_default();
        let value = match params.name.as_ref() {
            "corpus_search" => {
                json!({"results":self.search(args.get("query").and_then(|v|v.as_str()).ok_or_else(||ErrorData::invalid_params("query required",None))?)})
            }
            "corpus_read" => {
                let id = args
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ErrorData::invalid_params("id required", None))?;
                let doc = self.documents.iter().find(|d| d.id == id).ok_or_else(|| {
                    ErrorData::invalid_params("document absent from permitted corpus", None)
                })?;
                let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let text: String = doc.text.chars().skip(offset).take(16000).collect();
                let next = offset.saturating_add(text.chars().count());
                json!({"id":doc.id,"url":format!("corpus:{}",doc.id),"text":text,"next_offset":(next < doc.text.chars().count()).then_some(next)})
            }
            _ => return Err(ErrorData::invalid_params("unknown corpus tool", None)),
        };
        Ok(CallToolResult::structured(value).into())
    }
}
async fn auth(
    State(expected): State<Arc<String>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        != Some(expected.as_str())
    {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(request).await)
}
pub async fn serve(path: &Path, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let docs = std::fs::read_to_string(path)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<Document>, _>>()?;
    let corpus = Corpus::new(docs)?;
    let token = std::env::var("DEEPRESEARCH_SOURCE_TOKEN")?;
    if token.is_empty() {
        return Err("corpus source token must not be empty".into());
    }
    let stop = CancellationToken::new();
    let service = StreamableHttpService::new(
        move || Ok(corpus.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true)
            .with_cancellation_token(stop.clone()),
    );
    let router = Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(
            Arc::new(format!("Bearer {token}")),
            auth,
        ));
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    println!(
        "Corpus MCP listening on http://{}/mcp",
        listener.local_addr()?
    );
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            stop.cancel();
        })
        .await?;
    Ok(())
}
