# Upstream changes carried into the Rust desktop port

- Repository: https://github.com/tonyminibot/Vikey.git (`origin`).
- Rust branch: `rust-port`; initial saved port: `950c93e`.
- Original migration baseline: `490820bc70c48c38b864cb1b63f6de61f9a44ab3`.
- Last upstream review: **`5f2bfb0257146eb86301f1b86e5c2fb313b1dcd8`** (43 commits since `79e0c50`). The two selected desktop fixes are recorded below.
- Scope: Windows, Linux, macOS and shared desktop typing/tool behavior. **iOS and Android are excluded.**

This is a behavioral port of selected fixes, not a merge of the original hosts into the Rust implementation. Upstream release numbers do not identify a Sevi release. Sevi 1.0.40/build 33 packages these approved ports; the integration artifacts below were produced with the preceding 1.0.30/build 32 test build.

## Applied

| Upstream commits | Rust behavior |
| --- | --- |
| `5112c3a`, `59f2a3a`, `061bbe1` | Full literal restoration preserves recognized English spellings, internal-capital identifiers and unlisted multi-syllable names. Explicit Telex modifiers and tone removal remain protected. New literal-only evidence cannot rewrite a preceding Vietnamese word. The doubled-key path and eight driver undo regressions are included. |
| `7e7c618`, `5069ba8` (D60) | Added the pinned 18,197 SCOWL en_US forms, compact Rust word/offset tables, deterministic importer, Rust Hunspell-based regeneration command, R21 and attribution. Original English scores retain precedence; forms score -8 only when Vietnamese recognition fails. Imported the 219-rule suite and upstream's documented golden changes. |
| `c9fc6f2`, `fbef873`, `5723d0f`, `5c70f96`, `7ee723f`, `e6e8cc4` | Windows/Linux key hotkeys use the current modifiers. Physical key releases distinguish fresh presses from repeats, including an initially unmatched chord. Used modifier chords remain suppressed until all modifiers are released. Click/focus/reload clear held-key state. Windows password/unknown-focus keys bypass hotkeys and transformations; the separate failed-insertion guard still permits language hotkeys. Rust portable/native runners support held-key fixtures. |
| `ed3002f`, `93ae46c`, `297de56` | Windows shortcuts immediately dismiss the emoji picker and pass through, device keys pass through without dismissal, and other keys stay captured. The helper translates AltGr using the foreground keyboard layout. Private protocol 2 distinguishes immediate dismissal from Escape's clear-then-close behavior. Portable routing/state tests and nonempty-search dismissal regression pass. |
| `5069ba8` coverage tooling | `cargo xtask check` audits the historical decisions against tagged engine cases and named Rust tests/checks. A Rust manifest records removed/mobile behavior and native-only gaps. Tests verify that missing coverage and stale named references fail the audit. Current result: 46 tested, four explicit native-acceptance gaps, nine scope exclusions. |
| `06a82f1`, `aa3d0b3`, `f2927da` | Windows/Linux numpad digits follow digit word boundaries. Unmodeled printable/operator keys end typing/log context and cancel pending modifier hotkeys without becoming Esc. Restoring a word preserves the original physical key, including Home and Windows' extended flag; Windows leaves physically held Shift alone. Both live runners understand the new upstream fixtures. |
| `e06b4b1` | Linux updates IBus menu properties only when their displayed values change, and refreshes them on focus. |
| `8125896`, `588747c`, `3824cae`, `9725b77` | Linux stats accumulate in memory until the existing flush timer; failed writes remain dirty for retry, settings reload preserves pending counters, and Clear resets the engine's pending word/counters/events as well as the UI's files. |
| `c334724` | macOS lazily memoizes rewrite/app/Spotlight policy per event, saves smart-switch memory after returning from the event tap, and writes/reads log files on an ordered background worker. Stats are saved only when dirty. |
| `b36a636`, `6f048bc`, `4674005` | macOS hotkey recording uses the same logical layout mapping as hotkey dispatch. Repeated keydown events are swallowed; another press with the modifier held still works. Any mouse button or key cancels a modifier-only chord until every modifier is released. The reverted Latin-layout Telex changes are excluded. |
| `e1ebf6c`, `9f6a323`, `f4672b3` | macOS history reload replaces the previous response receiver so stale reads cannot restore cleared rows. The stats snapshot precedes flushing and the read is ordered after that flush. A bounded tail reader retains a complete record exactly at the byte boundary. |
| `33450eb` | The macOS application menu includes the settings command with Command-comma, in addition to the existing status-menu shortcut. |
| `5e643f8`, `6f1bfa4` | A Spotlight-owned window at the status-item level does not count as the visible Spotlight dialog. |
| `6b29780` (desktop D58) | `docs` remains English in Vietnamese mode: its English dictionary score changes from -1 to 20. Rust and reference dictionary generators agree. Imported the author's exact rules, R20 report, changed golden expectations/metrics and native fixtures; regenerated Rust data with the importer. No other dictionary entry changes. |
| `53eb58e` | Desktop CI drops the `pull_request_review` triggers/associated job conditions and keeps regular push/pull-request execution. |
| `03334ac`, `45c3253` | Restored punctuation and Space use the triggering key's own Shift state; Caps Lock affects letters only. This fixes `NAS,`, `THOONG.` and immediate Backspace undo at Space with Caps Lock. macOS forwards control keys with the original event's full flags. Windows already preserves physical modifiers, and Linux already commits restoration and passes the original event through. Imported all three upstream regression cases and added Caps Lock commands to both Rust live runners and the portable fixture replay. |
| `fc632be`, `403e809` | Windows uses a hidden top-level tool window to receive taskbar/theme broadcasts. A nonzero registered TaskbarCreated message restores the current tray icon after Explorer restarts. Only a taskbar color change rebuilds icons; the popup menu posts WM_NULL on dismissal. Existing DPI handling and keyboard/storage workers are retained. |
| `a4255bc`, `c2f825d`, `c20200b` | Linux tracks the focused client's last reported surrounding-text cursor and resets it on focus-in. On GNOME Wayland, or when the environment cannot rule it out, deletion is capped at that reported cursor; a missing/zero cursor permits no surrounding deletion. Confirmed X11 and non-GNOME desktops keep their previous behavior. Ported upstream's cursor/environment regression matrix. |
| `6ac96b1`, `a389961`, `c193b68` | macOS compares hotkey chords without beep/legacy high bits, prevents collisions in both recorder directions while emoji is enabled, requires Control/Option/Command for emoji chords, clears stale recorder modifiers and invalid saved emoji shortcuts, and clears an old collision when enabling emoji. Repeats are consumed without toggling the picker again; releasing the emoji chord does not trigger a modifier-only language switch. Language-switch recording still accepts Shift with a key. |
| `1775172` | The test executable already used a static runtime. The project Cargo configuration now also enables `crt-static` for MSVC targets, so standard Cargo and CMake builds preserve that behavior without a special shell flag. CMake watches the configuration file as a build dependency. PE import checks verify the x64 executable has no Visual C++ runtime DLL dependency. |
| `5867e3e` | Forgetting typing context clears the engine's after-digit state, so a digit before a language switch cannot make a later `w` behave like a unit. Covers manual and automatic language switches (D33). |
| `9e9b63d` | A word restored to its original keys ends lookback history. A later English word cannot treat an earlier word as adjacent and overwrite text across the restored word (D8.1). |
| `21cb5ea` | Rebuilding keys after Backspace preserves uppercase vowel/tone keys for a whole all-caps word, while a single initial capital retains lowercase extra keys. Reopened `VIEEJT` no longer becomes mixed-case on restoration (D57). |
| `b4fded3` | Clicks and language/context switches discard saved cursor round-trip words in the shared engine and desktop English-mode state. The Rust rule harness/simulators and copied C++ references reset the same navigation state (D51). Imported the author's 12 new rule cases, bringing the locked suite to 169. |
| `f45a489` | Linux commits pending pre-edit text before sending the empty pre-edit update. This follows upstream's Chromium/Electron IBus Wayland duplication fix (#35), without changing the text or adding delays. |
| `2933497` | Debian packaging enables CPack's shared-library dependency scanner to include the minimum versions required by the actual Rust binaries. The Rust Arch repacker accepts generated libc/GCC/GLib/IBus/Cairo/Pango/Fontconfig/X11 dependencies and converts Debian prerelease constraints as upstream does. |

