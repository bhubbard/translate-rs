use crate::detect::LanguageDetector;
use crate::engine::{AppleTranslator, Translating};
use crate::error::TranslateError;
use crate::output::{render_records, OutputWriter};
use crate::server::{create_router, ServerState};
use crate::stream::StreamProcessor;
use crate::types::{
    is_language_supported, normalize_language_code, parse_language_hints, OutputFormat,
    TranslationRecord, SUPPORTED_LANGUAGES,
};
use clap::Parser;
use std::fs::File;
use std::io::{self, IsTerminal, Read};
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(
    name = "translate",
    about = "Deterministic on-device translation CLI and multi-provider HTTP server in Rust",
    version
)]
pub struct Cli {
    /// Target language code (e.g. en, es, de, fr, ja, zh)
    #[arg(short = 't', long = "to")]
    pub to: Option<String>,

    /// Source language code (auto-detected if omitted)
    #[arg(short = 'f', long = "from")]
    pub from: Option<String>,

    /// Detect language of input and exit
    #[arg(short = 'd', long = "detect-only")]
    pub detect_only: bool,

    /// Output format (plain, json, ndjson)
    #[arg(long = "format", default_value = "plain")]
    pub format: OutputFormat,

    /// Preserve newline characters within translated segments
    #[arg(long = "preserve-newlines")]
    pub preserve_newlines: bool,

    /// Process input line-by-line in batches rather than by paragraphs
    #[arg(long = "batch")]
    pub batch: bool,

    /// Files to translate
    #[arg(long = "file")]
    pub files: Vec<String>,

    /// Do not download or install missing models
    #[arg(long = "no-install")]
    pub no_install: bool,

    /// Comma-separated hints for language detection (e.g. "en,es,fr")
    #[arg(long = "langs")]
    pub langs: Option<String>,

    /// Suppress progress output on stderr
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Run as an HTTP server compatible with DeepL, LibreTranslate, and Google v2
    #[arg(short = 's', long = "serve")]
    pub serve: bool,

    /// HTTP server port
    #[arg(long = "port", default_value_t = 8080)]
    pub port: u16,

    /// HTTP server host
    #[arg(long = "host", default_value = "127.0.0.1")]
    pub host: String,

    /// Optional API key requirement for HTTP server
    #[arg(long = "api-key")]
    pub api_key: Option<String>,

    /// Download and install model for a language pair (e.g. "en-es")
    #[arg(long = "install")]
    pub install: Option<String>,

    /// List installed language pairs
    #[arg(long = "installed")]
    pub installed: bool,

    /// List all available language pairs supported by Apple Translation
    #[arg(long = "available")]
    pub available: bool,

    /// Positional text to translate
    pub text: Vec<String>,
}

