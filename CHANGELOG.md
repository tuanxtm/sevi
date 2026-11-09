# Changelog

## 1.0.40 — 2026-11-09

- Refined the shared desktop UI with the new background, black and orange accent colors, accented title initials and Credits links, and full-width VIETNAMESE/ENGLISH controls under ACTIVE INPUT. The switch spans the emoji checkbox top through the shortcut text baseline.
- Updated About copy and its three-line Overview. CHECK FOR UPDATE turns orange on hover; update notices replace the footer version for 10 seconds before the version returns.
- Updated the Windows application icon and tray interaction: single-click switches language; double-click opens Settings without also toggling the language.
- Ported Telex literal restoration fixes for complete English spellings, identifiers and names, preserving Vietnamese modifiers and Backspace undo without rewriting preceding Vietnamese words.
- Added 18,197 SCOWL en_US word forms so words such as `resets` and `quests` stay English. Existing frequency scores and Vietnamese recognition take precedence. The data, regeneration tool and attribution are included.
- Fixed Windows/Linux hotkey handling for held modifiers, repeated keys, missed key releases, clicks and focus changes. Windows password fields receive hotkey letters unchanged; language switching remains available after an unrelated text-insertion failure.
- Windows shortcuts now dismiss the emoji picker and pass through. Media/device keys pass through while it stays open, and search translation supports AltGr using the foreground keyboard layout.
- Added decision-to-test coverage to the Rust checks, with explicit mobile exclusions and native acceptance gaps.

Validation includes 219 typing rules, both 14-case Simple Telex suites, 21 reported regressions, driver undo, unchanged syllable snapshots, and 46 native Linux typing cases. Non-timing quality measurements match upstream `79e0c50`. See [UPSTREAM_PORTS.md](UPSTREAM_PORTS.md) for detailed checks and remaining native Windows/macOS/Wayland acceptance limits.

Windows x64 downloads include the installer, standalone executable, portable ZIP, signed update feed and SHA-256 checksums. This is Sevi build 33; upstream Vikey release numbers are independent.
