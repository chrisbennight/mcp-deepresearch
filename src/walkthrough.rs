//! Small native MCP client for a reproducible local walkthrough.
use crate::{
    files::{FileDigest, FileValue},
    research::{ResearchId, ResearchRequest},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rmcp::{
    ClientServiceExt, RoleClient,
    model::*,
    service::{ClientLifecycleMode, PeerRequestOptions, RunningService},
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};
use tokio::io::AsyncWriteExt;

struct Client {
    http: reqwest::Client,
    url: String,
    service: RunningService<RoleClient, ()>,
}
impl Client {
    async fn rpc(
        &self,
        method: &str,
        mut params: Value,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        if method == "tools/call" {
            let prefix = std::env::var("DEEPRESEARCH_TOOL_PREFIX").unwrap_or_default();
            if let Some(name) = params["name"].as_str() {
                params["name"] = Value::String(format!("{prefix}{name}"));
            }
        }
        let request = serde_json::from_value(json!({"method":method,"params":params}))?;
        let mut meta = RequestMetaObject::new();
        meta.insert("io.modelcontextprotocol/clientCapabilities".into(), json!({"files":{"upload":true,"download":true,"transports":["https"]},"extensions":{"io.modelcontextprotocol/tasks":{}}}));
        let response = self.service.peer().send_request_with_option(request, PeerRequestOptions::with_timeout(Duration::from_secs(60)).with_meta(meta)).await
            .map_err(|_| "MCP request could not be sent")?
            .await_response().await.map_err(|_| "MCP operation failed; inspect service status without logging transfer credentials")?;
        Ok(serde_json::to_value(response)?)
    }
    fn transfer(
        &self,
        descriptor: &Value,
        method: &str,
    ) -> Result<reqwest::RequestBuilder, Box<dyn std::error::Error>> {
        if descriptor["method"] != method || descriptor["transport"] != "https" {
            return Err("unsupported transfer descriptor".into());
        }
        let url = reqwest::Url::parse(descriptor["url"].as_str().ok_or("missing transfer URL")?)?;
        let endpoint = reqwest::Url::parse(&self.url)?;
        if url.origin() != endpoint.origin()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err("walkthrough accepts same-origin file transfers only; use the gateway helper for other approved origins".into());
        }
        let mut request = self.http.request(method.parse()?, url);
        for (name, value) in descriptor["headers"]
            .as_object()
            .ok_or("missing transfer headers")?
        {
            request = request.header(name, value.as_str().ok_or("invalid transfer header")?);
        }
        Ok(request)
    }
}

