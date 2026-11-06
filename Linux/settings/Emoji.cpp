// The emoji picker on Linux (sevi-settings --emoji), a port of App/Views/EmojiPicker.swift + App/Model/EmojiSearch.swift.
// The IBus engine starts it when the emoji hotkey is pressed: a borderless panel in the middle of the screen.
// Typing searches in English and Vietnamese (and the Telex reading of the keys), the arrows move row by row,
// Enter types the emoji into the app you were in, Esc clears the search and then closes, clicking elsewhere closes.
// The choice goes back through the engine's command file ("emoji 🐴"), and the engine commits it to the app
// once that app has the focus again. The picker reads raw keys (it runs with GTK's simple input method, not IBus).
#include "Common.h"
#include <vector>
#include <string>
#include <map>
#include <algorithm>
#include <fstream>
#include <sstream>
#include <cstring>
#include <cmath>
#include <fontconfig/fontconfig.h>
#ifdef SEVI_HAVE_X11
#include <gdk/x11/gdkx.h>
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/Xatom.h>
#endif
using namespace sevi;
using namespace ui;

namespace {
// ---- Emoji.json ------------------------------------------------------------------------------------------------
struct JV { int t = 0; double n = 0; std::string s; std::vector<JV> a; };   // t: 0 null, 1 number, 2 string, 3 array
void jws(const char*& p) { while (*p == ' ' || *p == '\n' || *p == '\r' || *p == '\t') p++; }
JV jparse(const char*& p) {
    jws(p); JV v;
    if (*p == '[') {
        v.t = 3; p++; jws(p); if (*p == ']') { p++; return v; }
        while (*p) { v.a.push_back(jparse(p)); jws(p); if (*p == ',') { p++; continue; } if (*p == ']') p++; break; }
        return v;
    }
    if (*p == '"') {
        v.t = 2; p++;
        while (*p && *p != '"') {
            if (*p != '\\') { v.s += *p++; continue; }
            p++; char c = *p ? *p++ : 0;
            if (c == 'n') v.s += '\n'; else if (c == 't') v.s += '\t';
            else if (c == 'u') {
                gunichar u = (gunichar)strtoul(std::string(p, 4).c_str(), nullptr, 16); p += 4;
                if (u >= 0xD800 && u < 0xDC00 && p[0] == '\\' && p[1] == 'u') { gunichar l = (gunichar)strtoul(std::string(p + 2, 4).c_str(), nullptr, 16); p += 6; u = 0x10000 + ((u - 0xD800) << 10) + (l - 0xDC00); }
                char b[8]; int n = g_unichar_to_utf8(u, b); v.s.append(b, n);
            } else if (c) v.s += c;
        }
        if (*p == '"') p++;
        return v;
    }
    if (isalpha((unsigned char)*p)) { while (isalpha((unsigned char)*p)) p++; return v; }   // null / true / false
    char* e = nullptr; v.t = 1; v.n = strtod(p, &e); if (e == p) { p++; v.t = 0; } else p = e;
    return v;
}

// ---- search (EmojiSearch.swift) ----------------------------------------------------------------------------------
std::string lower(const std::string& s) { gchar* d = g_utf8_strdown(s.c_str(), -1); std::string r = d; g_free(d); return r; }
/// Lowercase, no accents, đ → d.
std::string fold(const std::string& s) {
    gchar* d = g_utf8_strdown(s.c_str(), -1); gchar* n = g_utf8_normalize(d, -1, G_NORMALIZE_NFD); std::string out;
    for (const gchar* p = n; p && *p; p = g_utf8_next_char(p)) { gunichar c = g_utf8_get_char(p); if (g_unichar_ismark(c)) continue; if (c == 0x0111) c = 'd'; char b[8]; out.append(b, g_unichar_to_utf8(c, b)); }
    g_free(d); g_free(n); return out;
}
std::vector<std::string> words(const std::string& s) { std::vector<std::string> w; std::string cur; for (char c : s) { if (c == ' ') { if (!cur.empty()) w.push_back(cur); cur.clear(); } else cur += c; } if (!cur.empty()) w.push_back(cur); return w; }
std::string trim(const std::string& s) { size_t a = s.find_first_not_of(" \t\n"), b = s.find_last_not_of(" \t\n"); return a == std::string::npos ? "" : s.substr(a, b - a + 1); }
long ulen(const std::string& s) { return g_utf8_strlen(s.c_str(), -1); }

struct Term { std::string folded; std::vector<std::string> words, exact; int weight; };   // weight: name 3, keyword 2
Term term(const std::string& text, int weight) { Term t; t.folded = fold(text); t.words = words(t.folded); t.exact = words(lower(text)); t.weight = weight; return t; }
struct Emoji {
    std::string ch, en, vi; int group = 0; double version = 0; std::vector<std::string> skins; int pop = 0; std::vector<Term> terms;
    std::string withSkin(int t) const { return t > 0 && skins.size() == 5 ? skins[t - 1] : ch; }
};
std::vector<Emoji> _all;

void loadIndex() {
    std::string path = std::string(SEVI_DATADIR) + "/Emoji.json"; if (const char* p = getenv("SEVI_EMOJI_JSON")) path = p;
    std::ifstream f(path); if (!f) return; std::stringstream ss; ss << f.rdbuf(); std::string s = ss.str();
    const char* p = s.c_str(); JV root = jparse(p); if (root.t != 3) return;
    _all.reserve(root.a.size());
    for (auto& r : root.a) {
        if (r.t != 3 || r.a.size() < 8 || r.a[0].t != 2 || r.a[1].t != 1) continue;
        Emoji e; e.ch = r.a[0].s; e.group = (int)r.a[1].n; e.version = r.a[2].n; e.en = r.a[3].s; e.vi = r.a[4].s;
        for (auto& x : {e.en, e.vi}) if (!x.empty()) e.terms.push_back(term(x, 3));
        std::string kw = r.a[5].s + " | " + r.a[6].s; size_t a = 0;
        while (a <= kw.size()) { size_t b = kw.find(" | ", a); if (b == std::string::npos) b = kw.size(); std::string k = kw.substr(a, b - a); if (!k.empty()) e.terms.push_back(term(k, 2)); a = b + 3; }
        if (r.a[7].t == 3) for (auto& x : r.a[7].a) e.skins.push_back(x.s);
        e.pop = r.a.size() > 8 ? (int)r.a[8].n : 0;
        _all.push_back(std::move(e));
    }
}

// A small Telex reader for search text: each word's keys become the Vietnamese letters they type
// ("nguwaj" → "ngựa", "dduwowngf" → "đường").
const std::u32string kVowels = U"aăâeêioôơuưy";
bool isV(char32_t c) { return kVowels.find(c) != std::u32string::npos; }
char32_t toned(char32_t c, int tone) {
    static const std::map<char32_t, std::u32string> M = {{U'a', U"áàảãạ"}, {U'ă', U"ắằẳẵặ"}, {U'â', U"ấầẩẫậ"}, {U'e', U"éèẻẽẹ"}, {U'ê', U"ếềểễệ"}, {U'i', U"íìỉĩị"},
        {U'o', U"óòỏõọ"}, {U'ô', U"ốồổỗộ"}, {U'ơ', U"ớờởỡợ"}, {U'u', U"úùủũụ"}, {U'ư', U"ứừửữự"}, {U'y', U"ýỳỷỹỵ"}};
    auto it = M.find(c); return it == M.end() ? c : it->second[tone - 1];
}
int toneIndex(const std::u32string& w) {   // where the tone mark goes (modern style: hoà, thuý)
    size_t start = 0, n = w.size();
    if (n > 2 && w[0] == U'q' && w[1] == U'u') start = 2;
    else if (n > 2 && w[0] == U'g' && w[1] == U'i' && isV(w[2])) start = 2;
    std::vector<int> idx; for (size_t i = start; i < n; i++) if (isV(w[i])) idx.push_back((int)i);
    if (idx.empty()) return -1;
    int first = idx.front(), lastV = idx.back();
    const std::vector<int>& nucleus = idx;   // as EmojiSearch.swift: every vowel from the first
    for (auto it = nucleus.rbegin(); it != nucleus.rend(); ++it) if (std::u32string(U"ăâêôơư").find(w[*it]) != std::u32string::npos) return *it;
    if (nucleus.size() == 1) return first;
    bool endsInConsonant = lastV < (int)n - 1;
    if (endsInConsonant || nucleus.size() == 3) return nucleus[1];
    std::u32string pair = {w[nucleus[0]], w[nucleus[1]]};
    return (pair == U"oa" || pair == U"oe" || pair == U"uy") ? nucleus[1] : nucleus[0];
}
std::u32string telexWord(const std::u32string& keys) {
    std::u32string out; int tone = 0; char32_t toneKey = 0;
    for (char32_t k0 : keys) {
        char32_t k = g_unichar_tolower(k0); char32_t last = out.empty() ? 0 : out.back();
        if (k == U'd' && last == U'd') out.back() = U'đ';
        else if (k == U'a' && last == U'a') out.back() = U'â';
        else if (k == U'e' && last == U'e') out.back() = U'ê';
        else if (k == U'o' && last == U'o') out.back() = U'ô';
        else if (k == U'w') {   // ươ (uow / uwow), ă, ơ, ư; a lone w is ư
            int i = -1; for (int j = (int)out.size() - 1; j >= 0; j--) if (out[j] == U'a' || out[j] == U'o' || out[j] == U'u') { i = j; break; }
            bool ok = i >= 0; if (ok && i != (int)out.size() - 1) for (size_t j = i + 1; j < out.size(); j++) if (isV(out[j])) { ok = false; break; }
            if (ok) {
                char32_t c = out[i]; out[i] = c == U'a' ? U'ă' : c == U'o' ? U'ơ' : U'ư';
                if (c == U'o' && i > 0 && out[i - 1] == U'u') out[i - 1] = U'ư';
                if (c == U'u' && i + 1 < (int)out.size() && out[i + 1] == U'o') out[i + 1] = U'ơ';
            } else out += U'ư';
        } else if (k == U'z') { tone = 0; toneKey = 0; }
        else {
            int t = k == U's' ? 1 : k == U'f' ? 2 : k == U'r' ? 3 : k == U'x' ? 4 : k == U'j' ? 5 : 0;
            bool hasV = false; for (char32_t c : out) if (isV(c)) { hasV = true; break; }
            if (t && hasV) { if (toneKey == k) { tone = 0; toneKey = 0; out += k; } else { tone = t; toneKey = k; } }   // the same tone key twice: undo, keep the letter
            else out += k;
        }
    }
    if (tone == 0) return out;
    int i = toneIndex(out); if (i < 0) return out;
    out[i] = toned(out[i], tone); return out;
}
std::u32string u32(const std::string& s) { std::u32string o; for (const gchar* p = s.c_str(); *p; p = g_utf8_next_char(p)) o += (char32_t)g_utf8_get_char(p); return o; }
std::string u8(const std::u32string& s) { std::string o; char b[8]; for (char32_t c : s) o.append(b, g_unichar_to_utf8(c, b)); return o; }
std::string telex(const std::string& s) {   // word by word, spaces kept
    std::u32string in = u32(s), out, w;
    for (char32_t c : in) { if (c == U' ') { out += telexWord(w); out += U' '; w.clear(); } else w += c; }
    out += telexWord(w); return u8(out);
}

int score(const std::vector<std::pair<std::string, std::string>>& ws, const std::vector<Term>& terms) {
    int total = 0; std::string whole; for (auto& w : ws) { if (!whole.empty()) whole += ' '; whole += w.second; }
    for (auto& w : ws) {
        int best = 0; bool accented = w.first != w.second; long wl = ulen(w.second);
        for (auto& t : terms) {
            bool name = t.weight == 3; int s = 0;
            if (t.folded == whole) s = name ? 100 : 65;
            else for (auto& tw : t.words) if (tw.compare(0, w.second.size(), w.second) == 0) {
                bool same = ulen(tw) == wl; s = std::max(s, same ? (name ? 70 : 55) : (name ? 45 : 30)); if (same) break;
            }
            if (s <= 0) continue;
            if (std::find(t.exact.begin(), t.exact.end(), w.first) != t.exact.end()) s += 10; else if (accented) s -= 40;
            if (name) s += std::max(0, 6 - (int)t.words.size());   // short names are about the word: ❤️ "trái tim màu đỏ" over 🥰
            best = std::max(best, s);
        }
        if (best <= 0) return 0;
        total += best;
    }
    return total / (int)ws.size();
}
/// Best matches first. `raw` is the keys as typed; the Telex reading of the same keys is searched too.
std::vector<const Emoji*> search(const std::string& raw, const std::vector<const Emoji*>& among, size_t limit) {
    std::string q = trim(lower(raw)); if (q.empty()) return {};
    std::vector<std::string> variants = {q}; std::string t = telex(q); if (t != q) variants.push_back(t);
    std::vector<std::vector<std::pair<std::string, std::string>>> queries;
    for (auto& v : variants) { std::vector<std::pair<std::string, std::string>> ws; for (auto& w : words(v)) ws.push_back({w, fold(w)}); queries.push_back(ws); }
    struct S { int score, i; const Emoji* e; }; std::vector<S> scored;
    for (size_t i = 0; i < among.size(); i++) { int best = 0; for (auto& qw : queries) best = std::max(best, score(qw, among[i]->terms)); if (best > 0) scored.push_back({best * 2 + among[i]->pop, (int)i, among[i]}); }
    std::sort(scored.begin(), scored.end(), [](const S& a, const S& b) { return a.score != b.score ? a.score > b.score : a.i < b.i; });
    std::vector<const Emoji*> out; for (size_t i = 0; i < scored.size() && i < limit; i++) out.push_back(scored[i].e); return out;
}

// ---- Frequently used (EmojiUsage): each pick adds 1 and every older score fades a little -----------------------------
const char* kStarter[] = {"😂", "❤️", "🥰", "😍", "😭", "👍", "🙏", "😊", "🔥", "😘", "🤣", "🥺", "😅", "🎉", "💕", "😁", "🙂", "😉",
                          "🤔", "👏", "😎", "💪", "✨", "😢", "🙌", "💯", "😆", "🤗", "😴", "👌"};
std::map<std::string, double> usage() {   // "EmojiRecents": "😂=3.2|❤️=1"
    std::map<std::string, double> m; std::string s = prefs::getString("EmojiRecents", ""); size_t a = 0;
    while (a < s.size()) { size_t b = s.find('|', a); if (b == std::string::npos) b = s.size(); std::string it = s.substr(a, b - a); size_t eq = it.rfind('='); if (eq != std::string::npos && eq > 0) m[it.substr(0, eq)] = atof(it.c_str() + eq + 1); a = b + 1; }
    return m;
}
void recordUsage(const std::string& ch) {
    auto m = usage(); for (auto& kv : m) kv.second *= 0.97; m[ch] += 1;
    if (m.size() > 80) { std::vector<std::pair<std::string, double>> v(m.begin(), m.end()); std::sort(v.begin(), v.end(), [](auto& a, auto& b) { return a.second < b.second; }); for (size_t i = 0; i < m.size() - 80; i++) m.erase(v[i].first); }
    std::string s; char b[32]; for (auto& kv : m) { snprintf(b, sizeof b, "%.4f", kv.second); if (!s.empty()) s += '|'; s += kv.first + "=" + b; }
    prefs::setString("EmojiRecents", s);
}
std::vector<std::string> frequent(size_t n) {
    auto m = usage(); std::vector<std::pair<std::string, double>> v(m.begin(), m.end());
    std::sort(v.begin(), v.end(), [](auto& a, auto& b) { return a.second != b.second ? a.second > b.second : a.first < b.first; });
    std::vector<std::string> list; for (auto& kv : v) list.push_back(kv.first);
    for (auto s : kStarter) if (list.size() < n && std::find(list.begin(), list.end(), s) == list.end()) list.push_back(s);
    if (list.size() > n) list.resize(n); return list;
}

/// The newest emoji version the system's colour emoji font draws (newer ones would show as boxes).
double maxVersion() {
    double v = 14;
    FcPattern* pat = FcNameParse((const FcChar8*)"emoji:color"); FcConfigSubstitute(nullptr, pat, FcMatchPattern); FcDefaultSubstitute(pat);
    FcResult res; FcPattern* m = FcFontMatch(nullptr, pat, &res); FcCharSet* cs = nullptr;
    if (m && FcPatternGetCharSet(m, FC_CHARSET, 0, &cs) == FcResultMatch && cs) {
        if (FcCharSetHasChar(cs, 0x1FAEA)) v = 17;          // distorted face (Emoji 17.0)
        else if (FcCharSetHasChar(cs, 0x1FAE9)) v = 16;     // face with bags under eyes (16.0)
        else if (FcCharSetHasChar(cs, 0x1FAE8)) v = 15.1;   // shaking face (15.0; fonts with it ship 15.1's sequences too)
    }
    if (m) FcPatternDestroy(m); FcPatternDestroy(pat); return v;
}

// ---- the picker ------------------------------------------------------------------------------------------------
const int COLS = 9, CELL = 44, PAD = 12, WIDTH = COLS * CELL + 24, HEIGHT = 404, SHADOW = 20;
const char* kGroupVI[] = {"Mặt cười & cảm xúc", "Người & cơ thể", "", "Động vật & thiên nhiên", "Đồ ăn & thức uống", "Du lịch & địa điểm", "Hoạt động", "Đồ vật", "Biểu tượng", "Cờ"};
const char* kGroupEN[] = {"Smileys & Emotion", "People & Body", "", "Animals & Nature", "Food & Drink", "Travel & Places", "Activities", "Objects", "Symbols", "Flags"};
const char* kTones[] = {"✋", "✋🏻", "✋🏼", "✋🏽", "✋🏾", "✋🏿"};
struct Item { bool header; std::string title; int y, h, start, count; };
struct Picker {
    GtkAdjustment* adj = nullptr;   // the grid scrolls by hand: one drawing area the size of the view (a 10k px tall widget is over GPU texture limits)
    GtkApplication* app; GtkWidget *win, *area, *scroll, *display, *caret, *placeholder, *badge, *curEmoji, *curName, *toneBtn, *tonePop, *empty;
    std::string raw; bool allSelected = false, vietnamese = true, caretOn = true, wasActive = false, firstMotion = true, closing = false;
    int tone = 0, selected = 0; double lastX = -1, lastY = -1;
    std::vector<const Emoji*> all, recents, results, visible; std::vector<std::pair<int, std::vector<const Emoji*>>> groups;
    std::vector<Item> items; int contentH = 0;
} P;

bool searching() { return !trim(P.raw).empty(); }
const Emoji* current() { return P.selected >= 0 && P.selected < (int)P.visible.size() ? P.visible[P.selected] : nullptr; }
void relayout() {
    P.visible.clear(); P.items.clear();
    int y = 0, start = 0;
    auto add = [&](const std::string& title, int n) {
        if (!title.empty()) { int h = (P.items.empty() ? 0 : 8) + 14 + 6; P.items.push_back({true, title, y, h, 0, 0}); y += h; }
        for (int i = 0; i < n; i += COLS) { P.items.push_back({false, "", y, CELL, start + i, std::min(COLS, n - i)}); y += CELL; }
        start += n;
    };
    if (searching()) { P.visible = P.results; add("", (int)P.results.size()); }
    else {
        P.visible = P.recents; if (!P.recents.empty()) add(P.vietnamese ? "Dùng thường xuyên" : "Frequently Used", (int)P.recents.size());
        for (auto& g : P.groups) { P.visible.insert(P.visible.end(), g.second.begin(), g.second.end()); add(P.vietnamese ? kGroupVI[g.first] : kGroupEN[g.first], (int)g.second.size()); }
    }
    P.contentH = y; gtk_adjustment_set_upper(P.adj, std::max(y, 1));
    gtk_widget_set_visible(P.scroll, y > gtk_adjustment_get_page_size(P.adj));   // no scroll bar for a short list
    gtk_widget_set_visible(P.empty, searching() && P.results.empty());
}
const Item* rowOf(int i) { for (auto& it : P.items) if (!it.header && i >= it.start && i < it.start + it.count) return &it; return nullptr; }
GtkAdjustment* vadj() { return P.adj; }
void scrollTop() { gtk_adjustment_set_value(vadj(), 0); }
void scrollToSelected() {
    const Item* r = rowOf(P.selected); if (!r) return;
    GtkAdjustment* a = vadj(); double v = gtk_adjustment_get_value(a), page = gtk_adjustment_get_page_size(a);
    bool firstRow = false; for (auto& it : P.items) if (!it.header) { firstRow = &it == r; break; }
    if (firstRow) { gtk_adjustment_set_value(a, 0); return; }   // back on top: the title shows too
    if (r->y < v) gtk_adjustment_set_value(a, r->y); else if (r->y + r->h > v + page) gtk_adjustment_set_value(a, r->y + r->h - page);   // the least scrolling that shows it
}
void updateTexts() {
    std::string d = P.vietnamese ? telex(P.raw) : P.raw;
    gtk_label_set_text(GTK_LABEL(P.display), d.c_str());
    if (P.allSelected && !P.raw.empty()) gtk_widget_add_css_class(P.display, "emoji-allsel"); else gtk_widget_remove_css_class(P.display, "emoji-allsel");
    gtk_widget_set_visible(P.placeholder, P.raw.empty());
    gtk_label_set_text(GTK_LABEL(P.placeholder), P.vietnamese ? "Tìm emoji: ngựa, horse…" : "Search emoji: horse, ngựa…");
    gtk_label_set_text(GTK_LABEL(P.badge), P.vietnamese ? "VI" : "EN");
    gtk_label_set_text(GTK_LABEL(P.empty), P.vietnamese ? "Không tìm thấy emoji nào" : "No emoji found");
    const Emoji* e = current();
    gtk_label_set_text(GTK_LABEL(P.curEmoji), e ? e->withSkin(P.tone).c_str() : "");
    gtk_label_set_text(GTK_LABEL(P.curName), e ? (P.vietnamese ? e->vi : e->en).c_str() : "");
    gtk_menu_button_set_label(GTK_MENU_BUTTON(P.toneBtn), kTones[P.tone]);
    gtk_widget_set_tooltip_text(P.toneBtn, P.vietnamese ? "Màu da" : "Skin tone");
    gtk_widget_set_opacity(P.caret, P.caretOn && !P.allSelected ? 1 : 0);
    gtk_widget_queue_draw(P.area);
}
void doSearch() { P.results = searching() ? search(P.raw, P.all, 400) : std::vector<const Emoji*>{}; P.selected = 0; relayout(); scrollTop(); updateTexts(); }
void setRaw(const std::string& s) { P.raw = s; P.allSelected = false; doSearch(); }
void typeText(const std::string& s) { setRaw(P.allSelected ? s : P.raw + s); }
void deleteBack() { if (P.raw.empty()) return; const gchar* last = g_utf8_find_prev_char(P.raw.c_str(), P.raw.c_str() + P.raw.size()); P.raw.resize(last ? last - P.raw.c_str() : 0); P.allSelected = false; doSearch(); }
void deleteWord() { std::string r = P.raw; while (!r.empty() && r.back() == ' ') r.pop_back(); while (!r.empty() && r.back() != ' ') r.pop_back(); setRaw(r); }
void move(int d) { int n = (int)P.visible.size(); if (!n) return; P.selected = std::min(std::max(0, P.selected + d), n - 1); scrollToSelected(); updateTexts(); }
void moveRow(int d) {   // ↑/↓: the row above/below, same column (or its last emoji)
    std::vector<const Item*> rows; for (auto& it : P.items) if (!it.header) rows.push_back(&it);
    int r = -1; for (size_t i = 0; i < rows.size(); i++) if (P.selected >= rows[i]->start && P.selected < rows[i]->start + rows[i]->count) r = (int)i;
    int t = r + d; if (r < 0 || t < 0 || t >= (int)rows.size()) return;
    P.selected = rows[t]->start + std::min(P.selected - rows[r]->start, rows[t]->count - 1); scrollToSelected(); updateTexts();
}
void closePicker() { if (P.closing) return; P.closing = true; g_application_quit(G_APPLICATION(P.app)); }
/// Types the emoji into the app that was in front, after the panel is gone (the engine commits it at its focus_in).
void choose(const Emoji* e) {
    if (!e) return;
    std::string text = e->withSkin(P.tone); recordUsage(e->ch);
    prefs::sendCommand("emoji " + text);
    gtk_widget_set_visible(P.win, FALSE); closePicker();
}
bool hotkeyIs(int hk, int key, unsigned mods) { return hk && (hk & 0xFF) == key && (unsigned)(hk & 0xF00) == mods; }

gboolean onKey(GtkEventControllerKey*, guint kv, guint, GdkModifierType state, gpointer) {
    unsigned mods = modsOf(state); bool ctrl = state & GDK_CONTROL_MASK, alt = state & GDK_ALT_MASK, super = state & GDK_SUPER_MASK;
    int key = engineKeyOfKeyval(kv);
    if (key >= 0 && hotkeyIs(prefs::getInt("EmojiHotkey", 0), key, mods)) { closePicker(); return TRUE; }   // the hotkey again closes it
    if (key >= 0 && hotkeyIs(currentHotkey() & ~0x8000, key, mods)) {   // the switch hotkey still switches (the engine can't see keys while the picker has the focus)
        P.vietnamese = !P.vietnamese; prefs::sendCommand(P.vietnamese ? "lang vi user" : "lang en user"); relayout(); updateTexts(); return TRUE;
    }
    switch (kv) {
    case GDK_KEY_Escape: if (P.raw.empty()) closePicker(); else setRaw(""); return TRUE;   // Esc: clears the search, then closes
    case GDK_KEY_Return: case GDK_KEY_KP_Enter: choose(current()); return TRUE;
    case GDK_KEY_Left: P.allSelected = false; move(-1); return TRUE;   // the first emoji is selected from the start; the arrows move from there
    case GDK_KEY_Right: P.allSelected = false; move(1); return TRUE;
    case GDK_KEY_Down: P.allSelected = false; moveRow(1); return TRUE;
    case GDK_KEY_Up: P.allSelected = false; moveRow(-1); return TRUE;
    case GDK_KEY_BackSpace: if (P.allSelected) setRaw(""); else if (ctrl || alt) deleteWord(); else deleteBack(); return TRUE;
    }
    if (ctrl) {
        switch (gdk_keyval_to_lower(kv)) {
        case GDK_KEY_v: gdk_clipboard_read_text_async(gtk_widget_get_clipboard(P.win), nullptr, [](GObject* src, GAsyncResult* res, gpointer) { char* t = gdk_clipboard_read_text_finish(GDK_CLIPBOARD(src), res, nullptr); if (t) { std::string s = t; for (auto& c : s) if (c == '\n' || c == '\t') c = ' '; typeText(s); g_free(t); } }, nullptr); break;
        case GDK_KEY_a: if (!P.raw.empty()) { P.allSelected = true; updateTexts(); } break;
        case GDK_KEY_c: case GDK_KEY_x:
            if (P.allSelected && !P.raw.empty()) { gdk_clipboard_set_text(gtk_widget_get_clipboard(P.win), (P.vietnamese ? telex(P.raw) : P.raw).c_str()); if (gdk_keyval_to_lower(kv) == GDK_KEY_x) setRaw(""); }
            break;
        case GDK_KEY_w: closePicker(); break;
        }
        return TRUE;
    }
    if (alt || super) return TRUE;
    gunichar c = gdk_keyval_to_unicode(kv);
    if (c >= 0x20 && c != 0x7F) { char b[8]; typeText(std::string(b, g_unichar_to_utf8(c, b))); }
    return TRUE;
}

int indexAt(double x, double y) {
    for (auto& it : P.items) if (!it.header && y >= it.y && y < it.y + it.h) { int col = (int)floor((x - PAD) / CELL); if (col >= 0 && col < it.count) return it.start + col; return -1; }
    return -1;
}
void draw(GtkDrawingArea*, cairo_t* cr, int w, int, gpointer) {
    GdkRGBA fg; gtk_widget_get_color(P.area, &fg);
    bool dark = adw_style_manager_get_dark(adw_style_manager_get_default());
    double v = gtk_adjustment_get_value(vadj()), page = gtk_adjustment_get_page_size(vadj());
    cairo_translate(cr, 0, -v);
    PangoLayout* l = gtk_widget_create_pango_layout(P.area, nullptr);
    PangoFontDescription* ef = pango_font_description_new(); pango_font_description_set_absolute_size(ef, 28 * PANGO_SCALE);
    PangoFontDescription* hf = pango_font_description_copy(pango_context_get_font_description(gtk_widget_get_pango_context(P.area)));
    pango_font_description_set_absolute_size(hf, 11 * PANGO_SCALE); pango_font_description_set_weight(hf, PANGO_WEIGHT_SEMIBOLD);
    for (auto& it : P.items) {
        if (it.y + it.h < v || it.y > v + page) continue;
        if (it.header) {
            pango_layout_set_font_description(l, hf); pango_layout_set_text(l, it.title.c_str(), -1);
            cairo_set_source_rgba(cr, fg.red, fg.green, fg.blue, 0.55); cairo_move_to(cr, PAD, it.y + (it.h - 6 - 14)); pango_cairo_show_layout(cr, l);
            continue;
        }
        double cw = (double)(w - 2 * PAD) / COLS;
        for (int c = 0; c < it.count; c++) {
            int idx = it.start + c; if (idx >= (int)P.visible.size()) break;
            double x = PAD + c * cw;
            if (idx == P.selected) {
                if (dark) cairo_set_source_rgba(cr, 0.549, 0.529, 1, 0.22); else cairo_set_source_rgba(cr, 0.310, 0.275, 0.898, 0.22);
                double r = 8, x0 = x, y0 = it.y, x1 = x + cw, y1 = it.y + CELL;
                cairo_new_sub_path(cr); cairo_arc(cr, x1 - r, y0 + r, r, -M_PI / 2, 0); cairo_arc(cr, x1 - r, y1 - r, r, 0, M_PI / 2); cairo_arc(cr, x0 + r, y1 - r, r, M_PI / 2, M_PI); cairo_arc(cr, x0 + r, y0 + r, r, M_PI, 3 * M_PI / 2); cairo_close_path(cr); cairo_fill(cr);
            }
            pango_layout_set_font_description(l, ef); pango_layout_set_text(l, P.visible[idx]->withSkin(P.tone).c_str(), -1);
            int lw, lh; pango_layout_get_pixel_size(l, &lw, &lh);
            cairo_set_source_rgba(cr, fg.red, fg.green, fg.blue, 1); cairo_move_to(cr, x + (cw - lw) / 2, it.y + (CELL - lh) / 2.0); pango_cairo_show_layout(cr, l);
        }
    }
    pango_font_description_free(ef); pango_font_description_free(hf); g_object_unref(l);
}

#ifdef SEVI_HAVE_X11
/// Centre on the screen with the pointer (a bit above the middle, like the Mac panel), above other windows, not in the dock.
void placeX11(GtkWidget* w) {
    GdkSurface* s = gtk_native_get_surface(GTK_NATIVE(w)); if (!GDK_IS_X11_SURFACE(s)) return;
    Display* d = gdk_x11_display_get_xdisplay(gdk_surface_get_display(s)); Window xw = gdk_x11_surface_get_xid(s);
    int pw = WIDTH + 2 * SHADOW, ph = HEIGHT + 2 * SHADOW;
    Window rr, cr; int px = 0, py = 0, wx, wy; unsigned mask; XQueryPointer(d, DefaultRootWindow(d), &rr, &cr, &px, &py, &wx, &wy, &mask);
    GdkRectangle g = {0, 0, DisplayWidth(d, DefaultScreen(d)), DisplayHeight(d, DefaultScreen(d))};
    GListModel* mons = gdk_display_get_monitors(gdk_surface_get_display(s));
    for (guint i = 0; i < g_list_model_get_n_items(mons); i++) { GdkMonitor* m = (GdkMonitor*)g_list_model_get_item(mons, i); GdkRectangle r; gdk_monitor_get_geometry(m, &r); g_object_unref(m); int f = gdk_surface_get_scale_factor(s); if (i == 0 || (px / f >= r.x && px / f < r.x + r.width && py / f >= r.y && py / f < r.y + r.height)) g = r; }
    int sc = gdk_surface_get_scale_factor(s);   // GDK sizes are scaled; X positions are device pixels
    int x = (g.x + (g.width - pw) / 2) * sc, y = (g.y + (g.height - ph) / 2 - (int)(g.height * 0.08)) * sc;
    XMoveWindow(d, xw, x, y);
    // above other windows, not in the dock (GTK rewrites the property at map: ask the window manager instead)
    Atom st = XInternAtom(d, "_NET_WM_STATE", False);
    for (const char* a : {"_NET_WM_STATE_ABOVE", "_NET_WM_STATE_SKIP_TASKBAR", "_NET_WM_STATE_SKIP_PAGER"}) {
        XEvent e = {}; e.xclient.type = ClientMessage; e.xclient.window = xw; e.xclient.message_type = st; e.xclient.format = 32;
        e.xclient.data.l[0] = 1; e.xclient.data.l[1] = (long)XInternAtom(d, a, False); e.xclient.data.l[3] = 2;
        XSendEvent(d, DefaultRootWindow(d), False, SubstructureRedirectMask | SubstructureNotifyMask, &e);
    }
    XFlush(d);
}
void activateX11(GtkWidget* w) {   // started by the engine with no startup id: ask the window manager for the focus, as a pager would
    GdkSurface* s = gtk_native_get_surface(GTK_NATIVE(w)); if (!GDK_IS_X11_SURFACE(s)) return;
    Display* d = gdk_x11_display_get_xdisplay(gdk_surface_get_display(s)); Window xw = gdk_x11_surface_get_xid(s);
    XEvent e = {}; e.xclient.type = ClientMessage; e.xclient.window = xw; e.xclient.message_type = XInternAtom(d, "_NET_ACTIVE_WINDOW", False); e.xclient.format = 32;
    e.xclient.data.l[0] = 2; e.xclient.data.l[1] = CurrentTime;
    XSendEvent(d, DefaultRootWindow(d), False, SubstructureRedirectMask | SubstructureNotifyMask, &e); XFlush(d);
}
#endif
GtkWidget* lbl(const char* t, const char* cls = nullptr) { GtkWidget* l = gtk_label_new(t); gtk_label_set_xalign(GTK_LABEL(l), 0); if (cls) gtk_widget_add_css_class(l, cls); return l; }
}  // namespace

