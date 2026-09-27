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
    pub text: Option<serde_json::Value>,
    pub target_lang: Option<String>,
    pub source_lang: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct LibreParams {
    pub q: Option<serde_json::Value>,
    pub source: Option<String>,
    pub target: Option<String>,
    pub format: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct GoogleParams {
    pub q: Option<serde_json::Value>,
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

fn check_auth(state: &ServerState, headers: &HeaderMap, query_key: Option<&str>) -> bool {
    let Some(ref expected) = state.api_key else {
        return true;
    };

    if let Some(auth) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
        if auth.trim_start_matches("Bearer ").trim() == expected {
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

    if let Some(key) = query_key {
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
    if !check_auth(&state, &headers, None) {
        return json_response(
            StatusCode::FORBIDDEN,
            serde_json::json!({ "message": "Invalid API key" }),
        );
    }

    let params: DeepLParams = parse_json_or_form(&headers, &body);
    let Some(target) = params.target_lang else {
        return json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "message": "Missing 'target_lang' parameter" }),
        );
    };

    let texts = extract_strings(params.text);
    if texts.is_empty() {
        return json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "message": "Missing 'text' parameter" }),
        );
    }

    let joined = texts.join("\n");
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

    match state.translator.translate(&texts, &src, &dst, true).await {
        Ok(translated) => {
            let translations: Vec<serde_json::Value> = translated
                .into_iter()
                .map(|text| {
                    serde_json::json!({
                        "detected_source_language": src.to_uppercase(),
                        "text": text
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

async fn deepl_languages_handler() -> Response {
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
    let params: LibreParams = parse_json_or_form(&headers, &body);

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

    let is_array = matches!(params.q, Some(serde_json::Value::Array(_)));
    let texts = extract_strings(params.q);
    if texts.is_empty() {
        return json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": "Missing 'q' parameter" }),
        );
    }

    let joined = texts.join("\n");
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

    match state.translator.translate(&texts, &src, &dst, true).await {
        Ok(translated) => {
            if is_array {
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
    let params: LibreParams = parse_json_or_form(&headers, &body);
    let texts = extract_strings(params.q);
    let joined = texts.join("\n");

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
    let q = query.get("q").cloned().map(serde_json::Value::String);
    let key = query.get("key").cloned();

    handle_google_translate(state, headers, key.as_deref(), target, source, q).await
}

async fn google_translate_post_handler(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let params: GoogleParams = parse_json_or_form(&headers, &body);
    handle_google_translate(
        state,
        headers,
        params.key.as_deref(),
        params.target,
        params.source,
        params.q,
    )
    .await
}

async fn handle_google_translate(
    state: Arc<ServerState>,
    headers: HeaderMap,
    key: Option<&str>,
    target: Option<String>,
    source: Option<String>,
    q: Option<serde_json::Value>,
) -> Response {
    if !check_auth(&state, &headers, key) {
        return google_error(StatusCode::UNAUTHORIZED, "Invalid API key");
    }

    let Some(target) = target else {
        return google_error(StatusCode::BAD_REQUEST, "Missing 'target' parameter");
    };

    let texts = extract_strings(q);
    if texts.is_empty() {
        return google_error(StatusCode::BAD_REQUEST, "Missing 'q' parameter");
    }

    let joined = texts.join("\n");
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

    match state.translator.translate(&texts, &src, &dst, true).await {
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
    let q = query.get("q").cloned().map(serde_json::Value::String);
    handle_google_detect(q).await
}

async fn google_detect_post_handler(headers: HeaderMap, body: Bytes) -> Response {
    let params: GoogleParams = parse_json_or_form(&headers, &body);
    handle_google_detect(params.q).await
}

async fn handle_google_detect(q: Option<serde_json::Value>) -> Response {
    let texts = extract_strings(q);
    let joined = texts.join("\n");
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

fn parse_json_or_form<T: serde::de::DeserializeOwned + Default>(
    headers: &HeaderMap,
    body: &[u8],
) -> T {
    let content_type = headers
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    if content_type.contains("application/json") {
        if let Ok(val) = serde_json::from_slice(body) {
            return val;
        }
    }

    if let Ok(val) = serde_urlencoded::from_bytes(body) {
        return val;
    }

    if let Ok(val) = serde_json::from_slice(body) {
        return val;
    }

    T::default()
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
