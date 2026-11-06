import AppKit
import Combine
import SwiftUI

/// App-wide state the menu bar and windows observe.
@MainActor
final class AppState: ObservableObject {
    static let shared = AppState()

    @Published private(set) var language: Int = Int(SeviCore.language())
    @Published private(set) var isRunning = false
    @Published var needsOnboarding = false
    /// Bumped when the app in front changes, so the menu bar re-reads the per-app auto switch toggle.
    @Published private(set) var frontAppTick = 0
    func frontAppChanged() { frontAppTick &+= 1 }

    private var permissionTimer: Timer?

    private init() {
        NotificationCenter.default.addObserver(forName: .SeviLanguageDidChange, object: nil, queue: .main) { [weak self] _ in
            Task { @MainActor in self?.language = Int(SeviCore.language()) }
        }
    }

    /// Starts the keyboard core when Accessibility is granted; otherwise shows onboarding and waits.
    func startIfPossible() {
        if SeviCore.isAccessibilityTrusted(), SeviCore.start() {
            isRunning = true
            needsOnboarding = false
            language = Int(SeviCore.language())
            permissionTimer?.invalidate()
            permissionTimer = nil
            return
        }
        isRunning = false
        needsOnboarding = true
        OnboardingWindow.show()
        if permissionTimer == nil {
            permissionTimer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { _ in
                Task { @MainActor in
                    if SeviCore.isAccessibilityTrusted() { AppState.shared.startIfPossible() }
                }
            }
        }
    }

    func setLanguage(_ value: Int) {
        SeviCore.setLanguageByUser(Int32(value))
        language = Int(SeviCore.language())
    }

    func toggleLanguage() { setLanguage(language == 1 ? 0 : 1) }
}

extension Notification.Name {
    static let SeviLanguageDidChange = Notification.Name(rawValue: "SeviLanguageDidChange")
}
