use std::collections::HashMap;
use translate_rs::engine::{AppleTranslator, Translating};
use translate_rs::masker::TranslationMasker;

/// Computes BLEU-4 score with brevity penalty for evaluation
fn compute_sentence_bleu(hyp: &str, ref_str: &str) -> f64 {
    let tokenize = |s: &str| -> Vec<String> {
        s.to_lowercase()
            .replace(['.', ',', '!', '?', ':', ';', '(', ')', '"', '\''], " ")
            .split_whitespace()
            .map(|t| t.to_string())
            .collect()
    };

    let hyp_tokens = tokenize(hyp);
    let ref_tokens = tokenize(ref_str);

    if hyp_tokens.is_empty() || ref_tokens.is_empty() {
        return 0.0;
    }

    let mut precisions = Vec::new();

    for n in 1..=4 {
        if hyp_tokens.len() < n {
            break;
        }

        let mut hyp_ngrams: HashMap<Vec<String>, usize> = HashMap::new();
        for i in 0..=(hyp_tokens.len() - n) {
            let ng = hyp_tokens[i..(i + n)].to_vec();
            *hyp_ngrams.entry(ng).or_insert(0) += 1;
        }

        let mut ref_ngrams: HashMap<Vec<String>, usize> = HashMap::new();
        for i in 0..=(ref_tokens.len() - n) {
            let ng = ref_tokens[i..(i + n)].to_vec();
            *ref_ngrams.entry(ng).or_insert(0) += 1;
        }

        let mut clipped_matches = 0;
        let mut total_hyp_ngrams = 0;

        for (ng, count) in &hyp_ngrams {
            total_hyp_ngrams += count;
            if let Some(ref_count) = ref_ngrams.get(ng) {
                clipped_matches += count.min(ref_count);
            }
        }

        if total_hyp_ngrams > 0 {
            precisions.push((clipped_matches as f64) / (total_hyp_ngrams as f64));
        }
    }

    if precisions.is_empty() {
        return 0.0;
    }

    let bp = if hyp_tokens.len() > ref_tokens.len() {
        1.0
    } else {
        ((1.0 - (ref_tokens.len() as f64) / (hyp_tokens.len() as f64))).exp()
    };

    let log_sum: f64 = precisions.iter().map(|p| if *p > 0.0 { p.ln() } else { -9.0 }).sum();
    let score = bp * (log_sum / (precisions.len() as f64)).exp() * 100.0;
    score.clamp(0.0, 100.0)
}

/// Computes character n-gram F-score (ChrF)
fn compute_chrf(hyp: &str, ref_str: &str, max_n: usize) -> f64 {
    let clean = |s: &str| -> Vec<char> { s.chars().filter(|c| !c.is_whitespace()).collect() };
    let hyp_chars = clean(hyp);
    let ref_chars = clean(ref_str);

    if hyp_chars.is_empty() || ref_chars.is_empty() {
        return 0.0;
    }

    let mut total_p = 0.0;
    let mut total_r = 0.0;

    for n in 1..=max_n {
        let get_ngrams = |chars: &[char]| -> HashMap<Vec<char>, usize> {
            let mut map = HashMap::new();
            if chars.len() >= n {
                for i in 0..=(chars.len() - n) {
                    *map.entry(chars[i..(i + n)].to_vec()).or_insert(0) += 1;
                }
            }
            map
        };

        let hyp_map = get_ngrams(&hyp_chars);
        let ref_map = get_ngrams(&ref_chars);

        let mut overlap = 0;
        let hyp_count: usize = hyp_map.values().sum();
        let ref_count: usize = ref_map.values().sum();

        for (ng, count) in &hyp_map {
            if let Some(rc) = ref_map.get(ng) {
                overlap += count.min(rc);
            }
        }

        let p = if hyp_count > 0 { (overlap as f64) / (hyp_count as f64) } else { 0.0 };
        let r = if ref_count > 0 { (overlap as f64) / (ref_count as f64) } else { 0.0 };
        total_p += p;
        total_r += r;
    }

    let avg_p = total_p / (max_n as f64);
    let avg_r = total_r / (max_n as f64);

    if avg_p + avg_r == 0.0 {
        return 0.0;
    }

    let beta = 2.0;
    let b2 = beta * beta;
    ((1.0 + b2) * (avg_p * avg_r) / ((b2 * avg_p) + avg_r)) * 100.0
}