void buildEmoji(GtkApplication* app) {
    P.app = app;
    loadIndex();
    double maxV = maxVersion();
    for (auto& e : _all) if (e.version <= maxV) P.all.push_back(&e);
    std::map<int, std::vector<const Emoji*>> g; for (auto e : P.all) g[e->group].push_back(e);
    for (auto& kv : g) P.groups.push_back(kv);
    std::map<std::string, const Emoji*> byChar; for (auto e : P.all) byChar.emplace(e->ch, e);
    for (auto& c : frequent(COLS * 3)) { auto it = byChar.find(c); if (it != byChar.end()) P.recents.push_back(it->second); }
    P.vietnamese = prefs::getInt("InputMethod", 1) != 0; P.tone = std::min(std::max(prefs::getInt("EmojiSkinTone", 0), 0), 5);

    GtkCssProvider* css = gtk_css_provider_new();
    gtk_css_provider_load_from_string(css,
        "window.sevi-emoji { background: transparent; }\n"
        ".emoji-panel { background: @popover_bg_color; color: @popover_fg_color; border-radius: 14px; margin: 20px;"
        "  box-shadow: 0 0 0 1px alpha(black, 0.10), 0 2px 4px alpha(black, 0.12), 0 8px 24px alpha(black, 0.22); }\n"
        ".emoji-search { font-size: 17px; }\n"
        ".emoji-caret { background: #4F46E5; min-width: 2px; min-height: 18px; }\n"
        ".emoji-badge { font-size: 11px; font-weight: 600; padding: 2px 6px; border-radius: 5px; background: alpha(currentColor, 0.10); }\n"
        ".emoji-allsel { background: alpha(@accent_bg_color, 0.30); border-radius: 3px; }\n"
        ".emoji-cur { font-size: 22px; } .emoji-sep { opacity: 0.5; }\n"
        "menubutton.emoji-tone > button { font-size: 18px; padding: 0 4px; min-height: 26px; }\n"
        "button.emoji-tone-item { font-size: 20px; padding: 2px 6px; }\n");
    gtk_style_context_add_provider_for_display(gdk_display_get_default(), GTK_STYLE_PROVIDER(css), GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);

    P.win = gtk_window_new(); gtk_window_set_application(GTK_WINDOW(P.win), app); gtk_window_set_title(GTK_WINDOW(P.win), "Sevi Emoji");
    gtk_window_set_decorated(GTK_WINDOW(P.win), FALSE); gtk_window_set_resizable(GTK_WINDOW(P.win), FALSE);
    gtk_widget_remove_css_class(P.win, "background"); gtk_widget_add_css_class(P.win, "sevi-emoji");
    gtk_window_set_default_size(GTK_WINDOW(P.win), WIDTH + 2 * SHADOW, HEIGHT + 2 * SHADOW);
    GtkWidget* panel = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0); gtk_widget_add_css_class(panel, "emoji-panel"); gtk_widget_set_size_request(panel, WIDTH, HEIGHT); gtk_widget_set_overflow(panel, GTK_OVERFLOW_HIDDEN);
    // search field
    GtkWidget* sf = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8); for (auto f : {gtk_widget_set_margin_start, gtk_widget_set_margin_end, gtk_widget_set_margin_top}) f(sf, 14); gtk_widget_set_margin_bottom(sf, 8);
    GtkWidget* mag = gtk_image_new_from_icon_name("system-search-symbolic"); gtk_widget_add_css_class(mag, "dim-label"); gtk_box_append(GTK_BOX(sf), mag);
    GtkWidget* tb = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 1); gtk_widget_add_css_class(tb, "emoji-search"); gtk_widget_set_hexpand(tb, TRUE);
    P.display = lbl(""); gtk_label_set_ellipsize(GTK_LABEL(P.display), PANGO_ELLIPSIZE_START); gtk_box_append(GTK_BOX(tb), P.display);
    P.caret = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 0); gtk_widget_add_css_class(P.caret, "emoji-caret"); gtk_widget_set_valign(P.caret, GTK_ALIGN_CENTER); gtk_box_append(GTK_BOX(tb), P.caret);
    P.placeholder = lbl("", "dim-label"); gtk_widget_set_margin_start(P.placeholder, 2); gtk_box_append(GTK_BOX(tb), P.placeholder);
    gtk_box_append(GTK_BOX(sf), tb);
    P.badge = lbl("", "emoji-badge"); gtk_widget_set_valign(P.badge, GTK_ALIGN_CENTER); gtk_box_append(GTK_BOX(sf), P.badge);
    gtk_box_append(GTK_BOX(panel), sf);
    GtkWidget* s1 = gtk_separator_new(GTK_ORIENTATION_HORIZONTAL); gtk_widget_add_css_class(s1, "emoji-sep"); gtk_box_append(GTK_BOX(panel), s1);
    // grid: one drawing area (draws only the rows in view), inset so there is a gap under the divider at any scroll
    GtkWidget* overlay = gtk_overlay_new(); gtk_widget_set_vexpand(overlay, TRUE); gtk_widget_set_margin_top(overlay, 8); gtk_widget_set_margin_bottom(overlay, 8);
    P.adj = gtk_adjustment_new(0, 0, 1, CELL, CELL * 4, 1);
    P.area = gtk_drawing_area_new(); gtk_drawing_area_set_draw_func(GTK_DRAWING_AREA(P.area), draw, nullptr, nullptr); gtk_widget_set_vexpand(P.area, TRUE);
    g_signal_connect(P.area, "resize", G_CALLBACK(+[](GtkDrawingArea*, int, int h, gpointer) { gtk_adjustment_set_page_size(P.adj, h); gtk_adjustment_set_page_increment(P.adj, h); gtk_widget_set_visible(P.scroll, P.contentH > h); gtk_adjustment_set_value(P.adj, gtk_adjustment_get_value(P.adj)); }), nullptr);
    gtk_overlay_set_child(GTK_OVERLAY(overlay), P.area);
    P.scroll = gtk_scrollbar_new(GTK_ORIENTATION_VERTICAL, P.adj); gtk_widget_set_halign(P.scroll, GTK_ALIGN_END); gtk_widget_add_css_class(P.scroll, "overlay-indicator"); gtk_overlay_add_overlay(GTK_OVERLAY(overlay), P.scroll);
    GtkEventController* sc = gtk_event_controller_scroll_new((GtkEventControllerScrollFlags)(GTK_EVENT_CONTROLLER_SCROLL_VERTICAL | GTK_EVENT_CONTROLLER_SCROLL_KINETIC));
    g_signal_connect(sc, "scroll", G_CALLBACK(+[](GtkEventControllerScroll* c, double, double dy, gpointer) -> gboolean {
        double step = gtk_event_controller_scroll_get_unit(c) == GDK_SCROLL_UNIT_WHEEL ? CELL * 1.5 : 1.0;
        gtk_adjustment_set_value(P.adj, gtk_adjustment_get_value(P.adj) + dy * step); return TRUE; }), nullptr);
    gtk_widget_add_controller(P.area, sc);
    P.empty = gtk_label_new(""); gtk_widget_add_css_class(P.empty, "dim-label"); gtk_widget_set_valign(P.empty, GTK_ALIGN_START); gtk_widget_set_margin_top(P.empty, 110); gtk_overlay_add_overlay(GTK_OVERLAY(overlay), P.empty);
    g_signal_connect_swapped(vadj(), "value-changed", G_CALLBACK(gtk_widget_queue_draw), P.area);
    gtk_box_append(GTK_BOX(panel), overlay);
    GtkWidget* s2 = gtk_separator_new(GTK_ORIENTATION_HORIZONTAL); gtk_widget_add_css_class(s2, "emoji-sep"); gtk_box_append(GTK_BOX(panel), s2);
    // footer: the emoji in focus + its name · Sevi · skin tone
    GtkWidget* ft = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 10); gtk_widget_set_size_request(ft, -1, 44); gtk_widget_set_margin_start(ft, 14); gtk_widget_set_margin_end(ft, 14);
    P.curEmoji = lbl("", "emoji-cur"); gtk_widget_set_valign(P.curEmoji, GTK_ALIGN_CENTER); gtk_box_append(GTK_BOX(ft), P.curEmoji);
    P.curName = lbl("", "dim-label"); gtk_label_set_ellipsize(GTK_LABEL(P.curName), PANGO_ELLIPSIZE_END); gtk_widget_set_hexpand(P.curName, TRUE); gtk_widget_set_valign(P.curName, GTK_ALIGN_CENTER); gtk_box_append(GTK_BOX(ft), P.curName);
    GtkWidget* brand = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 5); gtk_widget_set_opacity(brand, 0.85); gtk_widget_set_valign(brand, GTK_ALIGN_CENTER);
    GtkWidget* bi = gtk_image_new_from_icon_name("sevi"); gtk_image_set_pixel_size(GTK_IMAGE(bi), 16); gtk_box_append(GTK_BOX(brand), bi); gtk_box_append(GTK_BOX(brand), wordmark(13));
    gtk_box_append(GTK_BOX(ft), brand);
    GtkWidget* vs = gtk_separator_new(GTK_ORIENTATION_VERTICAL); gtk_widget_set_size_request(vs, -1, 16); gtk_widget_set_valign(vs, GTK_ALIGN_CENTER); gtk_box_append(GTK_BOX(ft), vs);
    P.toneBtn = gtk_menu_button_new(); gtk_menu_button_set_has_frame(GTK_MENU_BUTTON(P.toneBtn), FALSE); gtk_menu_button_set_always_show_arrow(GTK_MENU_BUTTON(P.toneBtn), FALSE); gtk_widget_add_css_class(P.toneBtn, "emoji-tone"); gtk_widget_set_valign(P.toneBtn, GTK_ALIGN_CENTER);
    P.tonePop = gtk_popover_new(); GtkWidget* tones = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 2);
    for (int t = 0; t < 6; t++) {
        GtkWidget* b = gtk_button_new_with_label(kTones[t]); gtk_button_set_has_frame(GTK_BUTTON(b), FALSE); gtk_widget_add_css_class(b, "emoji-tone-item");
        g_signal_connect(b, "clicked", G_CALLBACK(+[](GtkButton*, gpointer p) { P.tone = GPOINTER_TO_INT(p); prefs::setInt("EmojiSkinTone", P.tone); gtk_popover_popdown(GTK_POPOVER(P.tonePop)); updateTexts(); }), GINT_TO_POINTER(t));
        gtk_box_append(GTK_BOX(tones), b);
    }
    gtk_popover_set_child(GTK_POPOVER(P.tonePop), tones); gtk_menu_button_set_popover(GTK_MENU_BUTTON(P.toneBtn), P.tonePop);
    gtk_box_append(GTK_BOX(ft), P.toneBtn);
    gtk_box_append(GTK_BOX(panel), ft);
    gtk_window_set_child(GTK_WINDOW(P.win), panel);

    // input: keys on the whole window (raw keys, no text widget), hover selects (only when the mouse really moved), click chooses
    GtkEventController* kc = gtk_event_controller_key_new(); gtk_event_controller_set_propagation_phase(kc, GTK_PHASE_CAPTURE);
    g_signal_connect(kc, "key-pressed", G_CALLBACK(onKey), nullptr); gtk_widget_add_controller(P.win, kc);
    GtkEventController* mc = gtk_event_controller_motion_new();
    g_signal_connect(mc, "motion", G_CALLBACK(+[](GtkEventControllerMotion*, double x, double y, gpointer) {
        double vy = y;   // the view's own coordinates: the grid scrolling under a still mouse is not a move
        if (P.firstMotion) { P.firstMotion = false; P.lastX = x; P.lastY = vy; return; }
        if (x == P.lastX && vy == P.lastY) return;
        P.lastX = x; P.lastY = vy; int i = indexAt(x, y + gtk_adjustment_get_value(vadj())); if (i >= 0 && i != P.selected) { P.selected = i; updateTexts(); } }), nullptr);
    gtk_widget_add_controller(P.area, mc);
    GtkGesture* click = gtk_gesture_click_new();
    g_signal_connect(click, "released", G_CALLBACK(+[](GtkGestureClick*, int, double x, double y, gpointer) { int i = indexAt(x, y + gtk_adjustment_get_value(vadj())); if (i >= 0) choose(P.visible[i]); }), nullptr);
    gtk_widget_add_controller(P.area, GTK_EVENT_CONTROLLER(click));
    // clicked elsewhere: close (not while the skin tone popover has the pointer)
    g_signal_connect(P.win, "notify::is-active", G_CALLBACK(+[](GObject*, GParamSpec*, gpointer) {
        bool active = gtk_window_is_active(GTK_WINDOW(P.win));
        if (active) P.wasActive = true; else if (P.wasActive && !gtk_widget_get_visible(P.tonePop)) closePicker(); }), nullptr);
    g_timeout_add(530, [](gpointer) -> gboolean { P.caretOn = !P.caretOn; gtk_widget_set_opacity(P.caret, P.caretOn && !P.allSelected ? 1 : 0); return G_SOURCE_CONTINUE; }, nullptr);
#ifdef SEVI_HAVE_X11
    g_signal_connect(P.win, "realize", G_CALLBACK(+[](GtkWidget* w, gpointer) { placeX11(w); }), nullptr);
    g_signal_connect(P.win, "map", G_CALLBACK(+[](GtkWidget* w, gpointer) { g_timeout_add(30, [](gpointer w) -> gboolean { placeX11((GtkWidget*)w); activateX11((GtkWidget*)w); return FALSE; }, w); }), nullptr);
#endif
    relayout(); updateTexts();
    gtk_window_present(GTK_WINDOW(P.win));
}
