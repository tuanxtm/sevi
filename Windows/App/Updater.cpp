#include "Updater.h"
#include "Prefs.h"
#include "resource.h"
#include <winhttp.h>
#include <shellapi.h>
#include <commctrl.h>
#include <string>
#include <vector>
#include <thread>
#include <atomic>
#include <fstream>
#include <ctime>
extern "C" {
#include "third_party/tweetnacl.h"
void randombytes(unsigned char* b, unsigned long long n) { for (unsigned long long i = 0; i < n; i++) b[i] = 0; }   // never used: we only verify
}
#pragma comment(lib, "winhttp.lib")

namespace sevi { namespace updater {

#ifndef SEVI_VERSION
#define SEVI_VERSION "0.0.0"
#endif
#if defined(_M_ARM64)
static const wchar_t* ARCH = L"arm64";
#else
static const wchar_t* ARCH = L"x64";
#endif
// Sparkle public key (App/Info.plist SUPublicEDKey), base64
static const char* PUBKEY_B64 = "HieG7HYkZUv8/R1Ve/+QdvpSUw4nL39th5T9FDnsh5U=";
static const wchar_t* DEFAULT_FEED = L""; // Configure a Sevi feed before enabling updates.

struct Release { std::wstring version, url, signature, notes; unsigned long long length = 0; };
static HWND _owner;
static std::atomic<bool> _busy{false};
static bool _manual = false;
static Release _found;                 // the newer release the last check found
static std::wstring _pendingPath;      // a downloaded + verified installer
static std::wstring _lastError;

// ---- helpers -------------------------------------------------------------------------------------
static std::vector<unsigned char> b64decode(const std::string& s) {
    static const std::string T = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    std::vector<unsigned char> out; int val = 0, bits = -8;
    for (char c : s) { if (c == '=') break; size_t p = T.find(c); if (p == std::string::npos) continue; val = (val << 6) | (int)p; bits += 6; if (bits >= 0) { out.push_back((unsigned char)((val >> bits) & 0xFF)); bits -= 8; } }
    return out;
}
static std::string narrow(const std::wstring& w) { int n = WideCharToMultiByte(CP_UTF8, 0, w.c_str(), (int)w.size(), nullptr, 0, nullptr, nullptr); std::string s(n, 0); WideCharToMultiByte(CP_UTF8, 0, w.c_str(), (int)w.size(), &s[0], n, nullptr, nullptr); return s; }
static std::wstring widen(const std::string& s) { int n = MultiByteToWideChar(CP_UTF8, 0, s.c_str(), (int)s.size(), nullptr, 0); std::wstring w(n, 0); MultiByteToWideChar(CP_UTF8, 0, s.c_str(), (int)s.size(), &w[0], n); return w; }
/// "1.0.14" vs "1.0.9": numeric per part
static int compareVersions(const std::wstring& a, const std::wstring& b) {
    size_t i = 0, j = 0;
    while (i < a.size() || j < b.size()) {
        long x = 0, y = 0;
        while (i < a.size() && a[i] != L'.') { if (iswdigit(a[i])) x = x * 10 + (a[i] - L'0'); i++; }
        while (j < b.size() && b[j] != L'.') { if (iswdigit(b[j])) y = y * 10 + (b[j] - L'0'); j++; }
        if (x != y) return x < y ? -1 : 1;
        if (i < a.size()) i++; if (j < b.size()) j++;
    }
    return 0;
}
static std::wstring attr(const std::string& tag, const char* name) {
    std::string k = std::string(name) + "=\""; size_t p = tag.find(k); if (p == std::string::npos) return L"";
    p += k.size(); size_t e = tag.find('"', p); return widen(tag.substr(p, e - p));
}
static std::string element(const std::string& xml, const char* name) {
    std::string open = "<" + std::string(name) + ">", close = "</" + std::string(name) + ">";
    size_t p = xml.find(open); if (p == std::string::npos) return ""; p += open.size(); size_t e = xml.find(close, p); return e == std::string::npos ? "" : xml.substr(p, e - p);
}
static std::wstring stripHtml(const std::string& html) {
    std::string s = html; size_t p;
    if ((p = s.find("<![CDATA[")) != std::string::npos) { s = s.substr(p + 9); if ((p = s.find("]]>")) != std::string::npos) s = s.substr(0, p); }
    std::string out; bool in = false;
    for (size_t i = 0; i < s.size(); i++) { char c = s[i]; if (c == '<') { in = true; if (s.compare(i, 4, "<li>") == 0) out += "\n• "; continue; } if (c == '>') { in = false; continue; } if (!in && c != '\n' && c != '\r') out += c; }
    std::wstring w = widen(out); while (!w.empty() && iswspace(w.front())) w.erase(0, 1); while (!w.empty() && iswspace(w.back())) w.pop_back(); return w;
}

// ---- HTTP (WinHTTP) ------------------------------------------------------------------------------
static bool httpGet(const std::wstring& url, std::vector<unsigned char>& out, unsigned long long maxBytes = 200ull << 20) {
    out.clear();
    URL_COMPONENTS uc = {sizeof(uc)}; wchar_t host[256], path[2048]; uc.lpszHostName = host; uc.dwHostNameLength = 256; uc.lpszUrlPath = path; uc.dwUrlPathLength = 2048;
    if (!WinHttpCrackUrl(url.c_str(), 0, 0, &uc)) return false;
    HINTERNET s = WinHttpOpen(L"Sevi/" SEVI_VERSION, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0);
    if (!s) return false;
    bool ok = false;
    HINTERNET c = WinHttpConnect(s, host, uc.nPort, 0);
    HINTERNET r = c ? WinHttpOpenRequest(c, L"GET", path, nullptr, WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES, uc.nScheme == INTERNET_SCHEME_HTTPS ? WINHTTP_FLAG_SECURE : 0) : nullptr;
    if (r && WinHttpSendRequest(r, WINHTTP_NO_ADDITIONAL_HEADERS, 0, WINHTTP_NO_REQUEST_DATA, 0, 0, 0) && WinHttpReceiveResponse(r, nullptr)) {
        DWORD status = 0, sl = sizeof(status);
        WinHttpQueryHeaders(r, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_HEADER_NAME_BY_INDEX, &status, &sl, WINHTTP_NO_HEADER_INDEX);
        if (status == 200) {
            ok = true; DWORD avail = 0;
            while (WinHttpQueryDataAvailable(r, &avail) && avail) {
                size_t at = out.size(); out.resize(at + avail); DWORD got = 0;
                if (!WinHttpReadData(r, out.data() + at, avail, &got)) { ok = false; break; }
                out.resize(at + got); if (out.size() > maxBytes) { ok = false; break; }
            }
        }
    }
    if (r) WinHttpCloseHandle(r); if (c) WinHttpCloseHandle(c); WinHttpCloseHandle(s);
    return ok;
}

// ---- feed -------------------------------------------------------------------------------------
static std::wstring feedUrl() { std::wstring u = prefs::getString(L"AppcastURL", L""); return u.empty() ? DEFAULT_FEED : u; }
/// The newest Windows release for this architecture in the feed; false when the feed has none.
static bool newestInFeed(const std::string& xml, Release& best) {
    bool any = false; size_t p = 0;
    while ((p = xml.find("<item>", p)) != std::string::npos) {
        size_t e = xml.find("</item>", p); if (e == std::string::npos) break;
        std::string item = xml.substr(p, e - p); p = e;
        size_t q = 0;
        while ((q = item.find("<enclosure", q)) != std::string::npos) {
            size_t qe = item.find('>', q); std::string tag = item.substr(q, qe - q); q = qe;
            if (attr(tag, "sparkle:os") != L"windows" || attr(tag, "sevi:arch") != ARCH) continue;
            Release r; r.url = attr(tag, "url"); r.signature = attr(tag, "sparkle:edSignature"); r.length = _wtoi64(attr(tag, "length").c_str());
            r.version = widen(element(item, "sparkle:shortVersionString")); if (r.version.empty()) r.version = widen(element(item, "sparkle:version"));
            r.notes = stripHtml(element(item, "description"));
            if (!any || compareVersions(r.version, best.version) > 0) { best = r; any = true; }
        }
    }
    return any;
}
/// Sparkle's EdDSA signature: ed25519 over the whole file, base64.
static bool verify(const std::vector<unsigned char>& file, const std::wstring& sigB64) {
    std::vector<unsigned char> sig = b64decode(narrow(sigB64)), pk = b64decode(PUBKEY_B64);
    if (sig.size() != 64 || pk.size() != 32) return false;
    std::vector<unsigned char> sm(sig); sm.insert(sm.end(), file.begin(), file.end());
    std::vector<unsigned char> m(sm.size()); unsigned long long mlen = 0;
    return crypto_sign_open(m.data(), &mlen, sm.data(), sm.size(), pk.data()) == 0;
}
static std::wstring updatesDir() { std::wstring d = prefs::appDataDir() + L"\\updates"; CreateDirectoryW(d.c_str(), nullptr); return d; }
static std::wstring installerPath(const Release& r) { return updatesDir() + L"\\Sevi-" + r.version + L"-" + ARCH + L"-Setup.exe"; }
static void rememberPending(const std::wstring& path, const std::wstring& version) { prefs::setString(L"PendingUpdatePath", path.c_str()); prefs::setString(L"PendingUpdateVersion", version.c_str()); _pendingPath = path; }
static void forgetPending() { prefs::remove(L"PendingUpdatePath"); prefs::remove(L"PendingUpdateVersion"); _pendingPath.clear(); }

/// Downloads and verifies the installer; keeps it for the install.
static bool download(const Release& r) {
    std::wstring path = installerPath(r);
    std::vector<unsigned char> file;
    if (!httpGet(r.url, file)) { _lastError = L"Không tải được bản cập nhật."; return false; }
    if (r.length && file.size() != r.length) { _lastError = L"File tải về không đúng kích thước."; return false; }
    if (!verify(file, r.signature)) { _lastError = L"Chữ ký của bản cập nhật không hợp lệ."; return false; }
    std::ofstream f(path, std::ios::binary); f.write((const char*)file.data(), file.size()); f.close();
    if (!f) { _lastError = L"Không ghi được file cập nhật."; return false; }
    rememberPending(path, r.version);
    return true;
}
static bool runInstaller(const std::wstring& path, bool silent) {
    std::wstring args = std::wstring(silent ? L"/VERYSILENT" : L"/SILENT") + L" /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS /RELAUNCH=1";
    HINSTANCE h = ShellExecuteW(nullptr, L"open", path.c_str(), args.c_str(), nullptr, silent ? SW_HIDE : SW_SHOW);
    return (INT_PTR)h > 32;
}

// ---- checks ---------------------------------------------------------------------------------------
static void checkThread(bool manual) {
    std::vector<unsigned char> xml; Event ev = EvFailed;
    if (httpGet(feedUrl(), xml, 4 << 20)) {
        Release r; std::string s(xml.begin(), xml.end());
        prefs::setInt(L"LastUpdateCheck", (int)time(nullptr));
        if (!newestInFeed(s, r) || compareVersions(r.version, widen(SEVI_VERSION)) <= 0) ev = EvUpToDate;
        else {
            _found = r;
            if (manual) ev = EvAvailable;                      // ask first
            else ev = download(r) ? EvDownloaded : EvFailed;   // automatic: fetch now, install at quit
        }
    } else _lastError = L"Không kiểm tra được cập nhật. Kiểm tra kết nối mạng rồi thử lại.";
    _manual = manual; _busy = false;
    PostMessageW(_owner, WM_UPDATER, ev, 0);
}
static void check(bool manual) {
    if (_busy.exchange(true)) return;
    std::thread(checkThread, manual).detach();
}
void onEvent(WPARAM ev) {
    if (!_manual) return;   // automatic checks stay silent
    std::wstring cur = widen(SEVI_VERSION);
    switch (ev) {
    case EvUpToDate: MessageBoxW(nullptr, (L"Bạn đang dùng bản mới nhất của Sevi (" + cur + L").").c_str(), L"Sevi", MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND); break;
    case EvFailed: MessageBoxW(nullptr, _lastError.c_str(), L"Sevi", MB_OK | MB_ICONWARNING | MB_SETFOREGROUND); break;
    case EvAvailable: {
        std::wstring head = L"Sevi " + _found.version + L" đã có", body = L"Bạn đang dùng " + cur + L".";
        if (!_found.notes.empty()) body += L"\n\n" + _found.notes;
        body += L"\n\nSevi sẽ tải về, cài đặt và tự mở lại.";
        TASKDIALOG_BUTTON btns[] = {{IDYES, L"Cài đặt"}, {IDNO, L"Để sau"}};
        TASKDIALOGCONFIG cfg = {sizeof(cfg)}; cfg.hInstance = GetModuleHandleW(nullptr); cfg.dwFlags = TDF_ALLOW_DIALOG_CANCELLATION | TDF_POSITION_RELATIVE_TO_WINDOW;
        cfg.pszWindowTitle = L"Sevi"; cfg.pszMainIcon = MAKEINTRESOURCEW(IDI_APP); cfg.pszMainInstruction = head.c_str(); cfg.pszContent = body.c_str();
        cfg.pButtons = btns; cfg.cButtons = 2; cfg.nDefaultButton = IDYES;
        int pressed = 0; TaskDialogIndirect(&cfg, &pressed, nullptr, nullptr);
        if (pressed != IDYES) break;
        _busy = true;
        std::thread([] { bool ok = download(_found); _busy = false; _manual = true; PostMessageW(_owner, WM_UPDATER, ok ? EvDownloaded : EvFailed, 0); }).detach();
        break; }
    case EvDownloaded:
        if (runInstaller(_pendingPath, false)) { forgetPending(); PostMessageW(_owner, WM_CLOSE, 0, 0); }
        else MessageBoxW(nullptr, L"Không chạy được trình cài đặt.", L"Sevi", MB_OK | MB_ICONWARNING | MB_SETFOREGROUND);
        break;
    }
}

static const UINT_PTR TIMER_UPDATE = 0x7001;
static void CALLBACK timerProc(HWND, UINT, UINT_PTR, DWORD) {
    if (!prefs::getInt(L"AutoUpdate", 1)) return;
    int last = prefs::getInt(L"LastUpdateCheck", 0);
    if (time(nullptr) - last >= 24 * 3600) check(false);
}
void start(HWND owner) {
    _owner = owner;
    _pendingPath = prefs::getString(L"PendingUpdatePath", L"");
    if (!_pendingPath.empty() && GetFileAttributesW(_pendingPath.c_str()) == INVALID_FILE_ATTRIBUTES) forgetPending();
    SetTimer(owner, TIMER_UPDATE, 60 * 60 * 1000, timerProc);                  // hourly: is a daily check due?
    std::thread([] { Sleep(20000); if (prefs::getInt(L"AutoUpdate", 1) && time(nullptr) - prefs::getInt(L"LastUpdateCheck", 0) >= 24 * 3600) check(false); }).detach();
}
void checkNow(HWND owner) { _owner = owner; check(true); }
bool installPendingAndQuit() {
    if (_pendingPath.empty()) return false;
    std::wstring v = prefs::getString(L"PendingUpdateVersion", L"");
    if (compareVersions(v, widen(SEVI_VERSION)) <= 0) { DeleteFileW(_pendingPath.c_str()); forgetPending(); return false; }
    bool ok = runInstaller(_pendingPath, true);
    forgetPending();
    return ok;
}
bool installPendingAtLaunch() {
    std::wstring p = prefs::getString(L"PendingUpdatePath", L""), v = prefs::getString(L"PendingUpdateVersion", L"");
    if (p.empty()) return false;
    if (GetFileAttributesW(p.c_str()) == INVALID_FILE_ATTRIBUTES || compareVersions(v, widen(SEVI_VERSION)) <= 0) { DeleteFileW(p.c_str()); forgetPending(); return false; }
    forgetPending();
    return runInstaller(p, true);
}

}}