#[tokio::test]
async fn test_accuracy_bleu_and_chrf_metric_calculations() {
    let candidate = "The quick brown fox jumps over the lazy dog";
    let reference = "The quick brown fox jumps over the lazy dog";

    let bleu_identical = compute_sentence_bleu(candidate, reference);
    let chrf_identical = compute_chrf(candidate, reference, 6);

    assert!(bleu_identical > 99.0, "Identical strings must have ~100 BLEU, got {bleu_identical}");
    assert!(chrf_identical > 99.0, "Identical strings must have ~100 ChrF, got {chrf_identical}");

    let candidate_synonym = "A quick brown fox leaps above a lazy hound";
    let bleu_partial = compute_sentence_bleu(candidate_synonym, reference);
    let chrf_partial = compute_chrf(candidate_synonym, reference, 6);

    assert!(bleu_partial > 0.0, "Synonym string should have partial BLEU, got {bleu_partial}");
    assert!(chrf_partial > 40.0, "Synonym string should have solid ChrF, got {chrf_partial}");
}

#[tokio::test]
async fn test_accuracy_ast_code_masking_preservation() {
    let input = "Deploy the service by visiting `https://code.brandonhubbard.com/translate-rs/` or running `cargo run --release`. Contact dev@example.com for assistance.";
    let segments = TranslationMasker::segments(input, false);

    let untranslatable: Vec<String> = segments
        .iter()
        .filter(|s| !s.is_translatable)
        .map(|s| s.text.clone())
        .collect();

    assert!(
        untranslatable.iter().any(|t| t == "`https://code.brandonhubbard.com/translate-rs/`"),
        "URL inside backticks must be preserved as non-translatable"
    );
    assert!(
        untranslatable.iter().any(|t| t == "`cargo run --release`"),
        "CLI command inside backticks must be preserved as non-translatable"
    );
    assert!(
        untranslatable.iter().any(|t| t == "dev@example.com"),
        "Email address must be preserved as non-translatable"
    );

    // Verify round-trip reassembly equals exact original string
    let reassembled: String = segments.into_iter().map(|s| s.text).collect();
    assert_eq!(reassembled, input);
}

#[tokio::test]
async fn test_accuracy_fenced_markdown_blocks_preserved() {
    let markdown = "Here is the installation procedure:\n\n```bash\ncargo install translate-rs\ntranslate --serve --port 8080\n```\n\nEnjoy using the tool!";
    let segments = TranslationMasker::segments(markdown, true);

    let code_block = segments.iter().find(|s| s.text.contains("cargo install translate-rs"));
    assert!(code_block.is_some(), "Fenced code block should be extracted");
    assert!(!code_block.unwrap().is_translatable, "Fenced code block must never be translatable");

    let reassembled: String = segments.into_iter().map(|s| s.text).collect();
    assert_eq!(reassembled, markdown, "Masking segments must perfectly reconstruct source");
}

#[tokio::test]
async fn test_accuracy_translation_output_quality() {
    let translator = AppleTranslator::new();
    let status = translator.pair_status("en", "es").await;

    if status != translate_rs::types::PairStatus::Installed {
        eprintln!("Skipping live translation test: en-es not installed on host");
        return;
    }

    let input = vec!["Hello world".to_string()];
    let res = translator.translate(&input, "en", "es", false).await;
    assert!(res.is_ok(), "Translation must succeed");

    let translated = res.unwrap();
    assert_eq!(translated.len(), 1);
    let output = translated[0].to_lowercase();
    assert!(
        output.contains("hola") && output.contains("mundo"),
        "Expected 'Hola mundo' in output, got: {output}"
    );

    // Test BLEU against gold reference
    let bleu = compute_sentence_bleu(&translated[0], "Hola mundo");
    assert!(bleu > 80.0, "BLEU score for 'Hello world' -> 'Hola mundo' must exceed 80, got {bleu}");
}