pub async fn run(
    request_path: &Path,
    output: &Path,
    attachment: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut request: ResearchRequest =
        serde_json::from_slice(&tokio::fs::read(request_path).await?)?;
    request.validate()?;
    let attachment = if let Some(path) = attachment {
        let size = tokio::fs::metadata(path).await?.len();
        if size > crate::files::MAX_BYTES as u64 {
            return Err("attachment exceeds the supported text limit".into());
        }
        let text = tokio::fs::read_to_string(path).await?;
        Some((path, text))
    } else {
        None
    };
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let url = std::env::var("DEEPRESEARCH_MCP_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8088/mcp".into());
    let transport = StreamableHttpClientTransport::with_client(
        http.clone(),
        StreamableHttpClientTransportConfig::with_uri(url.clone())
            .auth_header(std::env::var("DEEPRESEARCH_INBOUND_TOKEN")?)
            .reinit_on_expired_session(false)
            .max_sse_event_size(1024 * 1024),
    );
    let service = ()
        .serve_with_lifecycle(
            transport,
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            },
        )
        .await
        .map_err(|_| "MCP 2026-07-28 discovery failed")?;
    let client = Client { http, url, service };
    let _ = client.rpc("server/discover", json!({})).await?;
    let _ = client.rpc("tools/list", json!({})).await?;
    if let Some((path, text)) = attachment {
        let digest = FileDigest {
            algorithm: "sha-256".into(),
            value: URL_SAFE_NO_PAD.encode(Sha256::digest(text.as_bytes())),
        };
        let authorization=client.rpc("files/authorizeUpload",json!({"name":path.file_name().and_then(|n|n.to_str()).unwrap_or("attachment.txt"),"mimeType":"text/plain","size":text.len(),"digest":digest})).await?;
        client
            .transfer(&authorization["upload"], "PUT")?
            .body(text)
            .send()
            .await?
            .error_for_status()?;
        request.attachments.push(
            authorization["file"]["uri"]
                .as_str()
                .ok_or("missing upload URI")?
                .into(),
        );
    }
    tokio::fs::create_dir_all(output).await?;
    let id = ResearchId::default();
    let submitted = client
        .rpc(
            "tools/call",
            json!({"name":"research_submit","arguments":{"request_id":id,"request":request}}),
        )
        .await?;
    let task = submitted["taskId"]
        .as_str()
        .ok_or("no task handle returned")?;
    println!("Research {id}; task {task}");
    let deadline =
        tokio::time::Instant::now() + Duration::from_secs(request.limits.wall_seconds + 60);
    let state = loop {
        let state = client.rpc("tasks/get", json!({"taskId":task})).await?;
        match state["status"].as_str() {
            Some("completed" | "failed" | "cancelled") => break state,
            Some("input_required") => {
                let responses = state["inputRequests"]
                    .as_object()
                    .ok_or("missing input requests")?
                    .keys()
                    .map(|key| (key.clone(), json!({"action":"decline"})))
                    .collect::<serde_json::Map<_, _>>();
                let _ = client
                    .rpc(
                        "tasks/update",
                        json!({"taskId":task,"inputResponses":responses}),
                    )
                    .await?;
                println!(
                    "Clarification declined by the unattended walkthrough; assumptions must remain explicit."
                );
            }
            Some("working") => (),
            _ => return Err("unexpected task state".into()),
        }
        if tokio::time::Instant::now() >= deadline {
            let _ = client.rpc("tasks/cancel", json!({"taskId":task})).await?;
            return Err("walkthrough wait expired; cancellation requested; inspect task for observed termination".into());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    };
    let report = client
        .rpc(
            "tools/call",
            json!({"name":"research_report","arguments":{"research_id":id}}),
        )
        .await?;
    let file: FileValue = serde_json::from_value(report["structuredContent"]["file"].clone())?;
    let authorized = client
        .rpc("files/authorizeDownload", json!({"uri":file.uri}))
        .await?;
    let mut response = client
        .transfer(&authorized["download"], "GET")?
        .send()
        .await?
        .error_for_status()?;
    let path = output.join(format!("{id}.md"));
    let temporary = path.with_extension("part");
    let mut destination = tokio::fs::File::create(&temporary).await?;
    let mut digest = Sha256::new();
    let mut size = 0u64;
    while let Some(chunk) = response.chunk().await? {
        size = size
            .checked_add(chunk.len() as u64)
            .ok_or("report size overflow")?;
        if size > file.size {
            return Err("report exceeded advertised size".into());
        }
        digest.update(&chunk);
        destination.write_all(&chunk).await?;
    }
    destination.flush().await?;
    drop(destination);
    if size != file.size
        || file.digest.algorithm != "sha-256"
        || URL_SAFE_NO_PAD.encode(digest.finalize()) != file.digest.value
    {
        return Err("report transfer integrity mismatch".into());
    }
    tokio::fs::rename(&temporary, &path).await?;
    println!(
        "Task {}. Report saved to {}",
        state["status"].as_str().unwrap_or("unknown"),
        path.display()
    );
    if state["status"] != "completed"
        || report["structuredContent"]["status"]["state"] != "completed"
    {
        return Err(
            "research did not complete; the saved report preserves available material".into(),
        );
    }
    Ok(())
}
