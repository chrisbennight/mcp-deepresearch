//! MCP is a projection of Restate workflow state, not a second scheduler.
use crate::{
    lifecycle::{
        Clarification, Owner, ResearchIngressClient, ResearchState, Submission, UserInput,
        workflow_key,
    },
    research::{ResearchId, ResearchRequest, Status},
    runtime,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::Response,
};
use restate_sdk::{ingress::ReqwestClient, serde::Json};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::*,
    service::RequestContext,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct ServiceConfig {
    pub file_root: std::path::PathBuf,
    pub file_origin: String,
    pub restate_ingress: String,
    pub principal: String,
    pub bearer_current: String,
    pub bearer_previous: Option<String>,
    pub allowed_hosts: Vec<String>,
}
#[derive(Clone)]
struct ResearchMcp {
    files: crate::files::FileStore,
    ingress: ReqwestClient,
    principal: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SubmitInput {
    request_id: ResearchId,
    request: ResearchRequest,
    /// An authorized completed research run to revise into a new workspace.
    #[serde(default)]
    previous: Option<ResearchId>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ResearchInput {
    research_id: ResearchId,
}

#[derive(Serialize, JsonSchema)]
struct ReportOutput {
    research_id: ResearchId,
    status: Status,
    report: String,
    report_truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<crate::files::FileValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivery_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivery_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivery_limitations: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_operation: Option<bool>,
}

fn trust(untrusted: bool) -> MetaObject {
    let mut meta = MetaObject::new();
    meta.insert(
        "io.modelcontextprotocol/trust-annotations".into(),
        json!({"sensitive":true,"untrusted":untrusted}),
    );
    meta
}
fn error(message: &str) -> ErrorData {
    ErrorData::invalid_params(message.to_owned(), Some(json!({"_meta":trust(false)})))
}
fn unavailable() -> ErrorData {
    ErrorData::internal_error(
        "research execution service is unavailable; retry with the same request identity",
        Some(json!({"_meta":trust(false)})),
    )
}
fn tool_result(value: Value) -> CallToolResult {
    CallToolResult::structured(value).with_meta(Some(trust(true)))
}
fn task_id(id: ResearchId) -> String {
    format!("research:{id}")
}
fn research_id(task: &str) -> Result<ResearchId, ErrorData> {
    let value = task
        .strip_prefix("research:")
        .ok_or_else(|| error("invalid research task identity"))?;
    Ok(ResearchId(
        uuid::Uuid::parse_str(value).map_err(|_| error("invalid research task identity"))?,
    ))
}
fn timestamp(seconds: u64) -> String {
    jiff::Timestamp::from_second(seconds as i64)
        .expect("server clock is in the supported range")
        .to_string()
}

impl ResearchMcp {
    async fn report(&self, workspace: &crate::workspace::Workspace, files: bool) -> CallToolResult {
        let report = workspace.report();
        let summary: String = report.chars().take(32000).collect();
        let mut value = ReportOutput {
            research_id: workspace.id,
            status: workspace.status.clone(),
            report_truncated: summary.len() < report.len(),
            report: summary,
            file: None,
            delivery_status: None,
            delivery_error: None,
            delivery_limitations: None,
            retry_operation: None,
        };
        if workspace.status.terminal() && files {
            match self.files.publish_report(workspace.id, &report).await {
                Ok(file) => {
                    value.file = Some(file);
                    value.delivery_status = Some("available".into());
                }
                Err(reason) => {
                    value.delivery_status = Some("unavailable".into());
                    value.delivery_error = Some(reason.into());
                    value.retry_operation = Some(false);
                }
            }
        } else if value.report_truncated {
            value.delivery_limitations = Some(
                "Enable native file download capability to retrieve the full terminal report."
                    .into(),
            );
        }
        tool_result(serde_json::to_value(value).expect("report serializes"))
    }

    async fn state(&self, id: ResearchId) -> Result<ResearchState, ErrorData> {
        self.existing(id)
            .await?
            .ok_or_else(|| error("research is not available or has expired"))
    }
    async fn existing(&self, id: ResearchId) -> Result<Option<ResearchState>, ErrorData> {
        Ok(ResearchIngressClient::from_client(
            self.ingress.clone(),
            workflow_key(&self.principal, id),
        )
        .status(Json(Owner {
            principal: self.principal.clone(),
        }))
        .call()
        .await
        .map_err(|_| unavailable())?
        .into_body()
        .map_err(|_| unavailable())?
        .into_inner())
    }
    async fn cancel(&self, id: ResearchId) -> Result<(), ErrorData> {
        ResearchIngressClient::from_client(self.ingress.clone(), workflow_key(&self.principal, id))
            .cancel(Json(Owner {
                principal: self.principal.clone(),
            }))
            .call()
            .await
            .map_err(|_| unavailable())?
            .into_body()
            .map_err(|_| unavailable())?;
        Ok(())
    }
    fn task(state: &ResearchState) -> Task {
        let status = match state.controller.workspace.status {
            Status::Working => TaskStatus::Working,
            Status::InputRequired { .. } => TaskStatus::InputRequired,
            Status::Cancelled => TaskStatus::Cancelled,
            Status::Failed { .. } => TaskStatus::Failed,
            Status::Completed | Status::Exhausted { .. } => TaskStatus::Completed,
        };
        Task::new(
            task_id(state.controller.workspace.id),
            status,
            timestamp(state.controller.started_at),
            timestamp(state.updated_at),
        )
        .with_ttl_ms(7 * 24 * 60 * 60 * 1000)
        .with_poll_interval_ms(1000)
        .with_status_message(state.stage.clone())
    }
    async fn detailed(&self, id: ResearchId, files: bool) -> Result<GetTaskResult, ErrorData> {
        let state = self.state(id).await?;
        let workspace = &state.controller.workspace;
        let payload=match &workspace.status {
            Status::Working=>TaskPayload::Working,
            Status::Cancelled=>TaskPayload::Cancelled,
            Status::InputRequired{question}=>{
                let schema=serde_json::from_value(json!({"type":"object","properties":{"answer":{"type":"string","description":"Clarification for the research question"}},"required":["answer"]})).expect("static elicitation schema");
                let input=ElicitRequest::new(ElicitRequestParams::FormElicitationParams{meta:None,message:question.clone(),requested_schema:schema});
                let mut requests=InputRequests::new();requests.insert(state.controller.workspace.assignments_completed.to_string(),InputRequest::Elicitation(input));
                TaskPayload::InputRequired{input_requests:requests}
            },
            Status::Failed{reason}=>TaskPayload::Failed{error:json!({"code":-32603,"message":reason,"data":{"partial_result":self.report(workspace,files).await,"_meta":trust(true)}}).as_object().unwrap().clone()},
            Status::Completed|Status::Exhausted{..}=>TaskPayload::Completed{result:serde_json::to_value(self.report(workspace,files).await).expect("tool result serializes").as_object().unwrap().clone()},
        };
        let mut result = GetTaskResult::new(DetailedTask::new(Self::task(&state), payload));
        result.meta = Some(trust(true));
        Ok(result)
    }
}

impl ServerHandler for ResearchMcp {
    fn supported_protocol_versions(&self) -> std::borrow::Cow<'static, [ProtocolVersion]> {
        std::borrow::Cow::Owned(vec![ProtocolVersion::V_2026_07_28])
    }
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().enable_tasks().build()).with_protocol_version(ProtocolVersion::V_2026_07_28)
            .with_server_info(Implementation::new("mcp-deepresearch",env!("CARGO_PKG_VERSION")))
            .with_instructions("Submit research with a new UUID request_id. Retry the same request_id to attach to the existing run within retention. Use native Tasks to inspect work, answer clarification requests and cancel. A deliberate revision requires a new request_id.")
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let mut tools: Vec<Tool> = Vec::new();
        for (name, description, read_only, mut schema) in [
            (
                "research_submit",
                "Investigate a question across sources and return a durable task. Optional previous starts a revision of completed work.",
                false,
                serde_json::to_value(schemars::schema_for!(SubmitInput)).unwrap(),
            ),
            (
                "research_status",
                "Read concise progress for an authorized research execution.",
                true,
                serde_json::to_value(schemars::schema_for!(ResearchInput)).unwrap(),
            ),
            (
                "research_report",
                "Read the current or final report, including partial output after cancellation or failure.",
                true,
                serde_json::to_value(schemars::schema_for!(ResearchInput)).unwrap(),
            ),
        ] {
            if name == "research_submit" {
                schema["$defs"]["ResearchRequest"]["properties"]["attachments"]["items"]["x-mcp-file"] =
                    json!({"transferModes":["upload"],"maxSize":crate::files::MAX_BYTES});
            }
            tools.push(serde_json::from_value(json!({"name":name,"description":description,"inputSchema":schema,"annotations":{"readOnlyHint":read_only,"destructiveHint":false,"idempotentHint":true,"openWorldHint":!read_only},"_meta":{"io.modelcontextprotocol/action-metadata":{"inputMetadata":{"destination":if read_only {"internal"} else {"external"},"sensitivity":"sensitive"},"returnMetadata":{"source":"first-party","sensitivity":"sensitive"},"outcome":if read_only {"benign"} else {"research execution"},"requiresReview":false}}})).expect("static tool metadata"));
        }
        for tool in &mut tools {
            if tool.name != "research_status" {
                tool.output_schema = Some(Arc::new(
                    serde_json::to_value(schemars::schema_for!(ReportOutput))
                        .expect("schema")
                        .as_object()
                        .expect("object schema")
                        .clone(),
                ));
            }
        }
        Ok(ListToolsResult::with_all_items(tools)
            .with_ttl_ms(30000)
            .with_cache_scope(CacheScope::Private))
    }
    async fn call_tool(
        &self,
        params: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let arguments = Value::Object(params.arguments.unwrap_or_default());
        match params.name.as_ref() {
            "research_submit" => {
                if !context
                    .client_capabilities()
                    .is_some_and(|c| c.supports_tasks())
                {
                    return Err(error(
                        "research_submit requires the io.modelcontextprotocol/tasks client extension",
                    ));
                }
                let input: SubmitInput = serde_json::from_value(arguments)
                    .map_err(|_| error("invalid research submission"))?;
                input
                    .request
                    .validate()
                    .map_err(|e| error(&e.to_string()))?;
                if let Some(state) = self.existing(input.request_id).await? {
                    return Ok(CreateTaskResult::new(Self::task(&state))
                        .with_meta(trust(false))
                        .into());
                }
                let previous = if let Some(id) = input.previous {
                    let previous = self.state(id).await?.controller.workspace;
                    previous
                        .authorize(&self.principal)
                        .map_err(|_| error("research not found"))?;
                    if !previous.status.terminal() {
                        return Err(error("only completed research can be revised"));
                    }
                    Some(previous)
                } else {
                    None
                };
                let attachments = self
                    .files
                    .admit_attachments(
                        input.request_id,
                        &input.request.attachments,
                        previous.as_ref(),
                    )
                    .await
                    .map_err(error)?;
                let now = runtime::unix_seconds();
                let workflow = ResearchIngressClient::from_client(
                    self.ingress.clone(),
                    workflow_key(&self.principal, input.request_id),
                );
                let mut request = workflow.run(Json(Submission {
                    attachments,
                    trace_context: crate::research::TraceContext {
                        traceparent: context.meta.get_traceparent().map(str::to_owned),
                        tracestate: context.meta.get_tracestate().map(str::to_owned),
                    },
                    id: input.request_id,
                    owner: self.principal.clone(),
                    request: input.request,
                    created_at: now,
                    previous,
                }));
                for (name, value) in [
                    ("traceparent", context.meta.get_traceparent()),
                    ("tracestate", context.meta.get_tracestate()),
                ] {
                    if let Some(value) = value {
                        request = request.header(
                            name.parse().expect("static header"),
                            value.parse().map_err(|_| error("invalid trace metadata"))?,
                        );
                    }
                }
                request.send().await.map_err(|_| unavailable())?;
                let state = tokio::time::timeout(Duration::from_secs(10), async {
                    loop {
                        let state = workflow
                            .status(Json(Owner {
                                principal: self.principal.clone(),
                            }))
                            .call()
                            .await
                            .map_err(|_| unavailable())?
                            .into_body()
                            .map_err(|_| unavailable())?
                            .into_inner();
                        if let Some(state) = state {
                            return Ok::<_, ErrorData>(state);
                        }
                        tokio::time::sleep(Duration::from_millis(25)).await;
                    }
                })
                .await
                .map_err(|_| unavailable())??;
                let task = Self::task(&state);
                Ok(CreateTaskResult::new(task).with_meta(trust(false)).into())
            }
            "research_status" | "research_report" => {
                let input: ResearchInput = serde_json::from_value(arguments)
                    .map_err(|_| error("invalid research identifier"))?;
                let state = self.state(input.research_id).await?;
                let workspace = &state.controller.workspace;
                if params.name == "research_report" {
                    return Ok(self
                        .report(
                            workspace,
                            crate::files::client_supports(
                                &context.meta,
                                "download",
                                self.files.transport(),
                            ),
                        )
                        .await
                        .into());
                }
                let value = json!({"research_id":workspace.id,"status":workspace.status,"stage":state.stage,"assignments_completed":workspace.assignments_completed,"sources":workspace.sources.len(),"uncertainties":workspace.uncertainties});
                Ok(tool_result(value).into())
            }
            _ => Err(error("unknown research tool")),
        }
    }
    async fn get_task(
        &self,
        request: GetTaskParams,
        context: RequestContext<RoleServer>,
    ) -> Result<GetTaskResult, ErrorData> {
        self.detailed(
            research_id(&request.task_id)?,
            crate::files::client_supports(&context.meta, "download", self.files.transport()),
        )
        .await
    }
    async fn on_custom_request(
        &self,
        request: CustomRequest,
        context: RequestContext<RoleServer>,
    ) -> Result<CustomResult, ErrorData> {
        let params = request.params.unwrap_or_else(|| json!({}));
        let value = match request.method.as_ref() {
            "files/authorizeUpload" => {
                if !crate::files::client_supports(&context.meta, "upload", self.files.transport()) {
                    return Err(error(
                        "declare request-local files.upload and the advertised file transport capability",
                    ));
                }
                self.files.authorize_upload(params).await.map_err(error)?
            }
            "files/authorizeDownload" => {
                if !crate::files::client_supports(&context.meta, "download", self.files.transport())
                {
                    return Err(error(
                        "declare request-local files.download and the advertised file transport capability",
                    ));
                }
                self.files
                    .authorize_download(
                        params
                            .get("uri")
                            .and_then(Value::as_str)
                            .ok_or_else(|| error("file uri is required"))?,
                    )
                    .await
                    .map_err(error)?
            }
            _ => {
                return Err(ErrorData::new(
                    ErrorCode::METHOD_NOT_FOUND,
                    "unknown method",
                    Some(json!({"_meta":trust(false)})),
                ));
            }
        };
        // Native transfer authorization is a host control response, not a tool result.
        // A top-level tool-result _meta makes current SDKs misclassify this custom result.
        Ok(CustomResult::new(value))
    }
    async fn cancel_task(
        &self,
        request: CancelTaskParams,
        _: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        self.cancel(research_id(&request.task_id)?).await
    }
    async fn update_task(
        &self,
        request: UpdateTaskParams,
        _: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        let id = research_id(&request.task_id)?;
        let state = self.state(id).await?;
        if !matches!(
            state.controller.workspace.status,
            Status::InputRequired { .. }
        ) {
            return Ok(());
        }
        let question_id = state.controller.workspace.assignments_completed;
        let Some(response) = request.input_responses.get(&question_id.to_string()) else {
            return Ok(());
        };
        let response=match response.get("action").and_then(Value::as_str) {
            Some("cancel")=>Clarification::Cancel,
            Some("decline")=>Clarification::Answer("The user declined clarification. Proceed with available evidence and disclose assumptions.".to_owned()),
            Some("accept")=>Clarification::Answer(response.pointer("/content/answer").and_then(Value::as_str).ok_or_else(||error("clarification answer is required"))?.to_owned()),
            _=>return Err(error("invalid elicitation response")),
        };
        ResearchIngressClient::from_client(self.ingress.clone(), workflow_key(&self.principal, id))
            .provide_input(Json(UserInput {
                principal: self.principal.clone(),
                question_id,
                response,
            }))
            .call()
            .await
            .map_err(|_| error("clarification is unavailable, stale, or already answered"))?
            .into_body()
            .map_err(|_| unavailable())?;
        Ok(())
    }
}

