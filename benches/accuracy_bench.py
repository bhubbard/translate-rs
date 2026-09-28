#!/usr/bin/env python3
"""
Comprehensive Accuracy & BLEU Benchmark Runner for translate-rs (Rust) vs. translate (Swift, franzai.com)
Evaluates BLEU-4, ChrF, Parity Cross-Agreement, and Code/URL Masking Preservation against Human Reference Corpora.
"""

import subprocess
import math
import re
import sys
from collections import Counter

# ---------------------------------------------------------------------------
# Metric Calculations: BLEU-4 and ChrF (Zero external dependencies)
# ---------------------------------------------------------------------------

def tokenize(text, lang="en"):
    """Simple tokenizer for European languages; character tokenizer for CJK."""
    if lang in ("ja", "zh"):
        return [c for c in text if not c.isspace()]
    # Basic word tokenization preserving words and punctuation
    text = re.sub(r'([.,!?;:()\[\]"\'\/])', r' \1 ', text)
    return text.strip().split()

def get_ngrams(tokens, n):
    return [tuple(tokens[i:i+n]) for i in range(len(tokens) - n + 1)]

def compute_bleu(hypotheses, references, max_n=4, lang="en"):
    """Standard sentence/corpus BLEU-4 with brevity penalty."""
    weights = [0.25] * max_n
    p_n = [0.0] * max_n
    
    total_hyp_len = 0
    total_ref_len = 0
    
    clipped_counts = [0] * max_n
    total_counts = [0] * max_n

    for hyp, ref in zip(hypotheses, references):
        hyp_tokens = tokenize(hyp, lang)
        ref_tokens = tokenize(ref, lang)
        
        total_hyp_len += len(hyp_tokens)
        total_ref_len += len(ref_tokens)
        
        for n in range(1, max_n + 1):
            hyp_ngrams = Counter(get_ngrams(hyp_tokens, n))
            ref_ngrams = Counter(get_ngrams(ref_tokens, n))
            
            clipped = sum(min(count, ref_ngrams[ngram]) for ngram, count in hyp_ngrams.items())
            total = max(1, sum(hyp_ngrams.values()))
            
            clipped_counts[n-1] += clipped
            total_counts[n-1] += total

    for n in range(max_n):
        if total_counts[n] == 0 or clipped_counts[n] == 0:
            p_n[n] = 1e-9
        else:
            p_n[n] = clipped_counts[n] / total_counts[n]

    # Brevity penalty
    if total_hyp_len == 0:
        return 0.0
    if total_hyp_len > total_ref_len:
        bp = 1.0
    else:
        bp = math.exp(1.0 - (total_ref_len / total_hyp_len))

    log_p_sum = sum(w * math.log(p) for w, p in zip(weights, p_n))
    bleu = bp * math.exp(log_p_sum) * 100.0
    return max(0.0, min(100.0, bleu))

def compute_chrf(hypotheses, references, n=6, beta=2.0):
    """Character n-gram F-score (ChrF) with standard beta=2."""
    total_precision_ngrams = [0] * n
    total_recall_ngrams = [0] * n
    total_hyp_ngrams = [0] * n
    total_ref_ngrams = [0] * n

    for hyp, ref in zip(hypotheses, references):
        hyp_clean = "".join(hyp.split())
        ref_clean = "".join(ref.split())
        
        for i in range(1, n + 1):
            hyp_char_ngrams = Counter([hyp_clean[j:j+i] for j in range(len(hyp_clean) - i + 1)])
            ref_char_ngrams = Counter([ref_clean[j:j+i] for j in range(len(ref_clean) - i + 1)])
            
            overlap = sum(min(count, ref_char_ngrams[ngram]) for ngram, count in hyp_char_ngrams.items())
            total_precision_ngrams[i-1] += overlap
            total_recall_ngrams[i-1] += overlap
            total_hyp_ngrams[i-1] += sum(hyp_char_ngrams.values())
            total_ref_ngrams[i-1] += sum(ref_char_ngrams.values())

    precisions = [
        total_precision_ngrams[i] / total_hyp_ngrams[i] if total_hyp_ngrams[i] > 0 else 0
        for i in range(n)
    ]
    recalls = [
        total_recall_ngrams[i] / total_ref_ngrams[i] if total_ref_ngrams[i] > 0 else 0
        for i in range(n)
    ]

    avg_p = sum(precisions) / n if n > 0 else 0
    avg_r = sum(recalls) / n if n > 0 else 0

    if avg_p + avg_r == 0:
        return 0.0
    
    b2 = beta ** 2
    chrf = (1 + b2) * (avg_p * avg_r) / ((b2 * avg_p) + avg_r) * 100.0
    return max(0.0, min(100.0, chrf))

