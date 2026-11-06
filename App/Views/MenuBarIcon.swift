import AppKit

/// Menu bar icon: a "V" (Vietnamese) or "E" (English) in a rounded square, drawn as a template image.
enum MenuBarIcon {
    private static var cache: [Bool: NSImage] = [:]

    static func image(vietnamese: Bool) -> NSImage {
        if let cached = cache[vietnamese] { return cached }
        let size = NSSize(width: 18, height: 16)
        let image = NSImage(size: size, flipped: false) { rect in
            let box = rect.insetBy(dx: 1, dy: 0.5)
            let path = NSBezierPath(roundedRect: box, xRadius: 3.5, yRadius: 3.5)
            NSColor.black.setFill()
            path.fill()
            let letter = vietnamese ? "V" : "E"
            let attrs: [NSAttributedString.Key: Any] = [
                .font: NSFont.systemFont(ofSize: 11.5, weight: .bold),
                .foregroundColor: NSColor.black,
            ]
            let text = NSAttributedString(string: letter, attributes: attrs)
            let ts = text.size()
            // punch the letter out of the filled box
            NSGraphicsContext.current?.compositingOperation = .destinationOut
            text.draw(at: NSPoint(x: box.midX - ts.width / 2, y: box.midY - ts.height / 2))
            return true
        }
        image.isTemplate = true
        cache[vietnamese] = image
        return image
    }
}
