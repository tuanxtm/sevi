import AppKit
// Reads words on stdin, prints those the macOS English spell checker accepts.
let checker = NSSpellChecker.shared
checker.automaticallyIdentifiesLanguages = false
_ = checker.setLanguage("en")
while let line = readLine() {
    let w = line.trimmingCharacters(in: .whitespaces)
    if w.isEmpty { continue }
    // accept the word as typed or capitalized (names: "mary" -> "Mary")
    for form in [w, w.prefix(1).uppercased() + w.dropFirst()] {
        let r = checker.checkSpelling(of: form, startingAt: 0, language: "en", wrap: false, inSpellDocumentWithTag: 0, wordCount: nil)
        if r.location == NSNotFound { print(w); break }
    }
}
