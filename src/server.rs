use crate::detect::LanguageDetector;
use crate::engine::Translating;
use crate::types::{normalize_language_code, SUPPORTED_LANGUAGES};
use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

pub struct ServerState {
    pub translator: Arc<dyn Translating + Send + Sync>,
    pub api_key: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct DeepLParams {
    pub text: Vec<String>,
    pub target_lang: Option<String>,
    pub source_lang: Option<String>,
    pub auth_key: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct LibreParams {
    pub q: Vec<String>,
    pub q_was_array: bool,
    pub source: Option<String>,
    pub target: Option<String>,
    pub format: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct GoogleParams {
    pub qs: Vec<String>,
    pub target: Option<String>,
    pub source: Option<String>,
    pub key: Option<String>,
}

pub fn create_router(state: Arc<ServerState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", get(root_handler))
        .route("/health", get(health_handler))
        // DeepL endpoints
        .route("/v2/translate", post(deepl_translate_handler))
        .route("/v2/languages", get(deepl_languages_handler).post(deepl_languages_handler))
        // LibreTranslate endpoints
        .route("/translate", post(libre_translate_handler))
        .route("/detect", post(libre_detect_handler))
        .route("/languages", get(libre_languages_handler))
        .route("/frontend/settings", get(libre_settings_handler))
        .route("/suggest", get(libre_suggest_handler).post(libre_suggest_handler))
        // Google v2 endpoints
        .route(
            "/language/translate/v2",
            get(google_translate_get_handler).post(google_translate_post_handler),
        )
        .route(
            "/language/translate/v2/detect",
            get(google_detect_get_handler).post(google_detect_post_handler),
        )
        .route(
            "/language/translate/v2/languages",
            get(google_languages_get_handler).post(google_languages_post_handler),
        )
        .layer(cors)
        .with_state(state)
}

fn check_auth(state: &ServerState, headers: &HeaderMap, query_or_body_key: Option<&str>) -> bool {
    let Some(ref expected) = state.api_key else {
        return true;
    };

    if let Some(auth) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
        let trimmed = auth.trim();
        if let Some(rest) = trimmed.strip_prefix("DeepL-Auth-Key ") {
            if rest.trim() == expected {
                return true;
            }
        } else if let Some(rest) = trimmed.strip_prefix("Bearer ") {
            if rest.trim() == expected {
                return true;
            }
        } else if trimmed == expected {
            return true;
        }
    }

    if let Some(key) = headers.get("deepl-auth-key").and_then(|h| h.to_str().ok()) {
        if key.trim() == expected {
            return true;
        }
    }

    if let Some(key) = headers.get("x-goog-api-key").and_then(|h| h.to_str().ok()) {
        if key.trim() == expected {
            return true;
        }
    }

    if let Some(key) = query_or_body_key {
        if key == expected {
            return true;
        }
    }

    false
}

async fn root_handler() -> Response {
    json_response(
        StatusCode::OK,
        serde_json::json!({
            "name": "translate",
            "version": env!("CARGO_PKG_VERSION"),
            "status": "ok",
            "on_device": true,
            "engine": "Apple Translation Framework"
        }),
    )
}

async fn health_handler() -> Response {
    json_response(StatusCode::OK, serde_json::json!({ "status": "ok" }))
}

// ======================== DEEPL HANDLERS ========================

async fn deepl_translate_handler(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let params = parse_deepl_request(&headers, &body);

    if !check_auth(&state, &headers, params.auth_key.as_deref()) {
        return json_response(
            StatusCode::FORBIDDEN,
            serde_json::json!({ "message": "Invalid API key" }),
        );
    }

    let Some(target) = params.target_lang else {
        return json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "message": "Missing 'target_lang' parameter" }),
        );
    };

    if params.text.is_empty() {
        return json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "message": "Missing 'text' parameter" }),
        );
    }

    let joined = params.text.join("\n");
    let detector = LanguageDetector::new(&[]);
    let detection = match params.source_lang {
        Some(s) if !s.is_empty() => crate::types::DetectionResult {
            language_code: normalize_language_code(&s),
            confidence: 1.0,
        },
        _ => detector.detect(&joined).unwrap_or(crate::types::DetectionResult {
            language_code: "en".into(),
            confidence: 0.5,
        }),
    };

    let src = normalize_language_code(&detection.language_code);
    let dst = normalize_language_code(&target);

    match state.translator.translate(&params.text, &src, &dst, true).await {
        Ok(translated) => {
            let translations: Vec<serde_json::Value> = translated
                .into_iter()
                .map(|text| {
                    let billed = text.chars().count();
                    serde_json::json!({
                        "detected_source_language": src.to_uppercase(),
                        "text": text,
                        "billed_characters": billed
                    })
                })
                .collect();

            json_response(
                StatusCode::OK,
                serde_json::json!({ "translations": translations }),
            )
        }
        Err(e) => json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "message": e.to_string() }),
        ),
    }
}

