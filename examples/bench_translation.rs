use std::time::Instant;
use translate_rs::engine::{AppleTranslator, Translating};
use translate_rs::masker::TranslationMasker;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("===============================================================");
    println!("  translate-rs — Performance & Translation Benchmark Runner");
    println!("===============================================================\n");

    let translator = AppleTranslator::new();

    // Check status
    let status = translator.pair_status("en", "es").await;
    println!("Checking 'en' -> 'es' language status: {:?}", status);

    // Warm-up
    println!("\nWarming up translation engine...");
    let warmup_text = vec!["Hello, world! Welcome to offline neural translation.".to_string()];
    let _ = translator.translate(&warmup_text, "en", "es", false).await;

    // Test Corpora
    let single_sentence = vec![
        "Artificial intelligence and neural machine translation have revolutionized cross-border communication on local devices.".to_string()
    ];

    let paragraph = vec![
        "Artificial intelligence and machine learning technologies have fundamentally transformed how software developers approach localization. By running neural models entirely on-device, modern applications can preserve user privacy, eliminate network latency, and operate seamlessly in offline environments. Furthermore, system resources such as memory footprint and binary size are critical considerations for background daemons.".to_string()
    ];

    let doc_batch = vec![
        "Item 1: The rapid advancement of systems programming in Rust empowers developers to build safe, concurrent, and blazing-fast tools without compromising on reliability.".to_string(),
        "Item 2: On-device neural machine translation ensures that sensitive text, personal communications, and proprietary documents never leave local storage.".to_string(),
        "Item 3: Low memory overhead is critical for system background daemons to avoid triggering operating system memory pressure or paging.".to_string(),
        "Item 4: Full compatibility with DeepL and LibreTranslate REST APIs allows drop-in replacement across web extensions and desktop applications.".to_string(),
    ];

    println!("\n--- Benchmark 1: Single Sentence Latency ---");
    run_benchmark(&translator, "Single Sentence (~25 words)", &single_sentence, 2).await;

    println!("\n--- Benchmark 2: Paragraph Latency ---");
    run_benchmark(&translator, "Paragraph (~60 words)", &paragraph, 2).await;

    println!("\n--- Benchmark 3: Document Batch Latency ---");
    run_benchmark(&translator, "Document Batch (4 sentences / ~100 words)", &doc_batch, 2).await;

    // Masking Benchmark
    println!("\n--- Benchmark 4: Inline Code & URL Masking Engine ---");
    let markdown_sample = "Check out `https://code.brandonhubbard.com/translate-rs/` and email dev@example.com for `cargo run` details.";
    let start = Instant::now();
    for _ in 0..10_000 {
        let _ = TranslationMasker::segments(markdown_sample, true);
    }
    let elapsed = start.elapsed();
    let per_op_us = elapsed.as_micros() as f64 / 10_000.0;
    println!("10,000 Masker Segments passes completed in {:?}", elapsed);
    println!("Average Masker Latency: {:.2} µs/op ({:.0} ops/sec)", per_op_us, 1_000_000.0 / per_op_us);

    println!("\n===============================================================");
    println!("  Benchmark Completed Successfully");
    println!("===============================================================");
    Ok(())
}

async fn run_benchmark(
    translator: &AppleTranslator,
    name: &str,
    input: &[String],
    iterations: usize,
) {
    let word_count: usize = input.iter().map(|s| s.split_whitespace().count()).sum();
    let char_count: usize = input.iter().map(|s| s.len()).sum();

    let mut durations = Vec::new();

    for _ in 0..iterations {
        let t0 = Instant::now();
        let res = translator.translate(input, "en", "es", false).await;
        let dur = t0.elapsed();
        if let Ok(translated) = res {
            durations.push(dur);
            if durations.len() == 1 && !translated.is_empty() {
                println!("  Sample Output: \"{}\"", translated[0].lines().next().unwrap_or(""));
            }
        }
    }

    if durations.is_empty() {
        println!("  {}: Failed to execute translation.", name);
        return;
    }

    let min_dur = durations.iter().min().unwrap();
    let sum_millis: f64 = durations.iter().map(|d| d.as_secs_f64() * 1000.0).sum();
    let avg_millis = sum_millis / (durations.len() as f64);
    let words_per_sec = (word_count as f64) / (avg_millis / 1000.0);

    println!("  Workload: {}", name);
    println!("  Total Words: {}, Total Chars: {}", word_count, char_count);
    println!("  Min Latency: {:.2} ms | Avg Latency: {:.2} ms", min_dur.as_secs_f64() * 1000.0, avg_millis);
    println!("  Throughput: {:.0} words/sec", words_per_sec);
}