pub async fn run_cli(cli: Cli) -> Result<(), TranslateError> {
    let translator = Arc::new(AppleTranslator::new());
    let hints = parse_language_hints(cli.langs.as_deref());

    // 1. Handle HTTP Server mode
    if cli.serve {
        let state = Arc::new(ServerState {
            translator: translator.clone(),
            api_key: cli.api_key,
        });
        let app = create_router(state);
        let addr: SocketAddr = format!("{}:{}", cli.host, cli.port)
            .parse()
            .map_err(|e: std::net::AddrParseError| TranslateError::Usage(e.to_string()))?;

        if !cli.quiet {
            eprintln!("translate: serving on http://{addr}");
            eprintln!("  DeepL API:          http://{addr}/v2/translate");
            eprintln!("  LibreTranslate:     http://{addr}/translate");
            eprintln!("  Google Translate:   http://{addr}/language/translate/v2");
        }

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await.map_err(|e| TranslateError::Server(e.to_string()))?;
        return Ok(());
    }

    // 2. Handle --installed
    if cli.installed {
        let pairs = translator.installed_pairs().await;
        for p in pairs {
            println!("{p}");
        }
        return Ok(());
    }

    // 3. Handle --available
    if cli.available {
        for (src, _) in SUPPORTED_LANGUAGES {
            for (dst, _) in SUPPORTED_LANGUAGES {
                if src != dst {
                    println!("{src}-{dst}");
                }
            }
        }
        return Ok(());
    }

    // 4. Handle --install <pair>
    if let Some(ref pair_spec) = cli.install {
        let parts: Vec<&str> = pair_spec.split('-').collect();
        if parts.len() != 2 {
            return Err(TranslateError::Usage("--install expects a pair like de-en".into()));
        }
        let src = normalize_language_code(parts[0]);
        let dst = normalize_language_code(parts[1]);
        translator.prepare(&src, &dst, false, cli.quiet).await?;
        if !cli.quiet {
            eprintln!("translate: successfully installed {src}-{dst}");
        }
        return Ok(());
    }

    // 5. Handle --detect-only
    if cli.detect_only {
        let detector = LanguageDetector::new(&hints);

        if !cli.text.is_empty() {
            let sample = cli.text.join("\n");
            let result = detector.detect(&sample)?;
            println!("{}\t{}", result.language_code, crate::output::format_number(result.confidence));
            return Ok(());
        }

        if !cli.files.is_empty() {
            for file_path in &cli.files {
                let mut f = File::open(file_path)?;
                let mut buf = vec![0u8; 4096];
                let n = f.read(&mut buf)?;
                let sample = String::from_utf8_lossy(&buf[..n]);
                let result = detector.detect(&sample)?;
                println!("{}\t{}", result.language_code, crate::output::format_number(result.confidence));
            }
            return Ok(());
        }

        if io::stdin().is_terminal() {
            return Err(TranslateError::Usage(
                "--detect-only needs text arguments, --file, or piped stdin".into(),
            ));
        }

        let mut buf = vec![0u8; 4096];
        let n = io::stdin().read(&mut buf)?;
        if n == 0 {
            return Err(TranslateError::Input("empty stdin; nothing to detect".into()));
        }
        let sample = String::from_utf8_lossy(&buf[..n]);
        let result = detector.detect(&sample)?;
        println!("{}\t{}", result.language_code, crate::output::format_number(result.confidence));
        return Ok(());
    }

    // 6. Regular translation requires --to
    let Some(ref target_code) = cli.to else {
        return Err(TranslateError::Usage(
            "--to is required unless --detect-only, --install, --installed, or --available is used".into(),
        ));
    };

    let dst_code = normalize_language_code(target_code);
    if !is_language_supported(&dst_code) {
        return Err(TranslateError::UnsupportedPair(format!("unsupported target language '{target_code}'")));
    }

    // 7. Positional arguments
    if !cli.text.is_empty() {
        let sample = cli.text.join("\n");
        let detection = if let Some(ref s) = cli.from {
            crate::types::DetectionResult {
                language_code: normalize_language_code(s),
                confidence: 1.0,
            }
        } else {
            LanguageDetector::new(&hints).detect(&sample)?
        };

        let src_code = normalize_language_code(&detection.language_code);
        translator
            .prepare(&src_code, &dst_code, cli.no_install, cli.quiet)
            .await?;

        let translated = translator
            .translate(&cli.text, &src_code, &dst_code, cli.preserve_newlines)
            .await?;

        let records: Vec<TranslationRecord> = cli
            .text
            .iter()
            .zip(translated.iter())
            .map(|(src, dst)| TranslationRecord {
                from: src_code.clone(),
                to: dst_code.clone(),
                src: src.clone(),
                dst: dst.clone(),
                conf: detection.confidence,
            })
            .collect();

        render_records(&records, cli.format, true)?;
        return Ok(());
    }

    // 8. File input or Piped Stdin streaming
    let mut writer = OutputWriter::new(cli.format);
    let processor = StreamProcessor::new();

    if !cli.files.is_empty() {
        for file_path in &cli.files {
            let f = File::open(file_path)?;
            processor
                .process(
                    f,
                    cli.from.as_deref(),
                    &dst_code,
                    &hints,
                    translator.as_ref(),
                    &mut writer,
                    cli.no_install,
                    cli.quiet,
                    cli.preserve_newlines,
                    cli.batch,
                )
                .await?;
        }
        writer.finish()?;
        return Ok(());
    }

    if io::stdin().is_terminal() {
        return Err(TranslateError::Usage(
            "no input: pass text arguments, use --file, or pipe UTF-8 text on stdin".into(),
        ));
    }

    processor
        .process(
            io::stdin(),
            cli.from.as_deref(),
            &dst_code,
            &hints,
            translator.as_ref(),
            &mut writer,
            cli.no_install,
            cli.quiet,
            cli.preserve_newlines,
            cli.batch,
        )
        .await?;
    writer.finish()?;

    Ok(())
}
