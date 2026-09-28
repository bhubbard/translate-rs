use std::time::Instant;
use translate_rs::detect::LanguageDetector;
use translate_rs::engine::{AppleTranslator, Translating};
use translate_rs::masker::TranslationMasker;
use translate_rs::stream::{LineSplitter, ParagraphSplitter};

#[tokio::test]
async fn test_performance_masker_throughput_and_latency() {
    let document = "## System Overview\n\n\
        The `translate-rs` daemon exposes an HTTP server at `http://127.0.0.1:8080/translate`.\n\
        ```json\n\
        {\"q\": \"Hello world\", \"source\": \"en\", \"target\": \"es\"}\n\
        ```\n\
        For questions, contact `dev@example.com` or consult the [documentation](https://code.brandonhubbard.com/translate-rs/).\n\
        Use `cargo run --release` for maximum performance on Apple Silicon.";

    // Warmup
    for _ in 0..100 {
        let _ = TranslationMasker::segments(document, true);
    }

    let iterations = 2_000;
    let start = Instant::now();
    for _ in 0..iterations {
        let segments = TranslationMasker::segments(document, true);
        assert!(!segments.is_empty());
    }
    let elapsed = start.elapsed();
    let per_op = elapsed.as_secs_f64() / (iterations as f64);
    let per_op_micros = per_op * 1_000_000.0;
    let ops_per_sec = (iterations as f64) / elapsed.as_secs_f64();

    println!("Masker Performance: {:.2} µs/op ({:.0} ops/sec)", per_op_micros, ops_per_sec);

    // Performance assertion: Masking complex Markdown must complete under 150 µs/op in debug/test mode
    assert!(
        per_op_micros < 150.0,
        "Masker latency must stay under 150µs/op, measured: {per_op_micros:.2}µs"
    );
}

#[tokio::test]
async fn test_performance_stream_splitters_throughput() {
    let mut line_splitter = LineSplitter::new();
    let chunk = "Line 1: High performance streaming translation.\n\
                 Line 2: Zero heap fragmentation.\n\
                 Line 3: Instant newline emission.\n";

    let iterations = 5_000;
    let start = Instant::now();
    let mut total_tokens = 0;
    for _ in 0..iterations {
        let tokens = line_splitter.feed(chunk);
        total_tokens += tokens.len();
    }
    let elapsed = start.elapsed();
    let tokens_per_sec = (total_tokens as f64) / elapsed.as_secs_f64();

    println!("Line Splitter: {} tokens processed in {:?} ({:.0} tokens/sec)", total_tokens, elapsed, tokens_per_sec);
    assert!(total_tokens > 0);
    assert!(tokens_per_sec > 100_000.0, "Splitter throughput must exceed 100,000 tokens/sec");

    // Paragraph Splitter
    let mut para_splitter = ParagraphSplitter::new();
    let para_chunk = "Paragraph 1 text.\n\nParagraph 2 text with more contents.\n\n";
    let p_start = Instant::now();
    let mut total_paras = 0;
    for _ in 0..iterations {
        let paras = para_splitter.feed(para_chunk);
        total_paras += paras.len();
    }
    let p_elapsed = p_start.elapsed();
    assert!(total_paras > 0);
    assert!(p_elapsed.as_millis() < 500, "Paragraph splitter must process 10k paragraphs under 500ms");
}

#[tokio::test]
async fn test_performance_language_detection_latency() {
    let hints: Vec<String> = vec![];
    let detector = LanguageDetector::new(&hints);
    let samples = [
        "This is an English sentence for language identification benchmark testing.",
        "Esta es una frase en español para probar el rendimiento del detector.",
        "Dies ist ein deutscher Satz zur Überprüfung der Spracherkennung.",
        "Ceci est une phrase française pour tester la détection de langue.",
        "これは言語検出のベンチマークテスト用の日本語の文です。",
    ];

    let iterations = 20;
    let start = Instant::now();
    for _ in 0..iterations {
        for sample in &samples {
            let res = detector.detect(sample);
            assert!(res.is_ok());
        }
    }
    let elapsed = start.elapsed();
    let total_detections = iterations * samples.len();
    let per_detection_micros = (elapsed.as_secs_f64() / total_detections as f64) * 1_000_000.0;

    println!("Detector Performance: {} detections in {:?} ({:.2} ms/op)", total_detections, elapsed, per_detection_micros / 1000.0);
    assert!(
        per_detection_micros < 50_000.0,
        "Language detection must be under 50ms/op, measured: {:.2}ms",
        per_detection_micros / 1000.0
    );
}

#[tokio::test]
async fn test_performance_live_engine_translation_responsiveness() {
    let translator = AppleTranslator::new();
    let status = translator.pair_status("en", "es").await;

    if status != translate_rs::types::PairStatus::Installed {
        eprintln!("Skipping engine latency test: en-es not installed");
        return;
    }

    let query = vec!["Fast deterministic on-device neural translation.".to_string()];
    let start = Instant::now();
    let res = translator.translate(&query, "en", "es", false).await;
    let elapsed = start.elapsed();

    assert!(res.is_ok());
    println!("Live Single Sentence Translation Latency: {:?}", elapsed);
    assert!(
        elapsed.as_secs() < 4,
        "Translation took too long: {:?}", elapsed
    );
}