Cleared log files are moved to uniquely named recoverable archives rather than permanently deleted, consistent with this workspace's deletion policy. They are excluded from the active log view.

Linux implementation detail: the initial direct translation of upstream's synthetic Home forwarding failed the native GTK4 fixture. The Rust adapter commits restored text and lets the original event continue instead. This retains native widget navigation and modifiers; IBus GTK4's synthetic forwarding path calls the input-context filter ([IBus source](https://github.com/ibus/ibus/blob/main/client/gtk2/ibusimcontext.c)), rather than replaying a normal widget event.

## Already satisfied by the Rust host

- `1b82721`, `0bcdf79`, `948b9db`: AppKit windows use explicit content sizes; settings/acknowledgements do not rely on the affected SwiftUI fitting-size behavior. Windows uses explicit dimensions too.
- `35831dd`: every macOS menu action already refreshes the menu after changing the per-app auto-switch preference.
- Onboarding permission retry already starts the event tap quietly and redraws the guide only when advancing its permission step.
- The Rust emoji grid uses native AppKit rendering rather than the SwiftUI view invalidated by the old caret-blink timer; that redraw fix is not applicable.
- Linux log file writes already run from the timer rather than the key callback. The per-word stats writes did need the fix above.
- `bf15e68`, `b220a3d`, `1358d1f`, `7f5c85e`: the newer tab-height changes also target SwiftUI/NSHostingView sizing. The Rust AppKit host explicitly sizes its settings content and does not use that layout path. The commits titled “Windows” here modify the macOS `App/Views/Windows.swift` helper.
- `cc31a9c`, `496306d`: the Rust macOS excluded-app list already lives in a fixed 166-point scroll viewport inside a 470-point settings page. Its controls fit inside the scroll content width, including with a conventional scroller. A longer list therefore does not increase window height or push the page off center; the upstream SwiftUI screen-height/scrollbar workaround is unnecessary here.

## Intentionally excluded

- All iOS/Android app, keyboard, release and prediction/Apple Intelligence changes, including phone-only D59 punctuation snapback. Desktop has no corresponding snapback behavior.
- `16975cd`/`05c9f81` mobile startup/memory changes; desktop search behavior is unchanged.
- Crash-report collection/upload and new settings (`c453d46`, `e81b379`, `21e7c18`): a new feature, not an existing desktop bug fix.
- Website/feedback/backend changes, store/release metadata and publication.
- The 2026-11-03/04 phone-only layout, selection deletion, punctuation snapback, pause/reload, stale undo, Android action-key and iOS language-mark changes. The earlier `0e272a7..b74496a` review range had no shared desktop Engine or language-data changes.
- `7f124cd`, `3895f60`, `a1c3c8e`, `f7036d9` and their merge: Android keyboard navigation-bar insets and associated device-test changes. Desktop apps have no corresponding IME keyboard surface.
- `1a6b647` archival test-binary untracking and triage housekeeping. Reference files are retained; pre-existing unrelated deletions and the crash dump are preserved outside this update.

## Validation — 2026-11-02 review

Validation artifacts are under ignored `target/native-session/upstream-*`:

- `cargo xtask check`: passed, including 157/157 rules, 20/20 reported regressions, 11,092 syllable cases, updated upstream golden metrics, driver parity, performance and realistic-edit/touch limits. The existing four stuck synthetic cases remain at baseline; text-safety violations are zero.
- Desktop/session tests and five persistence regressions passed. The macOS serial log worker's write/read/clear ordering test passed on Linux.
- All ten imported upstream dictionary/rule/golden/native-fixture files match the reviewed revision byte for byte. The Rust importer check passes and only the `docs` English score differs from the original Rust dictionary.
- Native GTK4/IBus: **40/40**, including numpad input and restored-word Home movement (`target/native-session/run.ewx6IV/live.log`). A separate three-case Home/End regression run passed too.
- Strict Clippy: portable/Linux, Windows x64, macOS Intel and Apple Silicon passed. Linux CMake release rebuild and macOS Apple Silicon static archive build passed.
- Quality comparison against the reviewed C++ upstream revision: every non-timing measurement matches. This run measured 526 ns/key upstream versus 571 ns/key Rust on Linux; this is an engine microbenchmark, not whole-application Windows performance.
- Windows x64 app and live runner link successfully. PE/resource/static-runtime inspection passed. Refreshed executable: `target/windows-test/Sevi-x64.exe`, 5,431,296 bytes, SHA-256 `b9a12977757e7df5fb8d6f696c760fe3bcff4e8eccc44487ca46605cef708cf8`. Only x64 was rebuilt; the prior executable is preserved under `target/windows-test/previous/950c93e/`.

Windows execution and macOS bundle/runtime acceptance require their respective operating systems; cross-compilation is not a substitute. The executable is unsigned. The user reported normal Windows testing passed for the previous `950c93e` build. No remote push, publication, system installation or mobile changes were performed.

## Validation — 2026-11-04 review

Review artifacts are under `Rust/target/upstream-2026-11-04/` (or `target/upstream-2026-11-04/` when running from `Rust`). `commits.txt` records all 41 reviewed commits, and `desktop.diff` records the upstream desktop changes. Only Rust-port sources/resources were edited; original hosts outside `Rust/` and mobile apps were left alone. The prior Windows background-storage latency fix remains in place.

- `cargo xtask check`: passed, including 157 rules, 20 reported regressions, unchanged engine goldens, syllable lock, driver parity, and the accepted existing realistic-edit/touch limits. The engine/dictionary source did not need a port in this range.
- Portable desktop/persistence tests pass, including upstream's Caps Lock fixtures on all three desktop policies, macOS emoji repeat/release behavior and invalid saved-chord cleanup. The prior Windows blocked-storage and persistence regressions still pass.
- Linux's cursor-limit/environment matrix passes, including absent environment variables, zero/missing cursor, GNOME Wayland, X11 and KDE.
- Native GTK4/IBus on an isolated X11 desktop: **43/43**, including all three new Caps Lock cases, numpad/navigation, emoji commit and prior undo/lookback behavior. The harness uses a project-owned profile and private D-Bus/PID/display namespaces; no normal desktop services were restarted.
- Strict Clippy passes for portable code, Linux, Windows x64, and macOS Intel/Apple Silicon. The Linux CMake desktop build, Windows x64 application/live runner, and macOS Apple Silicon static archive build pass. Windows PE/static-runtime/resource and exact source-icon checks pass.
- The Windows release was also rebuilt with `RUSTFLAGS` and `CARGO_ENCODED_RUSTFLAGS` unset, verifying that the project configuration supplies static runtime linkage. See [Rust's runtime linkage reference](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes) and [Cargo's compiler-flag configuration](https://doc.rust-lang.org/cargo/reference/config.html#buildrustflags).

The refreshed Windows test package is `Rust/target/windows-test/Sevi-x64.exe`. Its JSON manifest records the exact hash and preserved prior package. Native Windows Explorer/theme behavior, macOS UI/event delivery and GNOME Wayland compositor behavior require testing on those systems; cross-builds and policy tests do not establish that acceptance.

## Validation — 2026-11-06 review

Artifacts are under `Rust/target/upstream-2026-11-06/`. The complete 21-commit range and upstream desktop diff are recorded in `commits.txt` and `desktop.diff`. All edits remain inside `Rust/`; copied C++ engine/test references were updated only for these fixes. No dictionary changes, mobile features, new UI features or automatic update feeds were added. The existing Sevi icons, branding, insertion checks and Windows storage-latency fix are preserved.

- `cargo xtask check`: passed **169/169** rules and **20/20** reported bugs, with unchanged goldens/syllable lock and passing driver, realistic-edit, touch and performance limits. The known synthetic stuck cases remain within the existing accepted baseline.
- A desktop-session regression checks that a click or two manual switches while away cannot resurrect the English word buffer on Windows, Linux or macOS. It also checks that an uninterrupted cursor round trip still restores the intended context, with automatic switching explicitly enabled.
- Quality comparison against upstream `350efb2`: **every non-timing measurement matches**, including text-safety metrics and all 169 locked cases. The Linux engine benchmark measured 581 ns/key for Rust versus 532 ns/key for C++ in this run; this is not whole-application Windows latency.
- Native GTK4/IBus X11 suite: **43/43**. The pre-edit commit-order change introduces no regressions in these clients. Chromium/Electron on IBus Wayland still needs native acceptance; X11 testing does not prove that compositor-specific behavior.
- Strict portable/Linux/Windows x64/macOS Intel and Apple Silicon Clippy passed. Linux desktop/live-runner, Windows x64 app/live-runner, and macOS Apple Silicon static archive builds passed. Windows PE/static-runtime and exact source-icon checks passed.
- CMake's generated CPack configuration enables `CPACK_DEBIAN_PACKAGE_SHLIBDEPS`; Arch shared-library dependency/version conversion tests pass. A new Debian package was not generated on this host because Debian's `dpkg-shlibdeps` tooling is unavailable. Debian package creation must be validated on a Debian/Ubuntu build host with that tool installed; no system dependencies or user configuration were changed here.

The refreshed Windows executable and matching live runner/PDB are under `Rust/target/windows-test/`; `Sevi-x64.json` records their hashes and the previous package's preserved location. Native Windows/macOS acceptance remains pending. Version/build stay at 1.0.30/32.

## Assessment — 2026-11-09 (completed by the validated port below)

Fetched `origin main` after the requested UI adjustments. Reviewed the complete
`350efb2..79e0c50` range (27 commits, 44 changed files). Upstream's desktop version
is 1.0.32; Sevi remains 1.0.30/build 32. The initial assessment did not apply
changes. The subsequent user-authorized implementation is recorded below.

Artifacts are under `Rust/target/upstream-review-2026-11-09/`: `commits.txt`, the
complete `upstream.diff`, and `rule-comparison.json`. At assessment time, replaying
all **50 newly added upstream rules** with the then-current Rust harness gave
**21 matches and 29 differences**. All 50 now pass as part of the 219-rule suite.

### Approved implementation plan (now completed)

| Priority | Upstream commits | Decision and Rust targets |
| --- | --- | --- |
| 1 | `5112c3a`, `59f2a3a`, `061bbe1`, merge `51c4899` | Port literal restoration as one complete behavior change in `sevi-engine/src/engine.rs`. Preserve full recognized English spellings and internal-capital identifiers, allow unlisted multi-syllable literals, and keep explicit Telex modifiers/tone removal protected. New literal-only evidence must restore the current word without rewriting preceding Vietnamese. Include the doubled-key path, Backspace undo and all accompanying guards. |
| 2 | `7e7c618`, final implementation `5069ba8` | Port D60's **18,197 SCOWL en_US word forms**, their importer/generator, regression R21 and attribution. Integrate through `sevi-engine/src/data.rs` and every English-scoring call path. Existing dictionary scores take precedence; a missing form scores -8 only when it is not Vietnamese. Use the final affix-expanded, listed-base-word dataset, rather than the rejected suffix guess or unrestricted dictionary. Update acknowledgements and UI Credits when the data is actually included. |
| 3 | `c9fc6f2`, `fbef873`, `5723d0f`, `5c70f96`, `7ee723f`, `e6e8cc4`, merges `0c79876`, `a74c78e` | Port Windows/Linux hotkey parity into `sevi-runtime/src/session.rs`, Windows `host.rs` and Linux `ibus.rs`. Match current key modifiers; distinguish a fresh press from held-key repeats, including an initially unmatched chord; suppress modifier-only switches after clicks, other keys and emoji chords until all modifiers are released. Clear held/chord state on focus changes and settings reload. Check Windows password/unknown-focus protection before language hotkeys. Import held-key native fixture syntax and cases into the Rust runners. |
| 4 | `ed3002f`, `93ae46c`, `297de56`, merge `3113ead` | Port Windows picker key routing in `sevi-windows/src/host.rs` and `sevi-ui/src/native/windows.rs`: Windows shortcuts close the picker and pass through; media/browser/launch/PrintScreen keys pass through while it stays open; other keys remain captured. Support AltGr through Ctrl+Alt translation in the foreground keyboard layout, preserving Alt+Backspace and clipboard actions. |
| 5 | `5069ba8` decision coverage tooling | Adapt decision-to-test coverage to Rust `xtask check` after the engine/data ports. The upstream manifest refers to C++/Swift tests and excluded mobile paths; map applicable decisions to Rust tests, with explicit native-acceptance gaps, rather than copying that manifest unchanged. |

Representative gaps measured before the port (now corrected; spaces omitted here for readability):

| Typed text / initial language | Current Rust output | Upstream expectation |
| --- | --- | --- |
| `currentState` / Vietnamese | `curentState` | `currentState` |
| `dessert` / Vietnamese | `desert` | `dessert` |
| `Sapa` / Vietnamese | `Sâp` | `Sapa` |
| `been Momo` / Vietnamese | `bên Môm` | `bên Momo` |
| `this get resets` / English | `this get rêts` | `this get resets` |
| `quests` / Vietnamese | `quets` | `quests` |

The pre-port harness already preserved the sampled Telex guards `hiszuf` → `hìu`
and `chi phis` → `chi phí`; it also already restores `resizes`. Preserve those
successes while fixing the missing cases. Before the port, the shared runtime's
repeat/current-flag and used-chord protections were restricted to macOS, and Windows
routed all picker keydowns into a helper whose translation guard rejected AltGr.
The completed port corrects these production paths.

Bring the new **219-rule** upstream suite, R21, the eight new driver undo cases,
held-key live fixtures and documented golden changes with the corresponding ports.
Validate default and Simple Telex, portable desktop/session behavior, native live
typing, and non-timing quality parity. The upstream golden wrong-word counts are
unchanged; manual-switch counts improve from 61/74/120 to 57/70/110 across chat,
chat_typos and synthetic sets. These are upstream results, not results claimed for
the pre-port Rust implementation. The completed comparison below confirms parity.

### Deferred or excluded from this range

- `a8980e3`, `99b394f`, merge `394120a`: defer the Linux first-install IBus recovery
  until there is an appropriate Sevi setup action. Upstream runs an asynchronous
  `ibus list-engine --name-only` query at a specific onboarding step and restarts
  once only after a successful nonempty list omits its engine. Sevi opens its shared
  settings directly and has no equivalent setup step. Do not put a restart on every
  settings launch or copy the upstream `vikey` engine identifier. Any future recovery
  should use `sevi`, retain the failed/empty/already-known guards, and run only in the
  intended setup flow. No IBus query, restart or shared-service change was performed
  during this assessment.
- `c8342ab`, `f8e2fa0`, `edefd0c`, `fce5b00`: iOS release, review/feedback nudges,
  hidden diagnostics and mobile suggestion-strip changes remain excluded.
- `9556269`, `79e0c50`: upstream changelog/release metadata, appcast, hashes and
  version/build changes are excluded. Keep Sevi's branding, updater configuration,
  icons, release process and local UI changes.

## Validation — 2026-11-09 approved ports

The implementation, data and fixture changes are confined to `Rust/`. Reference
engine/undo sources preserve the existing Sevi identifiers. The new Overview copy
has the requested three explicit lines; all earlier title, color, section, footer
timer, Credits and full-label switch changes remain present.

- `cargo xtask check`: passed. All **219 rules**, both **14-case** Simple Telex
  suites, **21 reported regressions**, **14 driver undo cases**, exact upstream
  goldens/metrics and the unchanged **11,092-syllable** snapshot pass. Engine,
  desktop, persistence, tools, UI and protocol tests pass. The performance and
  realistic-edit/touch budgets pass; the existing synthetic stuck-case baseline
  remains unchanged, with zero guard/elsewhere text-safety violations.
- `cargo xtask import-legacy-data --check`: all reference tables match byte for
  byte, including the supplemental form blob and offsets. The original English
  table's 41,225 entries and frequency scores are unchanged by D60.
- Quality comparison against `79e0c50`: **every non-timing measurement matches**,
  including all 219 locked cases, 21/21 reports and zero bad text edits. This run
  measured 640 ns/key for Rust versus 567 ns/key for C++; the Rust check's measured
  mean was 643 ns/key against the 1,000 ns/key budget. These are Linux engine
  microbenchmarks, not native Windows application-latency measurements.
- Native GTK4/IBus X11 typing: **46/46 passed**, including holding Alt for two
  distinct Z presses, a held/repeating Z switching once, prior Caps Lock cases,
  restored-word navigation and emoji insertion. The session used a project-owned
  profile, PID/network/mount namespaces, Xvfb and private D-Bus/IBus. No normal
  desktop configuration or shared service was changed. The task namespace exited.
- Strict Clippy passes for portable/tool/UI code, Windows x64 (including native
  hook/helper/live-runner targets), and Linux with `native-tests,settings-ui`.
  Linux production tests and the final protocol policy tests pass. Both native
  Linux fixture/settings binaries and Windows app/live-runner binaries build.
- Windows PE checks confirm AMD64, GUI subsystem and system DLL imports without
  a Visual C++/dynamic CRT dependency. Native Windows shortcut routing, AltGr in
  actual keyboard layouts and password-field acceptance still require Windows;
  cross-building and portable policy tests do not establish those OS interactions.
  macOS bundle/runtime and GNOME Wayland acceptance remain native follow-ups.

Validation artifacts are under `Rust/target/upstream-port-2026-11-09/`, including
`check.log`, `quality.json`, the private-session launcher and
`native.fsYX01/live.log`. The refreshed executable is
`Rust/target/windows-test/Sevi-upstream-20261109-x64.exe` (SHA-256
`6dfe299167e5db25fca7d7adda8b59154addeac94895439cfe4926a7a0ed40ad`), with a
matching `Sevi-upstream-20261109-livetest-x64.exe` and acknowledgement file.
Earlier test executables are preserved. No commit, push, publication, signing,
installer execution, global installation, mobile port or version bump was made.

## Port — 2026-11-19

The November 19 review ported Vikey #121 (D61) and #128 (D62). Backspace
first removes the separator, and a second consecutive Backspace restores the
original word and language. Return and keypad Enter end the word without
rewriting it. The regression cases cover both language directions and keep
the corpus snapshots based on the exact key sequences. The manual-switch
simulator also tries a space before Enter if it would correct the last word.

`cargo xtask check` passed all 229 rules with the existing typing metrics
and safety limits. Native GTK4/IBus X11 passed 55 cases, including both fixes,
after a warm-up key. A cold run duplicated the first fixture and passed the
other 49. Windows and macOS were covered by the portable desktop tests.

The macOS stapling retry (`e1ea59e`) remains deferred until macOS release
work. Mobile suggestions, tap areas, memory fixes, and model loads are
outside the desktop port. The unmerged word-pair scoring experiment was
reviewed separately and remains deferred.

## Next review

The Rust implementation and regression fixtures live entirely under `Rust/`.
The copied C/C++/Objective-C++/Swift implementations and header importer have
been retired. Translate selected upstream behavior into Rust and retain relevant
regression cases. Generate Rust data directly with `cargo xtask tool`, review
intentional data changes, then run `cargo xtask lock-data` and `cargo xtask check`.
Scorecard comparisons require a Rust-era revision. Historical import/check
results and artifact paths above describe the implementation at that time.

Fetch `origin main`, then review the diff from the last reviewed upstream SHA
above. Select desktop bug fixes, translate behavior into the Rust hosts,
import applicable upstream regression expectations, and review the entire
range before advancing the marker. Keep the deferred Linux setup behavior,
mobile and new feature exclusions, and native acceptance limits explicit.
