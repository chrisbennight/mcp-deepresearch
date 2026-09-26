use mcp_deepresearch::{files::FileStore, research::ResearchId};
use serde_json::json;

#[tokio::test]
async fn files_require_owner_and_transfer_authority_without_rerunning_research() {
    let root = std::env::temp_dir().join(format!("research-files-test-{}", uuid::Uuid::new_v4()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let files = FileStore::new(root.clone(), origin.clone(), "owner-a".into()).unwrap();
    let router = files.router();
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = reqwest::Client::new();
    let failed = files
        .authorize_upload(json!({"name":"source.txt","size":100}))
        .await
        .unwrap();
    let descriptor = &failed["upload"];
    let response = client
        .put(descriptor["url"].as_str().unwrap())
        .header(
            "Authorization",
            descriptor["headers"]["Authorization"].as_str().unwrap(),
        )
        .body("short")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        files
            .authorize_download(failed["file"]["uri"].as_str().unwrap())
            .await
            .is_err()
    );
    assert!(
        files
            .authorize_upload(json!({"mimeType":"application/pdf"}))
            .await
            .is_err()
    );
    let report = files
        .publish_report(ResearchId::default(), "# A cited report\n")
        .await
        .unwrap();
    let other = FileStore::new(root.clone(), origin, "owner-b".into()).unwrap();
    assert!(other.authorize_download(&report.uri).await.is_err());
    assert!(
        files
            .authorize_download("mcp-file://deepresearch/reports/../../secret")
            .await
            .is_err()
    );
    let authorization = files.authorize_download(&report.uri).await.unwrap();
    let descriptor = &authorization["download"];
    assert_eq!(
        client
            .get(descriptor["url"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    let response = client
        .get(descriptor["url"].as_str().unwrap())
        .header(
            "Authorization",
            descriptor["headers"]["Authorization"].as_str().unwrap(),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.text().await.unwrap(), "# A cited report\n");
    server.abort();
    std::fs::remove_dir_all(root).unwrap();
}
