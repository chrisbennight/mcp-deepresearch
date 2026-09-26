//! Opt-in integration against an isolated Restate server, never a model provider.
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};
use uuid::Uuid;

struct Application {
    child: Child,
    directory: PathBuf,
    mcp_port: u16,
    workflow_port: u16,
    ingress: String,
}
impl Application {
    fn start(ingress: String) -> Self {
        let port = || {
            let listener = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
            listener.local_addr().unwrap().port()
        };
        let directory =
            std::env::temp_dir().join(format!("research-integration-{}", Uuid::new_v4()));
        let mcp_port = port();
        let workflow_port = port();
        let child = Self::spawn(&directory, mcp_port, workflow_port, &ingress);
        Self {
            child,
            directory,
            mcp_port,
            workflow_port,
            ingress,
        }
    }
    fn spawn(directory: &std::path::Path, mcp: u16, workflow: u16, ingress: &str) -> Child {
        Command::new(env!("CARGO_BIN_EXE_mcp-deepresearch"))
            .args(["serve", "fixture"])
            .arg(directory)
            .env_clear()
            .env("DEEPRESEARCH_INBOUND_TOKEN", "local-fixture-token")
            .env("DEEPRESEARCH_PRINCIPAL", "fixture-owner")
            .env(
                "DEEPRESEARCH_FILE_ORIGIN",
                format!("http://127.0.0.1:{mcp}"),
            )
            .env("DEEPRESEARCH_RESTATE_INGRESS", ingress)
            .env("DEEPRESEARCH_MCP_LISTEN", format!("127.0.0.1:{mcp}"))
            .env(
                "DEEPRESEARCH_WORKFLOW_LISTEN",
                format!("0.0.0.0:{workflow}"),
            )
            .env("DEEPRESEARCH_ALLOWED_HOSTS", format!("127.0.0.1:{mcp}"))
            .env("DEEPRESEARCH_FIXTURE_DELAY_MS", "250")
            .env("DEEPRESEARCH_FIXTURE_CLARIFICATION", "true")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap()
    }
    fn restart(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
        self.child = Self::spawn(
            &self.directory,
            self.mcp_port,
            self.workflow_port,
            &self.ingress,
        );
    }
    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.mcp_port)
    }
    async fn ready(&mut self) {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                assert!(
                    self.child.try_wait().unwrap().is_none(),
                    "application exited during startup"
                );
                if tokio::net::TcpStream::connect(("127.0.0.1", self.mcp_port))
                    .await
                    .is_ok()
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .unwrap();
    }
}
impl Drop for Application {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

async fn rpc(client: &reqwest::Client, url: &str, method: &str, mut params: Value) -> Value {
    params["_meta"] = json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{"files":{"upload":true,"download":true,"transports":["https"]},"extensions":{"io.modelcontextprotocol/tasks":{}}}});
    let mut request = client
        .post(url)
        .bearer_auth("local-fixture-token")
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("Mcp-Method", method)
        .header("Accept", "application/json, text/event-stream");
    if let Some(name) = params
        .get("name")
        .or_else(|| params.get("taskId"))
        .and_then(Value::as_str)
    {
        request = request.header("Mcp-Name", name);
    }
    let response = request
        .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.text().await.unwrap();
    assert!(
        status.is_success(),
        "MCP {method} HTTP failure: {status}: {body}"
    );
    serde_json::from_str(&body).unwrap()
}
async fn wait_status(client: &reqwest::Client, url: &str, task: &str, wanted: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(35), async {
        loop {
            let response = rpc(client, url, "tasks/get", json!({"taskId":task})).await;
            if response["result"]["status"] == wanted {
                return response["result"].clone();
            }
            assert!(
                response.get("error").is_none(),
                "task lookup failed: {response}"
            );
            assert_ne!(
                response["result"]["status"], "failed",
                "task failed: {response}"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap()
}
fn submit(id: Uuid, previous: Option<Uuid>, wall: u64) -> Value {
    json!({"name":"research_submit","arguments":{"request_id":id,"previous":previous,"request":{"objective":"Compare the fixture approaches for recovery.","strategy":"comparison","format":"report","limits":{"wall_seconds":wall}}}})
}

#[tokio::test]
#[ignore = "requires a dedicated Restate server; run scripts/test-integration.sh"]
async fn native_tasks_survive_reconnect_restart_and_support_input_revision_and_cancellation() {
    let admin = std::env::var("DEEPRESEARCH_TEST_RESTATE_ADMIN")
        .expect("isolated Restate admin URL required");
    let ingress = std::env::var("DEEPRESEARCH_TEST_RESTATE_INGRESS")
        .expect("isolated Restate ingress URL required");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if client
                .get(format!("{admin}/health"))
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("isolated Restate did not become ready");
    let mut application = Application::start(ingress);
    application.ready().await;
    let deployment=client.post(format!("{admin}/deployments")).json(&json!({"uri":format!("http://host.docker.internal:{}",application.workflow_port),"force":true})).send().await.unwrap();
    let status = deployment.status();
    let body = deployment.text().await.unwrap();
    assert!(status.is_success(), "registration failed: {status} {body}");
    let url = application.url();
    assert_eq!(client.post(&url).send().await.unwrap().status(), 401);
    let discovery = rpc(&client, &url, "server/discover", json!({})).await;
    assert!(
        discovery.get("result").is_some(),
        "discovery failed: {discovery}"
    );
    let tools = rpc(&client, &url, "tools/list", json!({})).await;
    for tool in tools["result"]["tools"].as_array().unwrap() {
        for hint in [
            "readOnlyHint",
            "destructiveHint",
            "idempotentHint",
            "openWorldHint",
        ] {
            assert!(tool["annotations"][hint].is_boolean());
        }
        assert!(
            tool["_meta"]["io.modelcontextprotocol/action-metadata"]["requiresReview"].is_boolean()
        );
    }
    let submit_tool = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "research_submit")
        .unwrap();
    assert_eq!(
        submit_tool["inputSchema"]["$defs"]["ResearchRequest"]["properties"]["attachments"]["items"]
            ["x-mcp-file"]["transferModes"][0],
        "upload"
    );
    assert!(submit_tool["outputSchema"]["properties"]["file"].is_object());
    let authorization = rpc(
        &client,
        &url,
        "files/authorizeUpload",
        json!({"name":"brief.md","mimeType":"text/markdown","size":43}),
    )
    .await;
    let uploaded = authorization["result"]["file"]["uri"].as_str().unwrap();
    let descriptor = &authorization["result"]["upload"];
    let upload = client
        .put(descriptor["url"].as_str().unwrap())
        .header(
            "Authorization",
            descriptor["headers"]["Authorization"].as_str().unwrap(),
        )
        .body("Attachment evidence: recovery is essential.")
        .send()
        .await
        .unwrap();
    assert_eq!(upload.status(), reqwest::StatusCode::CREATED);
    let id = Uuid::new_v4();
    let mut request = submit(id, None, 60);
    request["arguments"]["request"]["attachments"] = json!([uploaded]);
    let submitted = rpc(&client, &url, "tools/call", request).await;
    assert_eq!(submitted["result"]["resultType"], "task", "{submitted}");
    let task = submitted["result"]["taskId"].as_str().unwrap().to_owned();
    let retry = rpc(&client, &url, "tools/call", submit(id, None, 60)).await;
    assert_eq!(retry["result"]["taskId"], task);
    let detached_retry = rpc(
        &client,
        &url,
        "tools/call",
        submit(id, Some(Uuid::new_v4()), 60),
    )
    .await;
    assert_eq!(
        detached_retry["result"]["taskId"], task,
        "an existing request must not depend on parent availability"
    );
    let waiting = wait_status(&client, &url, &task, "input_required").await;
    application.restart();
    application.ready().await;
    let reconnected = reqwest::Client::new();
    let restored = wait_status(&reconnected, &url, &task, "input_required").await;
    assert_eq!(restored["inputRequests"], waiting["inputRequests"]);
    let question = restored["inputRequests"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    let unknown = rpc(
        &client,
        &url,
        "tasks/update",
        json!({"taskId":task,"inputResponses":{"unknown":{"action":"cancel"}}}),
    )
    .await;
    assert_eq!(unknown["result"]["resultType"], "complete");
    let still_waiting = wait_status(&client, &url, &task, "input_required").await;
    assert_eq!(still_waiting["inputRequests"], restored["inputRequests"]);
    let updated=rpc(&client,&url,"tasks/update",json!({"taskId":task,"inputResponses":{question.clone():{"action":"accept","content":{"answer":"Recovery matters most."}}}})).await;
    assert_eq!(updated["result"]["resultType"], "complete", "{updated}");
    for action in ["accept", "cancel"] {
        let replay = rpc(&client,&url,"tasks/update",json!({"taskId":task,"inputResponses":{question.clone():{"action":action,"content":{"answer":"Recovery matters most."}}}})).await;
        assert_eq!(replay["result"]["resultType"], "complete", "{replay}");
    }
    let completed = wait_status(&client, &url, &task, "completed").await;
    let report = completed["result"]["structuredContent"]["report"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        report.contains("[S1]") || report.contains("[^S1]"),
        "{report}"
    );
    let _ = rpc(&client, &url, "tasks/cancel", json!({"taskId":task})).await;
    assert_eq!(
        wait_status(&client, &url, &task, "completed").await["result"],
        completed["result"]
    );
    let report_file = completed["result"]["structuredContent"]["file"]["uri"]
        .as_str()
        .unwrap();
    let first_download = rpc(
        &client,
        &url,
        "files/authorizeDownload",
        json!({"uri":report_file}),
    )
    .await;
    let descriptor = &first_download["result"]["download"];
    assert_eq!(
        client
            .get(descriptor["url"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    application.restart();
    application.ready().await;
    assert_eq!(
        client
            .get(descriptor["url"].as_str().unwrap())
            .header(
                "Authorization",
                descriptor["headers"]["Authorization"].as_str().unwrap()
            )
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    let refreshed = rpc(
        &client,
        &url,
        "files/authorizeDownload",
        json!({"uri":report_file}),
    )
    .await;
    let descriptor = &refreshed["result"]["download"];
    let downloaded = client
        .get(descriptor["url"].as_str().unwrap())
        .header(
            "Authorization",
            descriptor["headers"]["Authorization"].as_str().unwrap(),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(downloaded, report);
    let upload_path = application.directory.join("files").join("uploads");
    std::fs::remove_dir_all(&upload_path).unwrap();
    std::fs::create_dir(&upload_path).unwrap();
    let revision = rpc(
        &client,
        &url,
        "tools/call",
        submit(Uuid::new_v4(), Some(id), 60),
    )
    .await;
    let revision_task = revision["result"]["taskId"].as_str().unwrap();
    assert_ne!(revision_task, task);
    let _ = wait_status(&client, &url, revision_task, "input_required").await;
    let cancelled = rpc(
        &client,
        &url,
        "tasks/cancel",
        json!({"taskId":revision_task}),
    )
    .await;
    assert_eq!(cancelled["result"]["resultType"], "complete");
    let _ = wait_status(&client, &url, revision_task, "cancelled").await;
    let original = wait_status(&client, &url, &task, "completed").await;
    assert_eq!(original["result"]["structuredContent"]["report"], report);
    let active = rpc(
        &client,
        &url,
        "tools/call",
        submit(Uuid::new_v4(), None, 60),
    )
    .await;
    let active_task = active["result"]["taskId"].as_str().unwrap();
    let _ = rpc(&client, &url, "tasks/cancel", json!({"taskId":active_task})).await;
    let _ = wait_status(&client, &url, active_task, "cancelled").await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    let late = rpc(&client, &url, "tasks/get", json!({"taskId":active_task})).await;
    assert_eq!(late["result"]["status"], "cancelled");
    let deadline = rpc(
        &client,
        &url,
        "tools/call",
        submit(Uuid::new_v4(), None, 10),
    )
    .await;
    let exhausted = wait_status(
        &client,
        &url,
        deadline["result"]["taskId"].as_str().unwrap(),
        "completed",
    )
    .await;
    assert_eq!(
        exhausted["result"]["structuredContent"]["status"]["state"],
        "exhausted"
    );
    assert!(
        exhausted["result"]["structuredContent"]["report"]
            .as_str()
            .unwrap()
            .contains("wall-clock")
    );
}
