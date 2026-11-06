# Shared desktop UI

`sevi-ui` draws the settings page, help and practice, about,
credits and emoji search on Windows, Linux and macOS. `sevi-ui-protocol` carries
bounded newline JSON over private parent/child stdin/stdout pipes. The native
hosts retain their own settings stores, accessibility, login, updater and text
insertion services. No socket listener, webview, JavaScript runtime or launcher
service is added. Third-party updater and OS permission/file dialogs retain
their platform implementations.

The UI starts on demand and follows the system theme. Its main width is 600
logical pixels; height fits the settings content and platform-only rows. The
emoji popup is 430 × 430. Settings have no scroll area; Help, About, credits and
long application lists can scroll.
UI fonts are embedded Inter and Geist Mono; colored emoji use the system font.
The title bar is 48 logical pixels high and the footer is 34; both meet the window
edges. VI/EN uses inverted square header cells. Caption icons are painted vectors.
Header/footer and section rules use one hairline stroke. Text actions use uppercase
Geist Mono with hover/focus underlines; selects show text and an arrow, opening a
boxed menu whose selected row uses the VI/EN inverse colors. Shortcut menus retain
the existing recorder. Back combines an 8-by-6-pixel vector arrow and 11-pixel mono BACK;
only the text underlines. There is no page name or rule on its row. The title bar
has only the subtitle, language and caption controls. Help/About/Credits sit at the footer's
left and `SEVI V1.0.30` at the right; the right content inset is 8 logical pixels.
Mono tracking is 0.44 logical pixels. Checked boxes use solid inset squares and
retain egui keyboard/accessibility behavior. Emoji/System share one horizontal row.
The arrow tip shares the content's left inset. Credits opens directly from the footer,
with no duplicate action in About. About presents a version heading, overview and implementation columns. The visible
credits list focuses on Vikey, OpenKey and language/evaluation data; third-party
license files and source acknowledgements remain available in the repository.
History has been removed from windows, native menus,
private commands and Linux launch options. Legacy recording preferences are ignored;
existing history files are not deleted or modified by this removal.
The native IME does not create an egui context, window or GPU device at startup.
The viewport explicitly disables eframe's default icon fallback. This prevents its
deferred icon setter from overwriting Windows' native small/big resource icons;
macOS retains the bundle icon. The supplied assets are unchanged.
The Windows notification-area icons are separate resources: `Windows/App/tray-v.ico`
for Vietnamese and `Windows/App/tray-e.ico` for English. Replacing either source
and rebuilding updates the embedded icon; no external icon files are required at
runtime. Each supplied ICO has 16, 24, 32, 48 and 64-pixel images. The native
[LoadIconMetric](https://learn.microsoft.com/en-us/windows/win32/api/commctrl/nf-commctrl-loadiconmetric)
loader uses the system small-icon metric and scales a larger image when necessary.
The host caches both icons, refreshes them on settings/display/DPI changes, keeps
the previous pair if loading fails, and releases old handles after the tray update.
The settings window and emoji popup have separate helper lifetimes, so a popup
cannot replace or activate a background settings window. Closing either helper
releases its renderer; closing the last window releases all UI processes.

Windows/Linux use Glow/OpenGL; macOS retains the Metal renderer for NSPanel.
The embedded fonts are fixed instances of the launcher's original faces with all
glyphs retained; their source hashes and coordinates are recorded in the assets.
Fat LTO and size optimization apply to the UI/graphics stack. The typing engine,
native hooks and callback-unwind behavior retain their existing speed/safety settings.

## Automated checks

Run from `Rust/`:

```sh
cargo test --locked -p sevi-ui -p sevi-ui-protocol
cargo test --locked -p sevi-linux --features settings-ui
cargo clippy --locked -p sevi-ui -p sevi-linux --features settings-ui --all-targets -- -D warnings
cargo xtask check
```

The layout test covers Windows, Linux and the extra macOS controls at 100%,
125%, 150%, 200% and 300% display scaling, including content sizing and aligned
Emoji/System headings. About layout checks verify left-aligned text, matching
column rows and uniform vertical spacing in both themes. Picker tests cover multilingual search,
grapheme deletion, selection and skin-tone frequency keys. Protocol tests cover
truncation, size limits, Unicode, the last command before helper exit and reopen.
Existing engine/desktop insertion regressions remain applicable.

## Native acceptance

- Open settings from the tray/menu. Check English labels, square checkboxes,
  dropdowns, shortcut capture/cancel, modifier-only language shortcuts and beep.
- Change the system light/dark theme and display scale; move between monitors
  with different scales. All main settings should remain visible at the minimum
  size. Check drag, double-click maximize, minimize, close and keyboard focus.
- Check the taskbar and Alt+Tab icon after startup, activation, minimize/restore,
  page changes, picker close and reopening settings. The deferred egui icon must
  never replace the native Sevi icon.
- Switch between Vietnamese and English. Verify the supplied orange V and blue e
  tray icons, including light/dark taskbars and changed display scale. The main
  window/executable retains its separate Sevi application icon.
- Open Help, About and Credits. They reuse the settings window. Verify
  practice conversion, back arrow, text actions and links.
- On macOS verify Accessibility, per-app automation, excluded apps/add/reset,
  browser automation consent, launch at login and configured Sparkle updates.
- Open emoji from an editor. Search in English, Vietnamese and Telex; test skin
  tones, category selection, mouse/arrow selection, Enter, Escape, select-all,
  paste and grapheme/word deletion. The emoji must reach the original editor.
  Windows must retain the editor's foreground HWND; macOS must use an NSPanel
  without activating the helper. Linux must commit after editor focus returns.
- Close the last window. Verify its helper process exits and typing still works;
  reopen repeatedly. Kill only the helper and verify the native IME recovers.
- Recheck Word, Excel, browser address bars/autocomplete, terminals and remote
  sessions, including blocked insertion into higher-privilege Windows apps.
- Check opening settings in Windows remote/virtual desktop sessions too; the new
  UI renderer needs a working OpenGL context. Text insertion still uses the
  native host and is independent of whether a settings window is open.

Linux screenshots/interactions can be checked in an isolated X11 session. Native
Windows and macOS focus/accessibility/injection behavior still requires testing
on those operating systems; cross-compilation alone cannot establish it.

## Validation in this workspace

The November 6, 2026 build passed the shared UI/model/protocol tests, strict Linux,
Windows x64 and macOS Clippy, Windows release linking/resource inspection, and
macOS ARM64 static-library compilation. The engine check passed 169 rules, both
14-case Simple Telex suites, all 20 reported-bug cases and unchanged golden files.
Its existing synthetic stress baseline remains unchanged.

The private X11 desktop test rendered both themes, changed a preference through
the real host pipe, selected a dropdown item, recorded Alt+X through the shortcut
menu, exercised About and its back arrow, opened emoji beside settings, closed it without losing
settings, and verified helper/host exit after the last window closed. Software
OpenGL produced the first captured frame in approximately 0.2–0.4 seconds in the
final run; this is a local observation, not a Windows/macOS startup benchmark.

The latest icon/footer/arrow screenshots, interaction checks and PE checks are
under `target/ui-touchup/`; the preceding compact layout is under `target/ui-final/`.
They cover both settings/About themes, Help, Credits, solid
square checkbox changes, fitted window size, dropdown selection, shortcut capture,
Back/footer navigation and independent picker/helper lifetimes. The preceding
text-control revision and typing suite log are under `target/ui-refine/`.
Native checks cover a compact dropdown
with inverse selected colors, not just its initial closed state. Legacy History
opt-ins remain disabled after reload on all three platforms; storage tests also
verify no new history counters are written while typing proceeds through blocked
storage. The preceding launcher-style revision is under `target/ui-polish/`;
the initial shared-UI logs remain under `target/ui-redesign/`. The Windows package is
`target/windows-test/Sevi-x64.exe` (unsigned, x64, 10,725,376 bytes), with metadata and
the previous build beside it. All supplied platform icon files are unchanged.

Windows x64 size comparison (bytes, same static CRT):

| Build | Bytes |
| --- | ---: |
| Previous shared UI | 19,407,872 |
| New layout with WGPU and variable fonts | 20,116,480 |
| Glow with fixed font instances and thin LTO | 13,018,624 |
| Glow, fat LTO, UI dependencies optimized for size | 11,364,352 |
| Same build, removing unused duplicate Windows resources | 10,934,784 |
| Inter wordmark, text controls and History removal | 10,696,704 |
| Content-fitted layout, square indicators and revised secondary pages | 10,704,384 |
| Embedded custom Vietnamese/English tray icons | 10,725,376 |

The text-control revision was 238,080 bytes smaller than the preceding launcher-style
build. That build is preserved in
`target/windows-test/previous/sevi-before-text-controls-747126e1/`.
The preceding text-control executable is preserved in
`target/windows-test/previous/sevi-before-compact-layout-bf1b23c8/`.
The icon/footer touch-up kept the executable at 10,704,384 bytes. Its previous build
is preserved in `target/windows-test/previous/sevi-before-icon-footer-eac11579/`.
All eight source ICO images still match the executable's embedded icon resources.
The runtime fallback fix is compiled and resource-checked here; Windows taskbar
and Alt+Tab acceptance still requires native execution.
The custom tray build is 10,725,376 bytes. All 16 portable Windows adapter tests,
strict Windows x64 Clippy, release linking and PE checks passed. Resource checks
match all 18 embedded images to the supplied ICO files (8 app, 5 V, 5 e), including
the language-specific group IDs. Validation output is in `target/tray-icons/`.
The previous executable is preserved in
`target/windows-test/previous/sevi-before-custom-tray-b397bf80/`.

An `opt-level="s"` comparison was larger than `"z"`, so the smaller configuration
is retained. No executable packer, glyph removal or typing-engine size optimization
is used. The emoji JSON and both switch sounds occur exactly once in the final
Windows executable; their compatibility RC entries remain in the reference source.
