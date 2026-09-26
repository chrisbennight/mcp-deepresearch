//! Draft SEP-2631 at the MCP edge; transfer tickets never enter research state.
use axum::{
    Router,
    body::Body,
    extract::{Path as HttpPath, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
    routing::{get, put},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{io::AsyncWriteExt, sync::Mutex};
use uuid::Uuid;

pub const MAX_BYTES: usize = 16 * 1024 * 1024;
const TICKET_SECONDS: u64 = 300;
#[derive(Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileValue {
    pub uri: String,
    pub name: String,
    pub mime_type: String,
    pub size: u64,
    pub digest: FileDigest,
}
#[derive(Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FileDigest {
    pub algorithm: String,
    pub value: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadRequest {
    pub name: Option<String>,
    pub mime_type: Option<String>,
    pub size: Option<u64>,
    pub digest: Option<FileDigest>,
}
#[derive(Serialize, Deserialize)]
struct StoredFile {
    owner: String,
    created_at: u64,
    file: FileValue,
}
#[derive(Clone)]
struct Ticket {
    key: FileKey,
    expires: u64,
    upload: Option<Arc<UploadRequest>>,
}
#[derive(Clone, PartialEq, Eq)]
struct FileKey {
    kind: &'static str,
    id: Uuid,
}
impl FileKey {
    fn parse(uri: &str) -> Result<Self, &'static str> {
        let value = uri
            .strip_prefix("mcp-file://deepresearch/")
            .ok_or("unknown research file")?;
        let (kind, id) = value.split_once('/').ok_or("invalid research file")?;
        let kind = match kind {
            "uploads" => "uploads",
            "reports" => "reports",
            _ => return Err("invalid research file"),
        };
        Ok(Self {
            kind,
            id: Uuid::parse_str(id).map_err(|_| "invalid research file")?,
        })
    }
    fn uri(&self) -> String {
        format!("mcp-file://deepresearch/{}/{}", self.kind, self.id)
    }
}
struct Inner {
    root: PathBuf,
    origin: String,
    owner: String,
    tickets: Mutex<HashMap<Uuid, Ticket>>,
    publication: Mutex<()>,
}
#[derive(Clone)]
pub struct FileStore(Arc<Inner>);
impl FileStore {
    pub fn new(
        root: PathBuf,
        origin: String,
        owner: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let url = reqwest::Url::parse(&origin)?;
        let local = url.scheme() == "http"
            && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
        if (url.scheme() != "https" && !local)
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err("file public origin must be HTTPS, or loopback HTTP for local use".into());
        }
        for kind in ["uploads", "reports"] {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(root.join(kind))?;
        }
        Ok(Self(Arc::new(Inner {
            root,
            origin: origin.trim_end_matches('/').into(),
            owner,
            tickets: Mutex::new(HashMap::new()),
            publication: Mutex::new(()),
        })))
    }
    fn path(&self, key: &FileKey) -> PathBuf {
        self.0.root.join(key.kind).join(key.id.to_string())
    }
    async fn lookup(&self, key: &FileKey) -> Result<FileValue, &'static str> {
        let bytes = tokio::fs::read(self.path(key).with_extension("json"))
            .await
            .map_err(|_| "research file is unavailable")?;
        let saved: StoredFile =
            serde_json::from_slice(&bytes).map_err(|_| "research file metadata is unavailable")?;
        if saved.owner != self.0.owner {
            return Err("research file is unavailable");
        }
        Ok(saved.file)
    }
    async fn save(
        &self,
        key: &FileKey,
        name: String,
        mime_type: String,
        size: u64,
        digest: FileDigest,
    ) -> Result<FileValue, &'static str> {
        let file = FileValue {
            uri: key.uri(),
            name,
            mime_type,
            size,
            digest,
        };
        let saved = StoredFile {
            owner: self.0.owner.clone(),
            created_at: crate::runtime::unix_seconds(),
            file: file.clone(),
        };
        let path = self.path(key).with_extension("json");
        let temporary = path.with_extension("json.part");
        tokio::fs::write(
            &temporary,
            serde_json::to_vec(&saved).expect("metadata serializes"),
        )
        .await
        .map_err(|_| "file metadata storage failed")?;
        tokio::fs::rename(temporary, path)
            .await
            .map_err(|_| "file publication failed")?;
        Ok(file)
    }
    async fn ticket(
        &self,
        key: FileKey,
        upload: Option<Arc<UploadRequest>>,
    ) -> Result<Value, &'static str> {
        let now = crate::runtime::unix_seconds();
        let mut tickets = self.0.tickets.lock().await;
        tickets.retain(|_, t| t.expires > now);
        if tickets.len() >= 1024 {
            return Err("file transfer capacity reached; retry later");
        }
        let token = Uuid::new_v4();
        let expires = now + TICKET_SECONDS;
        let method = if upload.is_some() { "PUT" } else { "GET" };
        let url = format!("{}/files/{}/{}", self.0.origin, key.kind, key.id);
        tickets.insert(
            token,
            Ticket {
                key,
                expires,
                upload,
            },
        );
        Ok(
            json!({"transport":"https","method":method,"url":url,"headers":{"Authorization":format!("Bearer {token}")},"expiresAt":jiff::Timestamp::from_second(expires as i64).expect("timestamp").to_string()}),
        )
    }
    pub async fn authorize_upload(&self, params: Value) -> Result<Value, &'static str> {
        let input: UploadRequest =
            serde_json::from_value(params).map_err(|_| "invalid upload request")?;
        if input.size.is_some_and(|size| size > MAX_BYTES as u64) {
            return Err("attachment exceeds the text ingestion limit");
        }
        if input.name.as_ref().is_some_and(|n| n.len() > 256) {
            return Err("attachment name is too long");
        }
        let mime = input.mime_type.as_deref().unwrap_or("text/plain");
        if !matches!(
            mime,
            "text/plain" | "text/markdown" | "application/json" | "text/csv"
        ) {
            return Err("upload extracted UTF-8 text, Markdown, JSON, or CSV");
        }
        if let Some(digest) = &input.digest
            && (digest.algorithm != "sha-256"
                || !URL_SAFE_NO_PAD
                    .decode(&digest.value)
                    .is_ok_and(|v| v.len() == 32))
        {
            return Err("unsupported file digest; use base64url SHA-256");
        }
        let key = FileKey {
            kind: "uploads",
            id: Uuid::new_v4(),
        };
        let file = json!({"uri":key.uri(),"name":input.name.as_deref().unwrap_or("attachment.txt"),"mimeType":mime,"size":input.size,"digest":input.digest});
        let upload = self.ticket(key, Some(Arc::new(input))).await?;
        Ok(json!({"file":file,"upload":upload}))
    }
    pub async fn authorize_download(&self, uri: &str) -> Result<Value, &'static str> {
        let key = FileKey::parse(uri)?;
        let file = self.lookup(&key).await?;
        let download = self.ticket(key, None).await?;
        Ok(json!({"file":file,"download":download}))
    }
    pub async fn attachment(&self, uri: &str) -> Result<(FileValue, PathBuf), &'static str> {
        let key = FileKey::parse(uri)?;
        if key.kind != "uploads" {
            return Err("attachment must be an admitted upload");
        }
        Ok((self.lookup(&key).await?, self.path(&key)))
    }
    pub async fn admit_attachments(
        &self,
        id: crate::research::ResearchId,
        uris: &[String],
        previous: Option<&crate::workspace::Workspace>,
    ) -> Result<Vec<crate::research::Attachment>, &'static str> {
        if uris.len() > 16 {
            return Err("at most sixteen attachments per request");
        }
        let directory = self.0.root.join("research").join(id.to_string());
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&directory)
            .map_err(|_| "attachment storage unavailable")?;
        let mut attachments = Vec::new();
        if let Some(previous) = previous {
            for attachment in &previous.attachments {
                let from = self
                    .0
                    .root
                    .join("research")
                    .join(previous.id.to_string())
                    .join(attachment.id.to_string());
                retain_attachment(&from, &directory.join(attachment.id.to_string())).await?;
                attachments.push(attachment.clone());
            }
        }
        for uri in uris {
            if attachments.iter().any(|a| a.uri == *uri) {
                continue;
            }
            let (file, path) = self.attachment(uri).await?;
            let key = FileKey::parse(uri)?;
            retain_attachment(&path, &directory.join(key.id.to_string())).await?;
            let text = tokio::fs::read_to_string(directory.join(key.id.to_string()))
                .await
                .map_err(|_| "attachment is not readable text")?;
            let mut excerpt: String = text.chars().take(8000).collect();
            if excerpt.len() < text.len() {
                excerpt.push_str("\n[Excerpt truncated; use read_source_material with the attachment material ID for further sections.]");
            }
            if excerpt.trim().is_empty() {
                return Err("attachment contains no text");
            }
            attachments.push(crate::research::Attachment {
                id: key.id,
                name: file.name,
                uri: file.uri,
                excerpt,
            });
        }
        if attachments.len() > 16 {
            return Err("at most sixteen attachments including inherited material");
        }
        Ok(attachments)
    }
    pub async fn publish_report(
        &self,
        id: crate::research::ResearchId,
        report: &str,
    ) -> Result<FileValue, &'static str> {
        let key = FileKey {
            kind: "reports",
            id: id.0,
        };
        let _guard = self.0.publication.lock().await;
        if self.path(&key).with_extension("json").exists() {
            return self.lookup(&key).await;
        }
        let temporary = self.path(&key).with_extension("part");
        tokio::fs::write(&temporary, report)
            .await
            .map_err(|_| "report storage unavailable")?;
        tokio::fs::rename(&temporary, self.path(&key))
            .await
            .map_err(|_| "report publication failed")?;
        self.save(
            &key,
            "research-report.md".into(),
            "text/markdown".into(),
            report.len() as u64,
            hash(report.as_bytes()),
        )
        .await
    }
    pub fn router(&self) -> Router {
        Router::new()
            .route("/files/{kind}/{id}", get(download).merge(put(upload)))
            .with_state(self.clone())
    }
    async fn admit(
        &self,
        kind: &str,
        id: &str,
        headers: &HeaderMap,
        upload: bool,
    ) -> Result<Ticket, StatusCode> {
        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|s| s.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;
        let mut tickets = self.0.tickets.lock().await;
        let ticket = tickets.get(&token).ok_or(StatusCode::UNAUTHORIZED)?;
        if ticket.expires <= crate::runtime::unix_seconds()
            || ticket.key.kind != kind
            || ticket.key.id.to_string() != id
            || ticket.upload.is_some() != upload
        {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let ticket = ticket.clone();
        if upload {
            tickets.remove(&token);
        }
        Ok(ticket)
    }
}
fn hash(bytes: &[u8]) -> FileDigest {
    FileDigest {
        algorithm: "sha-256".into(),
        value: URL_SAFE_NO_PAD.encode(Sha256::digest(bytes)),
    }
}
struct PartialFile(PathBuf);
impl Drop for PartialFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
async fn upload(
    State(store): State<FileStore>,
    HttpPath((kind, id)): HttpPath<(String, String)>,
    headers: HeaderMap,
    body: Body,
) -> Result<StatusCode, StatusCode> {
    let ticket = store.admit(&kind, &id, &headers, true).await?;
    let input = ticket.upload.expect("upload ticket");
    let path = store.path(&ticket.key);
    let partial = PartialFile(path.with_extension("part"));
    let mut file = tokio::fs::File::create(&partial.0)
        .await
        .map_err(|_| StatusCode::INSUFFICIENT_STORAGE)?;
    let mut stream = body.into_data_stream();
    let mut size = 0usize;
    let mut digest = Sha256::new();
    tokio::time::timeout(Duration::from_secs(TICKET_SECONDS), async {
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| StatusCode::BAD_REQUEST)?;
            size = size
                .checked_add(chunk.len())
                .ok_or(StatusCode::PAYLOAD_TOO_LARGE)?;
            if size > MAX_BYTES {
                return Err(StatusCode::PAYLOAD_TOO_LARGE);
            }
            digest.update(&chunk);
            file.write_all(&chunk)
                .await
                .map_err(|_| StatusCode::INSUFFICIENT_STORAGE)?;
        }
        file.flush()
            .await
            .map_err(|_| StatusCode::INSUFFICIENT_STORAGE)
    })
    .await
    .map_err(|_| StatusCode::REQUEST_TIMEOUT)??;
    drop(file);
    let digest = FileDigest {
        algorithm: "sha-256".into(),
        value: URL_SAFE_NO_PAD.encode(digest.finalize()),
    };
    if input.size.is_some_and(|n| n != size as u64)
        || input
            .digest
            .as_ref()
            .is_some_and(|d| d.value != digest.value)
    {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    tokio::fs::read_to_string(&partial.0)
        .await
        .map_err(|_| StatusCode::UNSUPPORTED_MEDIA_TYPE)?;
    tokio::fs::rename(&partial.0, &path)
        .await
        .map_err(|_| StatusCode::INSUFFICIENT_STORAGE)?;
    store
        .save(
            &ticket.key,
            input
                .name
                .clone()
                .unwrap_or_else(|| "attachment.txt".into()),
            input
                .mime_type
                .clone()
                .unwrap_or_else(|| "text/plain".into()),
            size as u64,
            digest,
        )
        .await
        .map_err(|_| StatusCode::INSUFFICIENT_STORAGE)?;
    Ok(StatusCode::CREATED)
}
async fn download(
    State(store): State<FileStore>,
    HttpPath((kind, id)): HttpPath<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let ticket = store.admit(&kind, &id, &headers, false).await?;
    let value = store
        .lookup(&ticket.key)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let file = tokio::fs::File::open(store.path(&ticket.key))
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Response::builder()
        .header(header::CONTENT_TYPE, value.mime_type)
        .header(header::CONTENT_LENGTH, value.size)
        .header(header::CACHE_CONTROL, "private, no-store")
        .body(Body::from_stream(tokio_util::io::ReaderStream::new(file)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub fn client_supports(meta: &rmcp::model::MetaObject, direction: &str) -> bool {
    meta.get("io.modelcontextprotocol/clientCapabilities")
        .and_then(|c| c.get("files"))
        .is_some_and(|files| {
            files.get(direction).and_then(Value::as_bool) == Some(true)
                && files
                    .get("transports")
                    .and_then(Value::as_array)
                    .is_some_and(|v| v.iter().any(|s| s == "https"))
        })
}
pub fn attachment_path(root: &Path, uri: &str) -> Result<PathBuf, &'static str> {
    let key = FileKey::parse(uri)?;
    if key.kind != "uploads" {
        return Err("not an attachment");
    }
    Ok(root.join(key.kind).join(key.id.to_string()))
}

async fn retain_attachment(from: &Path, to: &Path) -> Result<(), &'static str> {
    match tokio::fs::hard_link(from, to).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(_) => Err("attachment retention failed"),
    }
}
