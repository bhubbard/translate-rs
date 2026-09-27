import Foundation
import Translation
import NaturalLanguage

@available(macOS 15.0, *)
@main
struct AppleTranslateBridgeMain {
    static func main() async {
        let args = ProcessInfo.processInfo.arguments
        guard args.count >= 2 else {
            fputs("Usage: apple_translate_bridge <command> [args...]\n", stderr)
            exit(2)
        }

        let cmd = args[1]
        switch cmd {
        case "status":
            guard args.count >= 4 else { exit(2) }
            let src = Locale.Language(identifier: args[2])
            let dst = Locale.Language(identifier: args[3])
            let avail = LanguageAvailability()
            let status = await avail.status(from: src, to: dst)
            switch status {
            case .installed: print("installed")
            case .supported: print("supported")
            default: print("unsupported")
            }

        case "languages":
            let avail = LanguageAvailability()
            let langs = await avail.supportedLanguages
            var seen = Set<String>()
            for l in langs {
                let id = l.minimalIdentifier
                if !seen.contains(id) {
                    seen.insert(id)
                    print(id)
                }
            }

        case "detect":
            guard args.count >= 3 else { exit(2) }
            let text = args[2]
            let recognizer = NLLanguageRecognizer()
            recognizer.processString(text)
            if let dominant = recognizer.dominantLanguage {
                let hypotheses = recognizer.languageHypotheses(withMaximum: 1)
                let conf = hypotheses[dominant] ?? 1.0
                print("\(dominant.rawValue)\t\(conf)")
            } else {
                print("en\t0.5")
            }

        case "prepare":
            guard args.count >= 4 else { exit(2) }
            let src = Locale.Language(identifier: args[2])
            let dst = Locale.Language(identifier: args[3])
            let session = TranslationSession(installedSource: src, target: dst)
            do {
                try await session.prepareTranslation()
                print("ok")
            } catch {
                fputs("error: \(error.localizedDescription)\n", stderr)
                exit(1)
            }

        case "translate":
            guard args.count >= 5 else { exit(2) }
            let src = Locale.Language(identifier: args[2])
            let dst = Locale.Language(identifier: args[3])
            let text = args[4]
            let session = TranslationSession(installedSource: src, target: dst)
            do {
                try await session.prepareTranslation()
                let response = try await session.translate(text)
                print(response.targetText)
            } catch {
                fputs("error: \(error.localizedDescription)\n", stderr)
                exit(1)
            }

        case "translate-batch":
            guard args.count >= 4 else { exit(2) }
            let src = Locale.Language(identifier: args[2])
            let dst = Locale.Language(identifier: args[3])
            let inputData = FileHandle.standardInput.readDataToEndOfFile()
            guard let texts = try? JSONDecoder().decode([String].self, from: inputData) else {
                fputs("error: invalid JSON array of strings on stdin\n", stderr)
                exit(2)
            }

            if texts.isEmpty {
                print("[]")
                return
            }

            let session = TranslationSession(installedSource: src, target: dst)
            do {
                try await session.prepareTranslation()
                if texts.count == 1 {
                    let response = try await session.translate(texts[0])
                    let encoded = try JSONEncoder().encode([response.targetText])
                    FileHandle.standardOutput.write(encoded)
                    print("")
                } else {
                    let requests = texts.enumerated().map { idx, t in
                        TranslationSession.Request(sourceText: t, clientIdentifier: String(idx))
                    }
                    let responses = try await session.translations(from: requests)
                    let results = responses.map(\.targetText)
                    let encoded = try JSONEncoder().encode(results)
                    FileHandle.standardOutput.write(encoded)
                    print("")
                }
            } catch {
                fputs("error: \(error.localizedDescription)\n", stderr)
                exit(1)
            }

        default:
            fputs("Unknown command: \(cmd)\n", stderr)
            exit(2)
        }
    }
}