async fn deepl_languages_handler(Query(_query): Query<HashMap<String, String>>) -> Response {
    let languages: Vec<serde_json::Value> = SUPPORTED_LANGUAGES
        .iter()
        .map(|(code, name)| {
            serde_json::json!({
                "language": code.to_uppercase(),
                "name": name
            })
        })
        .collect();

    json_response(StatusCode::OK, serde_json::Value::Array(languages))
}

// ======================== LIBRETRANSLATE HANDLERS ========================

async fn libre_translate_handler(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let params = parse_libre_request(&headers, &body);

    if !check_auth(&state, &headers, params.api_key.as_deref()) {
        return json_response(
            StatusCode::FORBIDDEN,
            serde_json::json!({ "error": "Invalid API key" }),
        );
    }

    let Some(target) = params.target else {
        return json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": "Missing 'target' parameter" }),
        );
    };

    if params.q.is_empty() {
        return json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": "Missing 'q' parameter" }),
        );
    }

    let joined = params.q.join("\n");
    let detector = LanguageDetector::new(&[]);
    let detection = match params.source.as_deref() {
        Some(s) if !s.is_empty() && s != "auto" => crate::types::DetectionResult {
            language_code: normalize_language_code(s),
            confidence: 1.0,
        },
        _ => detector.detect(&joined).unwrap_or(crate::types::DetectionResult {
            language_code: "en".into(),
            confidence: 0.5,
        }),
    };

    let src = normalize_language_code(&detection.language_code);
    let dst = normalize_language_code(&target);

    match state.translator.translate(&params.q, &src, &dst, true).await {
        Ok(translated) => {
            if params.q_was_array {
                json_response(
                    StatusCode::OK,
                    serde_json::json!({ "translatedText": translated }),
                )
            } else {
                let first = translated.into_iter().next().unwrap_or_default();
                json_response(
                    StatusCode::OK,
                    serde_json::json!({
                        "translatedText": first,
                        "detectedLanguage": {
                            "confidence": (detection.confidence * 100.0).round(),
                            "language": src
                        }
                    }),
                )
            }
        }
        Err(e) => json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": e.to_string() }),
        ),
    }
}

async fn libre_detect_handler(headers: HeaderMap, body: Bytes) -> Response {
    let params = parse_libre_request(&headers, &body);
    let joined = params.q.join("\n");

    let detector = LanguageDetector::new(&[]);
    let detection = detector.detect(&joined).unwrap_or(crate::types::DetectionResult {
        language_code: "en".into(),
        confidence: 0.5,
    });

    json_response(
        StatusCode::OK,
        serde_json::json!([
            {
                "confidence": (detection.confidence * 100.0).round(),
                "language": detection.language_code
            }
        ]),
    )
}

async fn libre_languages_handler() -> Response {
    let list: Vec<serde_json::Value> = SUPPORTED_LANGUAGES
        .iter()
        .map(|(code, name)| {
            let targets: Vec<&str> = SUPPORTED_LANGUAGES
                .iter()
                .filter(|(c, _)| c != code)
                .map(|(c, _)| *c)
                .collect();

            serde_json::json!({
                "code": code,
                "name": name,
                "targets": targets
            })
        })
        .collect();

    json_response(StatusCode::OK, serde_json::Value::Array(list))
}

async fn libre_settings_handler() -> Response {
    json_response(
        StatusCode::OK,
        serde_json::json!({
            "character_limit": 1000000,
            "frontend_timeout": 30000,
            "api_keys": false,
            "suggestions": false
        }),
    )
}

