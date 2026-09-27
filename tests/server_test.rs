use std::sync::Arc;
use tokio::net::TcpListener;
use translate_rs::engine::AppleTranslator;
use translate_rs::server::{create_router, ServerState};

#[tokio::test]
async fn test_server_endpoints() {
    let state = Arc::new(ServerState {
        translator: Arc::new(AppleTranslator::new()),
        api_key: Some("secret-key".to_string()),
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
        .get(format!("http://{addr}/v2/languages?type=target"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json.as_array().unwrap().len() > 20);
    assert_eq!(json[0]["language"].as_str().unwrap(), json[0]["language"].as_str().unwrap().to_uppercase());

    // 3. DeepL translate JSON with Auth header
    let resp = client
        .post(format!("http://{addr}/v2/translate"))
        .header("Authorization", "DeepL-Auth-Key secret-key")
        .json(&serde_json::json!({
            "text": ["Good morning"],
            "target_lang": "ES"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["translations"][0]["text"].as_str().is_some());
    assert!(json["translations"][0]["billed_characters"].as_u64().is_some());

    // 4. DeepL translate Form URL-encoded with multiple text values and ignored params
    let form_body = "text=Hello&text=World&target_lang=ES&formality=more&tag_handling=xml&auth_key=secret-key";
    let resp = client
        .post(format!("http://{addr}/v2/translate"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form_body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    let translations = json["translations"].as_array().unwrap();
    assert_eq!(translations.len(), 2);

    // 5. DeepL error response shape on missing target_lang
    let resp = client
        .post(format!("http://{addr}/v2/translate"))
        .header("Authorization", "Bearer secret-key")
        .json(&serde_json::json!({
            "text": ["Hello"]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json.get("message").is_some());

    // 6. LibreTranslate languages
    let resp = client
        .get(format!("http://{addr}/languages"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // 7. LibreTranslate detect
    let resp = client
        .post(format!("http://{addr}/detect"))
        .json(&serde_json::json!({
            "q": "Bonjour tout le monde, comment allez-vous aujourd'hui?"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json[0]["language"], "fr");

    // 8. LibreTranslate array q response shape
    let resp = client
        .post(format!("http://{addr}/translate"))
        .json(&serde_json::json!({
            "q": ["Hello", "World"],
            "target": "es",
            "api_key": "secret-key"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["translatedText"].is_array());

    // 9. Google translate v2 languages
    let resp = client
        .get(format!("http://{addr}/language/translate/v2/languages"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["data"]["languages"].as_array().unwrap().len() > 20);

    // 10. Google translate v2 multiple q POST
    let resp = client
        .post(format!("http://{addr}/language/translate/v2"))
        .header("X-goog-api-key", "secret-key")
        .json(&serde_json::json!({
            "q": ["Hello", "World"],
            "target": "es"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["data"]["translations"].as_array().unwrap().len(), 2);
}
