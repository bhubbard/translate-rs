# translate-rs

[![macOS 26+](https://img.shields.io/badge/macOS-26+-blue.svg)](https://www.apple.com/macos/)
[![Rust 2021](https://img.shields.io/badge/rust-edition%202021-orange.svg)](Cargo.toml)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![On-device](https://img.shields.io/badge/translation-on--device-purple.svg)](#why-on-device)

**A high-performance, deterministic, on-device translator for macOS ported to native Rust.**  
A UNIX-style pipe-friendly filter CLI (`translate` / `ueb`) and drop-in HTTP server for **DeepL**, **LibreTranslate**, and **Google Cloud Translation v2** — running 100% on-device using Apple's Translation and NaturalLanguage frameworks.

Ported directly from [Arthur-Ficial/translate](https://github.com/Arthur-Ficial/translate).

---

## ⚡ What it is

| Mode | Command | Purpose |
| --- | --- | --- |
| **UNIX Tool** | `translate --to en` | Pipe-friendly filter, exit codes, plain/JSON/NDJSON output |
| **Short Alias** | `ueb --to de` | Fast drop-in alias |
| **HTTP Server** | `translate --serve` | Drop-in for DeepL, LibreTranslate, and Google v2 clients |

---

## 🚀 Key Features

- **100% On-Device & Deterministic**: Powered by Apple Silicon neural translation models on macOS Tahoe (macOS 26+). No external cloud calls, no API keys, no telemetry, no LLM hallucinations.
- **Smart Markdown & Code Masking**: Automatically shields inline backticks, multi-line fenced code blocks, email addresses, and URLs from translation corruption.
- **Multi-Dialect HTTP API Gateway**:
  - **DeepL API**: `POST /v2/translate`, `GET /v2/languages`
  - **LibreTranslate API**: `POST /translate`, `POST /detect`, `GET /languages`, `GET /frontend/settings`
  - **Google Cloud Translation v2**: `POST /language/translate/v2`, `GET /language/translate/v2`, detection, and language catalogs
- **Dual Language Detection**: Prioritizes Apple's `NLLanguageRecognizer` with `whatlang` n-gram fallback across 38+ supported languages.
- **Pipelining & Streaming**: Supports streaming stdin/stdout with paragraph and line-by-line (`--batch`) chunking.

---

## 🛠️ CLI Usage Examples

```bash
# Basic translation
translate --to es "Good morning, how are you?"

# Detect language only
translate --detect-only "Bonjour tout le monde"

# Structured output
translate --to de --format json "Ship fast and break nothing"
translate --to ja --format ndjson < input.txt

# Preserve newlines in multi-line documents
translate --to fr --preserve-newlines < document.md

# Start HTTP server
translate --serve --port 8080
```

---

## 🧪 Testing

```bash
cargo test --all-targets
```

---

## 📄 License

MIT License. See [LICENSE](LICENSE) for details.