async fn libre_suggest_handler() -> Response {
    json_response(StatusCode::OK, serde_json::json!({ "success": true }))
}

// ======================== GOOGLE V2 HANDLERS ========================

async fn google_translate_get_handler(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let target = query.get("target").cloned();
    let source = query.get("source").cloned();
    let qs: Vec<String> = query.get("q").into_iter().cloned().collect();
    let key = query.get("key").cloned();

    handle_google_translate(state, headers, key.as_deref(), target, source, qs).await
}

async fn google_translate_post_handler(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let params = parse_google_request(&headers, &body);
    handle_google_translate(
        state,
        headers,
        params.key.as_deref(),
        params.target,
        params.source,
        params.qs,
    )
    .await
}

async fn handle_google_translate(
    state: Arc<ServerState>,
    headers: HeaderMap,
    key: Option<&str>,
    target: Option<String>,
    source: Option<String>,
    qs: Vec<String>,
) -> Response {
    if !check_auth(&state, &headers, key) {
        return google_error(StatusCode::UNAUTHORIZED, "Invalid API key");
    }

    let Some(target) = target else {
        return google_error(StatusCode::BAD_REQUEST, "Missing 'target' parameter");
    };

    if qs.is_empty() {
        return google_error(StatusCode::BAD_REQUEST, "Missing 'q' parameter");
    }

    let joined = qs.join("\n");
    let detector = LanguageDetector::new(&[]);
    let detection = match source.as_deref() {
        Some(s) if !s.is_empty() => crate::types::DetectionResult {
            language_code: normalize_language_code(s),
            confidence: 1.0,
        },
        _ => detector.detect(&joined).unwrap_or(crate::types::DetectionResult {
            language_code: "en".into(),
            confidence: 0.5,
        }),
    };

    let src = normalize_language_code(&detection.language_code);
    let dst = normalize_language_code(&target);

    match state.translator.translate(&qs, &src, &dst, true).await {
        Ok(translated) => {
            let translations: Vec<serde_json::Value> = translated
                .into_iter()
                .map(|t| {
                    serde_json::json!({
                        "translatedText": t,
                        "detectedSourceLanguage": src
                    })
                })
                .collect();

            json_response(
                StatusCode::OK,
                serde_json::json!({
                    "data": {
                        "translations": translations
                    }
                }),
            )
        }
        Err(e) => google_error(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

async fn google_detect_get_handler(Query(query): Query<HashMap<String, String>>) -> Response {
    let qs: Vec<String> = query.get("q").into_iter().cloned().collect();
    handle_google_detect(qs).await
}

async fn google_detect_post_handler(headers: HeaderMap, body: Bytes) -> Response {
    let params = parse_google_request(&headers, &body);
    handle_google_detect(params.qs).await
}

async fn handle_google_detect(qs: Vec<String>) -> Response {
    let joined = qs.join("\n");
    let detector = LanguageDetector::new(&[]);
    let detection = detector.detect(&joined).unwrap_or(crate::types::DetectionResult {
        language_code: "en".into(),
        confidence: 0.5,
    });

    json_response(
        StatusCode::OK,
        serde_json::json!({
            "data": {
                "detections": [[
                    {
                        "confidence": detection.confidence,
                        "isReliable": true,
                        "language": detection.language_code
                    }
                ]]
            }
        }),
    )
}

async fn google_languages_get_handler() -> Response {
    handle_google_languages().await
}

async fn google_languages_post_handler() -> Response {
    handle_google_languages().await
}

async fn handle_google_languages() -> Response {
    let languages: Vec<serde_json::Value> = SUPPORTED_LANGUAGES
        .iter()
        .map(|(code, name)| {
            serde_json::json!({
                "language": code,
                "name": name
            })
        })
        .collect();

    json_response(
        StatusCode::OK,
        serde_json::json!({
            "data": {
                "languages": languages
            }
        }),
    )
}

fn google_error(status: StatusCode, message: &str) -> Response {
    json_response(
        status,
        serde_json::json!({
            "error": {
                "code": status.as_u16(),
                "message": message,
                "errors": [{
                    "message": message,
                    "domain": "global",
                    "reason": "invalid"
                }]
            }
        }),
    )
}

// ======================== HELPERS ========================

fn json_response(status: StatusCode, value: serde_json::Value) -> Response {
    let body = serde_json::to_string(&value).unwrap_or_default();
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(body.into())
        .unwrap()
}

fn parse_deepl_request(headers: &HeaderMap, body: &[u8]) -> DeepLParams {
    let content_type = headers
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    if content_type.contains("application/json") {
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(body) {
            let mut params = DeepLParams::default();
            if let Some(target) = val.get("target_lang").and_then(|v| v.as_str()) {
                params.target_lang = Some(target.to_string());
            }
            if let Some(source) = val.get("source_lang").and_then(|v| v.as_str()) {
                params.source_lang = Some(source.to_string());
            }
            if let Some(auth) = val.get("auth_key").and_then(|v| v.as_str()) {
                params.auth_key = Some(auth.to_string());
            }
            if let Some(t) = val.get("text") {
                params.text = extract_strings(Some(t.clone()));
            }
            return params;
        }
    }

    let mut params = DeepLParams::default();
    for (k, v) in form_urlencoded::parse(body) {
        match k.as_ref() {
            "text" => params.text.push(v.into_owned()),
            "target_lang" => params.target_lang = Some(v.into_owned()),
            "source_lang" => params.source_lang = Some(v.into_owned()),
            "auth_key" => params.auth_key = Some(v.into_owned()),
            _ => {} // Ignore unknown fields like formality, tag_handling
        }
    }
    params
}

fn parse_libre_request(headers: &HeaderMap, body: &[u8]) -> LibreParams {
    let content_type = headers
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    if content_type.contains("application/json") {
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(body) {
            let mut params = LibreParams::default();
            if let Some(target) = val.get("target").and_then(|v| v.as_str()) {
                params.target = Some(target.to_string());
            }
            if let Some(source) = val.get("source").and_then(|v| v.as_str()) {
                params.source = Some(source.to_string());
            }
            if let Some(api_key) = val.get("api_key").and_then(|v| v.as_str()) {
                params.api_key = Some(api_key.to_string());
            }
            if let Some(format) = val.get("format").and_then(|v| v.as_str()) {
                params.format = Some(format.to_string());
            }
            if let Some(q_val) = val.get("q") {
                params.q_was_array = q_val.is_array();
                params.q = extract_strings(Some(q_val.clone()));
            }
            return params;
        }
    }

    let mut params = LibreParams::default();
    for (k, v) in form_urlencoded::parse(body) {
        match k.as_ref() {
            "q" => params.q.push(v.into_owned()),
            "target" => params.target = Some(v.into_owned()),
            "source" => params.source = Some(v.into_owned()),
            "api_key" => params.api_key = Some(v.into_owned()),
            "format" => params.format = Some(v.into_owned()),
            _ => {}
        }
    }
    params
}

fn parse_google_request(headers: &HeaderMap, body: &[u8]) -> GoogleParams {
    let content_type = headers
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    if content_type.contains("application/json") {
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(body) {
            let mut params = GoogleParams::default();
            if let Some(target) = val.get("target").and_then(|v| v.as_str()) {
                params.target = Some(target.to_string());
            }
            if let Some(source) = val.get("source").and_then(|v| v.as_str()) {
                params.source = Some(source.to_string());
            }
            if let Some(key) = val.get("key").and_then(|v| v.as_str()) {
                params.key = Some(key.to_string());
            }
            if let Some(q_val) = val.get("q") {
                params.qs = extract_strings(Some(q_val.clone()));
            }
            return params;
        }
    }

    let mut params = GoogleParams::default();
    for (k, v) in form_urlencoded::parse(body) {
        match k.as_ref() {
            "q" => params.qs.push(v.into_owned()),
            "target" => params.target = Some(v.into_owned()),
            "source" => params.source = Some(v.into_owned()),
            "key" => params.key = Some(v.into_owned()),
            _ => {}
        }
    }
    params
}

fn extract_strings(val: Option<serde_json::Value>) -> Vec<String> {
    match val {
        Some(serde_json::Value::String(s)) => vec![s],
        Some(serde_json::Value::Array(arr)) => arr
            .into_iter()
            .filter_map(|v| match v {
                serde_json::Value::String(s) => Some(s),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}
