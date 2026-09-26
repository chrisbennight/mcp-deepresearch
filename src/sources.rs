//! Assignment-scoped source access. The host owns upstream credentials and files.
use crate::runtime::RuntimeError;
use axum::{
    Router,
    extract::{Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::Response,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::Url;
use rmcp::{
    ClientServiceExt, ErrorData, RoleClient, RoleServer, ServerHandler,
    model::*,
    service::{ClientLifecycleMode, PeerRequestOptions, RequestContext, RunningService},
    transport::{
        StreamableHttpClientTransport,
        streamable_http_client::StreamableHttpClientTransportConfig,
        streamable_http_server::{
            StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
        },
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::os::unix::fs::DirBuilderExt;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};
use tokio::{io::AsyncWriteExt, sync::Mutex};
use tokio_util::sync::CancellationToken;

const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
const MAX_EXCERPT_CHARS: usize = 32_000;
const CALL_TIMEOUT: Duration = Duration::from_secs(90);

fn failure(message: &str) -> RuntimeError {
    RuntimeError::Failed(message.into())
}

#[derive(Clone)]
pub struct SourceConfig {
    pub endpoint: String,
    pub token: Option<String>,
    pub tools: Vec<String>,
    /// Additional operator-approved file origins; redirects are never followed.
    pub file_origins: Vec<String>,
}

#[derive(Clone)]
struct SourceAccess {
    upstream: Arc<RunningService<RoleClient, ()>>,
    http: reqwest::Client,
    endpoint: Url,
    file_origins: HashSet<String>,
    tools: Vec<Tool>,
    remaining: Arc<AtomicU32>,
    cancel: CancellationToken,
    directory: PathBuf,
    materials: Arc<Mutex<HashMap<String, Material>>>,
}

#[derive(Clone)]
struct Material {
    path: PathBuf,
    remote: Option<FileValue>,
    nested_files: bool,
}

/// A private, temporary MCP endpoint for one assignment. Dropping it revokes access.
pub struct AssignmentSources {
    pub endpoint: String,
    pub token: String,
    cancel: CancellationToken,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for AssignmentSources {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.server.abort();
    }
}

impl AssignmentSources {
    pub async fn start(
        config: SourceConfig,
        allowance: u32,
        cancel: CancellationToken,
        directory: PathBuf,
    ) -> Result<Self, RuntimeError> {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&directory)
            .map_err(|_| failure("source material storage unavailable"))?;
        let endpoint =
            Url::parse(&config.endpoint).map_err(|_| failure("invalid source gateway URL"))?;
        if !matches!(endpoint.scheme(), "https" | "http")
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
        {
            return Err(failure(
                "source gateway must be HTTP(S) without embedded credentials",
            ));
        }
        if config.tools.is_empty() {
            return Err(failure("configure explicit source tools"));
        }
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(CALL_TIMEOUT)
            .build()
            .map_err(|_| failure("source HTTP client could not start"))?;
        let mut transport_config =
            StreamableHttpClientTransportConfig::with_uri(config.endpoint.clone())
                .reinit_on_expired_session(false)
                .max_sse_event_size(MAX_FILE_BYTES);
        if let Some(token) = config.token {
            transport_config = transport_config.auth_header(token);
        }
        let transport = StreamableHttpClientTransport::with_client(http.clone(), transport_config);
        let connect = ().serve_with_lifecycle(
            transport,
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            },
        );
        let upstream = tokio::select! {
            _ = cancel.cancelled() => return Err(RuntimeError::Cancelled),
            result = tokio::time::timeout(CALL_TIMEOUT, connect) => result.map_err(|_| failure("source discovery timed out"))?.map_err(|_| failure("source discovery failed: MCP 2026-07-28 is required"))?,
        };
        let available = tokio::select! {
            _ = cancel.cancelled() => return Err(RuntimeError::Cancelled),
            result = tokio::time::timeout(CALL_TIMEOUT, upstream.list_all_tools()) => result
                .map_err(|_| failure("source tool discovery timed out"))?
                .map_err(|_| failure("source tool discovery failed"))?,
        };
        let mut tools = Vec::new();
        for name in &config.tools {
            let mut tool = available
                .iter()
                .find(|t| t.name.as_ref() == name)
                .cloned()
                .ok_or_else(|| failure("a configured source tool was not discovered"))?;
            if tool.annotations.as_ref().and_then(|a| a.read_only_hint) != Some(true) {
                return Err(failure(
                    "configured source tools must declare read-only behavior",
                ));
            }
            // The adapter returns excerpts rather than the upstream's original result shape.
            tool.output_schema = None;
            tools.push(tool);
        }
        if config.tools.iter().any(|n| n == "read_source_material") {
            return Err(failure(
                "read_source_material is reserved by the research host",
            ));
        }
        tools.push(serde_json::from_value(json!({"name":"read_source_material","description":"Read another section of source material already returned by a source tool, or retry its file delivery without repeating the research operation. Offsets count Unicode characters.","inputSchema":{"type":"object","properties":{"material_id":{"type":"string"},"offset":{"type":"integer","minimum":0}},"required":["material_id"],"additionalProperties":false},"annotations":{"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}})).expect("static tool schema"));
        let token = uuid::Uuid::new_v4().to_string();
        let stop = cancel.child_token();
        let access = SourceAccess {
            upstream: Arc::new(upstream),
            http,
            endpoint,
            file_origins: config.file_origins.into_iter().collect(),
            tools,
            remaining: Arc::new(AtomicU32::new(allowance)),
            cancel: stop.clone(),
            directory,
            materials: Arc::new(Mutex::new(HashMap::new())),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| failure("could not bind assignment source endpoint"))?;
        let address = listener
            .local_addr()
            .map_err(|_| failure("source endpoint address unavailable"))?;
        let service = StreamableHttpService::new(
            move || Ok(access.clone()),
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default()
                .with_legacy_session_mode(false)
                .with_json_response(true)
                .with_cancellation_token(stop.clone())
                .with_allowed_hosts(vec![address.to_string()]),
        );
        let router = Router::new()
            .nest_service("/mcp", service)
            .layer(middleware::from_fn_with_state(token.clone(), authenticate));
        let shutdown = stop.clone();
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await;
        });
        Ok(Self {
            endpoint: format!("http://{address}/mcp"),
            token,
            cancel: stop,
            server,
        })
    }
}

async fn authenticate(
    State(token): State<String>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        != Some(format!("Bearer {token}").as_str())
    {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if request.method() != axum::http::Method::POST {
        return Err(StatusCode::METHOD_NOT_ALLOWED);
    }
    if request
        .headers()
        .get("MCP-Protocol-Version")
        .and_then(|v| v.to_str().ok())
        != Some("2026-07-28")
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(next.run(request).await)
}

impl ServerHandler for SourceAccess {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::V_2026_07_28)
            .with_server_info(Implementation::new(
                "research-sources",
                env!("CARGO_PKG_VERSION"),
            ))
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(self.tools.clone()))
    }
    async fn call_tool(
        &self,
        params: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if !self.tools.iter().any(|tool| tool.name == params.name) {
            return Err(ErrorData::invalid_params(
                "source tool is not allowed",
                None,
            ));
        }
        if self
            .remaining
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_err()
        {
            return Ok(
                tool_error("Source call allowance exhausted; use collected material.").into(),
            );
        }
        let result = tokio::select! {
            _ = self.cancel.cancelled() => Err(failure("source assignment cancelled")),
            _ = context.ct.cancelled() => Err(failure("source call cancelled")),
            result = self.call(params) => result,
        };
        Ok(match result {
            Ok(value) => tool_content(value),
            Err(error) => tool_error(&error.to_string()),
        }
        .into())
    }
}

fn tool_content(value: Value) -> CallToolResult {
    let mut meta = MetaObject::new();
    meta.insert(
        "io.modelcontextprotocol/trust-annotations".into(),
        json!({"untrusted":true,"sensitive":true}),
    );
    CallToolResult::structured(value).with_meta(Some(meta))
}
fn tool_error(message: &str) -> CallToolResult {
    let mut result = tool_content(
        json!({"error":message,"guidance":"Preserve this limitation. Do not claim the unavailable source was read."}),
    );
    result.is_error = Some(true);
    result
}

impl SourceAccess {
    async fn request(&self, request: ClientRequest) -> Result<Value, RuntimeError> {
        let mut meta = RequestMetaObject::new();
        meta.insert(
            "io.modelcontextprotocol/clientCapabilities".into(),
            json!({"files":{"download":true,"transports":["https"]}}),
        );
        let response = self
            .upstream
            .peer()
            .send_request_with_option(
                request,
                PeerRequestOptions::with_timeout(CALL_TIMEOUT).with_meta(meta),
            )
            .await
            .map_err(|_| failure("source gateway request unavailable"))?
            .await_response()
            .await
            .map_err(|_| failure("source gateway request failed or timed out"))?;
        serde_json::to_value(response).map_err(|_| failure("source response encoding failed"))
    }
    async fn call(&self, mut params: CallToolRequestParams) -> Result<Value, RuntimeError> {
        if params.name == "read_source_material" {
            let arguments = params.arguments.unwrap_or_default();
            let id = arguments
                .get("material_id")
                .and_then(Value::as_str)
                .ok_or_else(|| failure("material_id is required"))?;
            let offset = arguments
                .get("offset")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .min(MAX_FILE_BYTES as u64) as usize;
            return self.read_material(id, offset).await;
        }
        // Source requests carry host capabilities, not an agent's claimed identity or policy.
        params.meta = None;
        let result = self
            .request(ClientRequest::CallToolRequest(CallToolRequest::new(params)))
            .await?;
        if result.get("isError").and_then(Value::as_bool) == Some(true) {
            return Err(failure("source tool reported failure"));
        }
        if result
            .get("resultType")
            .and_then(Value::as_str)
            .is_some_and(|s| s != "complete")
        {
            return Err(failure(
                "source tool requested an unsupported interactive continuation",
            ));
        }
        if let Some(file) = retained_file(&result) {
            let file: FileValue = serde_json::from_value(file.clone())
                .map_err(|_| failure("invalid retained source reference"))?;
            let id = uuid::Uuid::new_v4().to_string();
            let path = self.directory.join(format!("{id}.txt"));
            self.materials.lock().await.insert(
                id.clone(),
                Material {
                    path,
                    remote: Some(file),
                    nested_files: false,
                },
            );
            return match self.read_material(&id, 0).await {
                Ok(value) => Ok(value),
                Err(error) => Ok(
                    json!({"operation_status":"succeeded","delivery_status":"failed","material_id":id,"error":error.to_string(),"guidance":"Retry read_source_material with this material_id. Do not repeat the successful source operation."}),
                ),
            };
        }
        let mut parts = Vec::new();
        if let Some(value) = result.get("structuredContent") {
            parts.push(serde_json::to_string(value).expect("JSON serializes"));
        }
        for text in result
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|content| content.get("text").and_then(Value::as_str))
        {
            if !parts.iter().any(|part| part == text) {
                parts.push(text.to_owned());
            }
        }
        let text = parts.join("\n\n");
        if text.len() > MAX_FILE_BYTES {
            return Err(failure("source material exceeds the text ingestion limit"));
        }
        if text.is_empty() {
            return Err(failure("source tool returned no readable text"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let path = self.directory.join(format!("{id}.txt"));
        tokio::fs::write(&path, &text)
            .await
            .map_err(|_| failure("source material storage unavailable"))?;
        self.materials.lock().await.insert(
            id.clone(),
            Material {
                path,
                remote: None,
                nested_files: contains_file_reference(&result),
            },
        );
        let mut selected = excerpt(&text, &id, 0);
        if contains_file_reference(&result) {
            selected["delivery_limitations"] = json!([
                "Nested file references were not downloaded. Only inline text and metadata were read; use a configured extraction tool for the referenced documents."
            ]);
        }
        Ok(selected)
    }
    async fn read_material(&self, id: &str, offset: usize) -> Result<Value, RuntimeError> {
        let material = self
            .materials
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| failure("source material is not available to this assignment"))?;
        if !material.path.exists() {
            let file = material
                .remote
                .as_ref()
                .ok_or_else(|| failure("source material is unavailable"))?;
            self.download(file, &material.path).await?;
        }
        let text = tokio::fs::read_to_string(&material.path)
            .await
            .map_err(|_| {
                failure(
                    "source material is not readable UTF-8 text; use a configured extraction tool",
                )
            })?;
        let mut selected = excerpt(&text, id, offset);
        if material.nested_files {
            selected["delivery_limitations"] = json!([
                "Nested file references were not downloaded. Only inline text and metadata were read."
            ]);
        }
        Ok(selected)
    }
    async fn download(&self, file: &FileValue, path: &Path) -> Result<(), RuntimeError> {
        let authorized = self
            .request(ClientRequest::CustomRequest(CustomRequest::new(
                "files/authorizeDownload",
                Some(json!({"uri":file.uri})),
            )))
            .await?;
        let authorization: DownloadAuthorization = serde_json::from_value(authorized)
            .map_err(|_| failure("source file authorization has an unsupported shape"))?;
        let url = Url::parse(&authorization.download.url)
            .map_err(|_| failure("invalid source file transfer URL"))?;
        let same_origin = url.origin() == self.endpoint.origin();
        if (!same_origin
            && !self
                .file_origins
                .contains(&url.origin().ascii_serialization()))
            || !url.username().is_empty()
            || url.password().is_some()
            || authorization.download.method != "GET"
            || authorization.download.transport != url.scheme()
            || !(url.scheme() == "https" || (same_origin && self.endpoint.scheme() == "http"))
        {
            return Err(failure("source file transfer destination is not approved"));
        }
        let mut request = self.http.get(url);
        for (name, value) in authorization.download.headers {
            if matches!(
                name.to_ascii_lowercase().as_str(),
                "host" | "connection" | "content-length" | "transfer-encoding"
            ) {
                return Err(failure("unsupported source transfer header"));
            }
            request = request.header(name, value);
        }
        let mut response = request.send().await.map_err(|_| {
            failure("source file download unavailable; do not rerun the source operation")
        })?;
        if !response.status().is_success() {
            return Err(failure(
                "source file download failed; do not rerun the source operation",
            ));
        }
        let temporary =
            TemporaryDownload(path.with_extension(format!("{}.part", uuid::Uuid::new_v4())));
        let mut destination = tokio::fs::File::create(&temporary.0)
            .await
            .map_err(|_| failure("source material storage unavailable"))?;
        let mut size = 0usize;
        let mut digest = Sha256::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| failure("source file transfer interrupted"))?
        {
            size += chunk.len();
            if size > MAX_FILE_BYTES {
                return Err(failure("source file exceeds the text ingestion limit"));
            }
            digest.update(&chunk);
            destination
                .write_all(&chunk)
                .await
                .map_err(|_| failure("source material storage unavailable"))?;
        }
        destination
            .flush()
            .await
            .map_err(|_| failure("source material storage unavailable"))?;
        drop(destination);
        let actual_digest = URL_SAFE_NO_PAD.encode(digest.finalize());
        for descriptor in [file, &authorization.file] {
            if descriptor
                .size
                .is_some_and(|expected| expected != size as u64)
            {
                return Err(failure("source file size mismatch"));
            }
            if let Some(digest) = &descriptor.digest
                && (digest.algorithm != "sha-256" || digest.value != actual_digest)
            {
                return Err(failure("source file integrity check failed"));
            }
        }
        tokio::fs::rename(&temporary.0, path)
            .await
            .map_err(|_| failure("source material publication failed"))?;
        Ok(())
    }
}

struct TemporaryDownload(PathBuf);
impl Drop for TemporaryDownload {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn excerpt(text: &str, id: &str, offset: usize) -> Value {
    let selected: String = text.chars().skip(offset).take(MAX_EXCERPT_CHARS).collect();
    let next = offset + selected.chars().count();
    json!({"material_id":id,"text":selected,"offset":offset,"next_offset":if text.chars().count()>next {Some(next)} else {None},"evidence_note":"Retrieved tool material. Search snippets are leads, not proof of full-document reading. Cite original URLs and distinguish extracted source text from inference."})
}
fn contains_file_reference(value: &Value) -> bool {
    match value {
        Value::String(text) => text.starts_with("mcp-file:"),
        Value::Array(items) => items.iter().any(contains_file_reference),
        Value::Object(object) => object.values().any(contains_file_reference),
        _ => false,
    }
}
fn retained_file(result: &Value) -> Option<&Value> {
    result
        .pointer("/_meta/io.cacahuate.mcp-gateway~1retained-delivery/file")
        .or_else(|| result.pointer("/structuredContent/_gateway_delivery/file"))
        .or_else(|| result.pointer("/structuredContent/result/file"))
}
#[derive(Clone, Serialize, Deserialize)]
struct FileValue {
    uri: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    digest: Option<FileDigest>,
}
#[derive(Clone, Serialize, Deserialize)]
struct FileDigest {
    algorithm: String,
    value: String,
}
#[derive(Deserialize)]
struct DownloadAuthorization {
    file: FileValue,
    download: DownloadDescriptor,
}
#[derive(Deserialize)]
struct DownloadDescriptor {
    transport: String,
    method: String,
    url: String,
    #[serde(default)]
    headers: std::collections::BTreeMap<String, String>,
}
