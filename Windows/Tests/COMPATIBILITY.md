# Windows Rust port acceptance

The insertion method is unchanged: tagged `SendInput` batches, Unicode text,
Backspace replacement and the existing narrow-space autocomplete workaround.
An event count confirms queue insertion, not that an application applied the
intended edit. Do not claim application compatibility from the standard-control
runner alone.

## Automated checks on Windows x64

Exit the previous Sevi tray process, launch the refreshed `Sevi-x64.exe`, then
run the packaged Rust live runner from a normal command prompt:

```bat
Sevi-x64-livetest.exe --control=edit
Sevi-x64-livetest.exe --control=richedit
```

Both commands include the original typing cases plus `cases-compat.tsv` by
default. Optional positional paths select explicit TSV fixtures. A test input
insertion failure aborts the runner with an error, rather than continuing with
an incomplete test sequence. The controls must remain foreground during testing.

## Application checks (require native Windows)

Use a disposable document/field/session. Type `tieengs vieetj ` (expect `tiếng
việt `), mixed text `deploy leen server nhes ` (expect `deploy lên server nhé `),
and the correction/undo sequence `more ` then Backspace (expect `moẻ`). Also
select existing text with Ctrl+A, Shift+Home and Shift+Left, replace it, delete a
selection, move with Home/End, and continue typing. Check surrounding text as well
as the new word; no extra spaces, missing characters or unintended deletions.

| Target | Additional scenario | Status |
| --- | --- | --- |
| Word | Plain document, selected text and continued editing | Pending Windows execution |
| Excel | Cell edit mode and formula bar separately; Enter/Tab commit | Pending Windows execution |
| Edge or Chrome | Textarea, contenteditable, address-bar and form autocomplete suggestions | Pending Windows execution |
| Windows Terminal | Local shell input, selection, Backspace and cursor movement; do not execute test text | Pending Windows execution |
| Remote Desktop | Compose inside remote editor; test local and remote IME separately to avoid double processing | Pending Windows execution |
| Elevated editor | Normal-privilege Sevi targeting an administrator editor | Pending Windows execution |

For blocked injection: the triggering physical key must pass through if zero
events were inserted. Subsequent typing stays raw in the same foreground window,
including after Space/Enter, clicks, settings reloads and language toggles. Switch
to a different window and return to allow another attempt. Password/unknown-focus
guards may already bypass transformation before `SendInput` is reached.

Partial insertion is covered by deterministic fault-injection unit tests. The
IME does not retry text or attempt document repair: the triggering key is
suppressed, stale correction/navigation state is discarded, and only an unmatched
synthetic key-up is attempted once. A blocked cleanup cannot guarantee release;
visible partial text may need manual correction. No elevation or UIAccess bypass
is introduced. Windows permits injection only at equal/lower integrity levels:
[SendInput reference](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput).

This Linux build environment cannot execute these native acceptance checks.