def jaccard_similarity(str1, str2):
    t1 = set(tokenize(str1))
    t2 = set(tokenize(str2))
    if not t1 and not t2:
        return 1.0
    return len(t1 & t2) / len(t1 | t2)

# ---------------------------------------------------------------------------
# Test Corpora with Human Reference Gold Standard
# ---------------------------------------------------------------------------

DATASETS = {
    "es": {
        "name": "English -> Spanish (ES)",
        "items": [
            {
                "src": "Artificial intelligence and neural machine translation have revolutionized cross-border communication on local devices.",
                "ref": "La inteligencia artificial y la traducción automática neuronal han revolucionado la comunicación transfronteriza en dispositivos locales."
            },
            {
                "src": "Please save all changes to the configuration file before restarting the background service.",
                "ref": "Por favor guarde todos los cambios en el archivo de configuración antes de reiniciar el servicio en segundo plano."
            },
            {
                "src": "The rapid advancement of systems programming in Rust empowers developers to build safe, concurrent, and blazing-fast tools without compromising on reliability.",
                "ref": "El rápido avance de la programación de sistemas en Rust permite a los desarrolladores crear herramientas seguras, concurrentes y ultrarrápidas sin comprometer la confiabilidad."
            },
            {
                "src": "According to the terms of the agreement, all confidential data must be encrypted both in transit and at rest.",
                "ref": "De acuerdo con los términos del acuerdo, todos los datos confidenciales deben cifrarse tanto en tránsito como en reposo."
            },
            {
                "src": "Where is the nearest train station, and what time does the next departure leave for Barcelona?",
                "ref": "¿Dónde está la estación de tren más cercana y a qué hora sale el próximo viaje hacia Barcelona?"
            },
            {
                "src": "Zero network latency and 100% offline privacy ensure that personal communications never leak to remote cloud servers.",
                "ref": "La latencia de red cero y la privacidad 100% fuera de línea garantizan que las comunicaciones personales nunca se filtren a servidores en la nube remotos."
            },
            {
                "src": "The compiler detected a lifetime mismatch error at line 42 of the source file.",
                "ref": "El compilador detectó un error de discrepancia de tiempo de vida en la línea 42 del archivo fuente."
            },
            {
                "src": "High memory consumption can trigger kernel out-of-memory errors on constrained servers.",
                "ref": "El alto consumo de memoria puede provocar errores de falta de memoria en servidores con recursos limitados."
            }
        ]
    },
    "de": {
        "name": "English -> German (DE)",
        "items": [
            {
                "src": "Artificial intelligence and neural machine translation have revolutionized cross-border communication on local devices.",
                "ref": "Künstliche Intelligenz und neuronale maschinelle Übersetzung haben die grenzüberschreitende Kommunikation auf lokalen Geräten revolutioniert."
            },
            {
                "src": "Please save all changes to the configuration file before restarting the background service.",
                "ref": "Bitte speichern Sie alle Änderungen in der Konfigurationsdatei, bevor Sie den Hintergrunddienst neu starten."
            },
            {
                "src": "The rapid advancement of systems programming in Rust empowers developers to build safe, concurrent, and blazing-fast tools without compromising on reliability.",
                "ref": "Die rasanten Fortschritte der Systemprogrammierung in Rust ermöglichen Entwicklern, sichere, nebenläufige und blitzschnelle Werkzeuge zu erstellen, ohne Kompromisse bei der Zuverlässigkeit einzugehen."
            },
            {
                "src": "According to the terms of the agreement, all confidential data must be encrypted both in transit and at rest.",
                "ref": "Gemäß den Vertragsbedingungen müssen alle vertraulichen Daten sowohl bei der Übertragung als auch im Ruhezustand verschlüsselt werden."
            },
            {
                "src": "Where is the nearest train station, and what time does the next departure leave for Berlin?",
                "ref": "Wo ist der nächste Bahnhof und wann fährt der nächste Zug nach Berlin ab?"
            },
            {
                "src": "Zero network latency and 100% offline privacy ensure that personal communications never leak to remote cloud servers.",
                "ref": "Keine Netzwerklatenz und 100 % Offline-Datenschutz stellen sicher, dass persönliche Nachrichten niemals an entfernte Cloud-Server gelangen."
            }
        ]
    },
    "fr": {
        "name": "English -> French (FR)",
        "items": [
            {
                "src": "Artificial intelligence and neural machine translation have revolutionized cross-border communication on local devices.",
                "ref": "L'intelligence artificielle et la traduction automatique neuronale ont révolutionné la communication transfrontalière sur les appareils locaux."
            },
            {
                "src": "Please save all changes to the configuration file before restarting the background service.",
                "ref": "Veuillez enregistrer toutes les modifications apportées au fichier de configuration avant de redémarrer le service en arrière-plan."
            },
            {
                "src": "The rapid advancement of systems programming in Rust empowers developers to build safe, concurrent, and blazing-fast tools without compromising on reliability.",
                "ref": "Les progrès rapides de la programmation système en Rust permettent aux développeurs de concevoir des outils sûrs, concurrents et ultra-rapides sans compromettre la fiabilité."
            },
            {
                "src": "According to the terms of the agreement, all confidential data must be encrypted both in transit and at rest.",
                "ref": "Conformément aux termes de l'accord, toutes les données confidentielles doivent être chiffrées en transit et au repos."
            },
            {
                "src": "Where is the nearest train station, and what time does the next departure leave for Paris?",
                "ref": "Où se trouve la gare la plus proche et à quelle heure part le prochain train pour Paris ?"
            },
            {
                "src": "Zero network latency and 100% offline privacy ensure that personal communications never leak to remote cloud servers.",
                "ref": "Une latence réseau nulle et une confidentialité 100 % hors ligne garantissent que les communications personnelles ne fuient jamais vers des serveurs distants."
            }
        ]
    },
    "ja": {
        "name": "English -> Japanese (JA)",
        "items": [
            {
                "src": "Artificial intelligence and neural machine translation have revolutionized cross-border communication on local devices.",
                "ref": "人工知能とニューラル機械翻訳はローカルデバイス上での国境を越えたコミュニケーションに革命をもたらしました。"
            },
            {
                "src": "Please save all changes to the configuration file before restarting the background service.",
                "ref": "バックグラウンドサービスを再起動する前に、構成ファイルへのすべての変更を保存してください。"
            },
            {
                "src": "The rapid advancement of systems programming in Rust empowers developers to build safe, concurrent, and blazing-fast tools without compromising on reliability.",
                "ref": "Rustにおけるシステムプログラミングの急速な進歩により、開発者は信頼性を損なうことなく、安全で並行性が高く超高速なツールを構築できます。"
            },
            {
                "src": "Where is the nearest train station, and what time does the next departure leave for Tokyo?",
                "ref": "一番近い駅はどこですか？東京行きの次の出発は何時ですか？"
            },
            {
                "src": "Zero network latency and 100% offline privacy ensure that personal communications never leak to remote cloud servers.",
                "ref": "ネットワーク遅延ゼロと100%のオフラインプライバシーにより、個人の通信がリモートクラウドサーバーに漏洩することはありません。"
            }
        ]
    },
    "masking": {
        "name": "Code, Markdown & URL Preservation (EN -> ES)",
        "lang": "es",
        "items": [
            {
                "src": "Visit `https://code.brandonhubbard.com/translate-rs/` to download the binary or run `cargo install translate-rs`.",
                "required_tokens": ["`https://code.brandonhubbard.com/translate-rs/`", "`cargo install translate-rs`"]
            },
            {
                "src": "Contact support via dev@example.com and check `--format json` flag.",
                "required_tokens": ["dev@example.com", "`--format json`"]
            },
            {
                "src": "Use `git clone https://github.com/bhubbard/translate-rs.git` to build from source.",
                "required_tokens": ["`git clone https://github.com/bhubbard/translate-rs.git`"]
            }
        ]
    }
}

