// Updates from vikey.app/appcast.xml: the same Sparkle feed as the Mac app, with Windows enclosures
// (sparkle:os="windows", sevi:arch="x64"|"arm64") signed with the same EdDSA key.
// Checks daily when "Tự động cập nhật" is on, downloads and verifies the installer, and installs it
// when Sevi quits (or at the next launch). "Kiểm tra cập nhật…" checks now and asks.
#pragma once
#include <windows.h>
namespace sevi { namespace updater {
const UINT WM_UPDATER = WM_APP + 4;   // posted to the owner: wParam = Event
enum Event { EvUpToDate = 1, EvFailed, EvAvailable, EvDownloaded };
void start(HWND owner);               // first check after a short delay, then daily
void checkNow(HWND owner);            // manual check; the result is shown to the user
void onEvent(WPARAM ev);              // the owner's handler for WM_UPDATER
bool installPendingAndQuit();         // at quit: runs a downloaded, verified installer; true if it did
bool installPendingAtLaunch();        // at launch: a verified newer installer waits → run it and exit
}}