pub fn router(
    config: ServiceConfig,
    cancel: CancellationToken,
) -> Result<Router, Box<dyn std::error::Error>> {
    if config.principal.is_empty() || config.bearer_current.is_empty() {
        return Err("configure a trusted principal and inbound bearer credential".into());
    }
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let files = crate::files::FileStore::new(
        config.file_root.clone(),
        config.file_origin.clone(),
        config.principal.clone(),
    )?;
    let service = ResearchMcp {
        files: files.clone(),
        ingress: ReqwestClient::new(config.restate_ingress.parse()?, http)?,
        principal: config.principal.clone(),
    };
    let transport = StreamableHttpService::new(
        move || Ok(service.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true)
            .with_cancellation_token(cancel)
            .with_allowed_hosts(config.allowed_hosts.clone()),
    );
    Ok(Router::new()
        .nest_service("/mcp", transport)
        .layer(middleware::from_fn_with_state(
            (config, files.transport().to_owned()),
            authenticate,
        ))
        .merge(files.router()))
}
async fn authenticate(
    State((config, file_transport)): State<(ServiceConfig, String)>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let bearer = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    if bearer != Some(config.bearer_current.as_str())
        && !(config.bearer_previous.as_deref().is_some()
            && bearer == config.bearer_previous.as_deref())
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
        || request.headers().contains_key("MCP-Session-Id")
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let discovery = request
        .headers()
        .get("Mcp-Method")
        .is_some_and(|v| v == "server/discover");
    let (parts, body) = request.into_parts();
    let bytes = tokio::time::timeout(Duration::from_secs(10), to_bytes(body, 512 * 1024))
        .await
        .map_err(|_| StatusCode::REQUEST_TIMEOUT)?
        .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)?;
    let response = next
        .run(Request::from_parts(parts, Body::from(bytes)))
        .await;
    if !discovery || !response.status().is_success() {
        return Ok(response);
    }
    // The SDK does not yet model draft file capabilities. Extend only its discovery result.
    let (mut parts, body) = response.into_parts();
    let bytes = to_bytes(body, 64 * 1024)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut value: Value =
        serde_json::from_slice(&bytes).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(capabilities) = value.pointer_mut("/result/capabilities") {
        capabilities["files"] =
            json!({"upload":true,"download":true,"transports":[file_transport]});
    }
    parts.headers.remove(axum::http::header::CONTENT_LENGTH);
    Ok(Response::from_parts(
        parts,
        Body::from(serde_json::to_vec(&value).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?),
    ))
}