# ---------------------------------------------------------------------------
# Execution Harness
# ---------------------------------------------------------------------------

RUST_BIN = "./target/release/translate"
SWIFT_BIN = "/opt/homebrew/bin/translate"

def run_translation(bin_path, text, to_lang, from_lang="en"):
    """Invokes CLI translator and captures stdout."""
    try:
        cmd = [bin_path, text, "--from", from_lang, "--to", to_lang, "--quiet"]
        res = subprocess.run(cmd, capture_output=True, text=True, check=True, timeout=15)
        return res.stdout.strip()
    except Exception as e:
        return f"[ERROR: {e}]"

def main():
    print("=" * 80)
    print("  TRANSLATION ACCURACY & BLEU BENCHMARK")
    print("  Comparing: translate-rs (Rust) vs. translate (Swift / translate.franzai.com)")
    print("=" * 80)
    print()

    summary_rows = []

    for lang_key, dataset in DATASETS.items():
        if lang_key == "masking":
            continue

        target_lang = lang_key
        ds_name = dataset["name"]
        items = dataset["items"]
        
        print(f"\n>>> Running Evaluation: {ds_name} ({len(items)} test sentences)")

        rust_hyps = []
        swift_hyps = []
        refs = []
        parity_matches = 0
        jaccard_scores = []

        for idx, item in enumerate(items, 1):
            src = item["src"]
            ref = item["ref"]
            
            r_out = run_translation(RUST_BIN, src, target_lang)
            s_out = run_translation(SWIFT_BIN, src, target_lang)

            rust_hyps.append(r_out)
            swift_hyps.append(s_out)
            refs.append(ref)

            # Check cross-parity
            is_exact = (r_out.strip() == s_out.strip())
            if is_exact:
                parity_matches += 1
            jacc = jaccard_similarity(r_out, s_out)
            jaccard_scores.append(jacc)

            print(f"  [{idx}/{len(items)}]")
            print(f"    Source: \"{src[:70]}...\"")
            print(f"    Rust:   \"{r_out}\"")
            print(f"    Swift:  \"{s_out}\"")
            print(f"    Ref:    \"{ref}\"")
            print(f"    Parity: {'IDENTICAL' if is_exact else f'Jaccard {jacc:.2f}'}")

        rust_bleu = compute_bleu(rust_hyps, refs, lang=target_lang)
        swift_bleu = compute_bleu(swift_hyps, refs, lang=target_lang)
        rust_chrf = compute_chrf(rust_hyps, refs)
        swift_chrf = compute_chrf(swift_hyps, refs)
        exact_parity_pct = (parity_matches / len(items)) * 100.0
        avg_jaccard = sum(jaccard_scores) / len(jaccard_scores) * 100.0

        summary_rows.append({
            "lang": ds_name,
            "target": target_lang,
            "count": len(items),
            "rust_bleu": rust_bleu,
            "swift_bleu": swift_bleu,
            "rust_chrf": rust_chrf,
            "swift_chrf": swift_chrf,
            "exact_parity": exact_parity_pct,
            "semantic_parity": avg_jaccard
        })

    # Test AST Content Masking
    print(f"\n>>> Running Evaluation: AST Code & URL Masking Preservation")
    masking_items = DATASETS["masking"]["items"]
    rust_preserved = 0
    swift_preserved = 0
    total_tokens = 0

    for idx, item in enumerate(masking_items, 1):
        src = item["src"]
        tokens = item["required_tokens"]
        total_tokens += len(tokens)

        r_out = run_translation(RUST_BIN, src, "es")
        s_out = run_translation(SWIFT_BIN, src, "es")

        r_ok = sum(1 for tok in tokens if tok in r_out)
        s_ok = sum(1 for tok in tokens if tok in s_out)
        rust_preserved += r_ok
        swift_preserved += s_ok

        print(f"  [{idx}/{len(masking_items)}]")
        print(f"    Source: \"{src}\"")
        print(f"    Rust:   \"{r_out}\" (Preserved: {r_ok}/{len(tokens)})")
        print(f"    Swift:  \"{s_out}\" (Preserved: {s_ok}/{len(tokens)})")

    rust_mask_pct = (rust_preserved / total_tokens) * 100.0
    swift_mask_pct = (swift_preserved / total_tokens) * 100.0

    print("\n" + "=" * 80)
    print("  ACCURACY BENCHMARK SUMMARY REPORT")
    print("=" * 80)
    print(f"{'Language Pair':<28} | {'Rust BLEU':<10} | {'Swift BLEU':<10} | {'Rust ChrF':<10} | {'Swift ChrF':<10} | {'Parity %':<10}")
    print("-" * 88)
    for row in summary_rows:
        print(f"{row['lang']:<28} | {row['rust_bleu']:<10.2f} | {row['swift_bleu']:<10.2f} | {row['rust_chrf']:<10.2f} | {row['swift_chrf']:<10.2f} | {row['semantic_parity']:<9.1f}%")
    
    print("-" * 88)
    print(f"AST Code & URL Preservation: Rust = {rust_mask_pct:.1f}% | Swift = {swift_mask_pct:.1f}%")
    print("=" * 80)

if __name__ == "__main__":
    main()
