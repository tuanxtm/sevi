#if DEBUG
import AppKit

/// Debug only: drives the emoji picker off screen with real key events and saves what it draws, so the
/// flows can be checked without the screen (`-emojiSelfTest <dir>`; never shown, never takes focus).
enum EmojiSelfTest {
    @MainActor static func run(dir: String) {
        try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
        if UserDefaults.standard.bool(forKey: "emojiSelfTestDark") { NSApp.appearance = NSAppearance(named: .darkAqua) }
        let model = EmojiPicker.shared.model
        model.reset(vietnamese: true)
        let panel = EmojiPanel(model: model)   // built like the real one, never ordered front
        var log = ""
        func settle() { RunLoop.main.run(until: Date().addingTimeInterval(0.35)) }
        func key(_ code: UInt16, _ chars: String = "", _ mods: NSEvent.ModifierFlags = []) {
            let e = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: mods, timestamp: 0, windowNumber: 0, context: nil,
                                     characters: chars, charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code)!
            panel.keyDown(with: e)
        }
        func type(_ s: String) { for c in s { key(0, String(c)) } }
        var n = 0
        func snap(_ what: String) {
            settle()
            n += 1
            let v = panel.contentView!
            v.layoutSubtreeIfNeeded()
            if let rep = v.bitmapImageRepForCachingDisplay(in: v.bounds) {
                v.cacheDisplay(in: v.bounds, to: rep)
                try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: "\(dir)/\(String(format: "%02d", n)).png"))
            }
            let first = model.visible.prefix(12).map(\.char).joined()
            log += String(format: "%02d", n) + " \(what): query '\(model.raw)' shows '\(model.display)' selected \(model.selected) = \(model.current?.char ?? "-") \(model.current?.nameEN ?? "") | first: \(first)\n"
        }
        snap("open")
        type("laugh"); snap("type laugh")
        key(124); snap("→")
        key(124); snap("→")
        key(125); snap("↓")
        key(123); snap("←")
        key(126); snap("↑")
        key(0, "a", .command); snap("⌘A")
        type("tim"); snap("type tim (replaces)")
        key(51, "", .command); snap("⌘⌫ clears")
        for _ in 0..<5 { key(125) }; snap("↓×5 in browse")
        for _ in 0..<12 { key(125) }; snap("↓×12 more")
        for _ in 0..<17 { key(126) }; snap("↑×17 back")
        for _ in 0..<15 { key(125) }; snap("↓×15 then close")
        model.reset(vietnamese: true); snap("reopen: top of the list")
        type("nguwaj"); snap("type nguwaj")
        type(" dua"); snap("type ' dua'")
        key(53); snap("esc clears the search")
        try? log.write(toFile: "\(dir)/log.txt", atomically: true, encoding: .utf8)
        NSApp.terminate(nil)
    }
}
#endif
