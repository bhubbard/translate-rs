use std::sync::Arc;
use tokio::net::TcpListener;
use translate_rs::engine::AppleTranslator;
use translate_rs::server::{create_router, ServerState};

#[tokio::test]
async fn test_server_endpoints() {
    let state = Arc::new(ServerState {
        translator: Arc::new(AppleTranslator::new()),
        api_key: None,
    });
    let app = create_router(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();

    // 1. Health check
    let resp = client
        .get(format!("http://{addr}/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // 2. DeepL languages
    let resp = client
        .get(format!("http://{addr}/v2/languages"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json.as_array().unwrap().len() > 20);

    // 3. DeepL translate JSON
    let resp = client
        .post(format!("http://{addr}/v2/translate"))
        .json(&serde_json::json!({
            "text": ["Good morning"],
            "target_lang": "ES"
        }))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let body_text = resp.text().await.unwrap();
    assert_eq!(status, 200, "Error body: {body_text}");
    let json: serde_json::Value = serde_json::from_str(&body_text).unwrap();
    assert!(json["translations"][0]["text"].as_str().is_some());

    // 4. LibreTranslate languages
    let resp = client
        .get(format!("http://{addr}/languages"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // 5. LibreTranslate detect
    let resp = client
        .post(format!("http://{addr}/detect"))
        .json(&serde_json::json!({
            "q": "Bonjour mon ami"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json[0]["language"], "fr");

    // 6. Google translate v2 languages
    let resp = client
        .get(format!("http://{addr}/language/translate/v2/languages"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["data"]["languages"].as_array().unwrap().len() > 20);
}
