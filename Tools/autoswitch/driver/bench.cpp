// Performance lock: types the chat corpus through Engine/Driver (the code every keystroke runs on
// iOS, and the same engine calls the Mac makes) and checks time per key.
// usage: bench units.txt [max_avg_ns] [max_p999_ns]   exit 1 when over budget
#include "Driver.h"
#include <chrono>
#include <algorithm>
#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <string>
#include <vector>
int vLanguage = 1, vSwitchKeyStatus = 0, vCheckSpelling = 1, vUseModernOrthography = 1, vRestoreIfWrongSpelling = 1,
    vFixRecommendBrowser = 1, vUseSmartSwitchKey = 1, vOtherLanguage = 1, vAutoSwitchByTyping = 1;
static Uint16 letterKey(char c) {
    static const Uint16 k[26] = {KEY_A,KEY_B,KEY_C,KEY_D,KEY_E,KEY_F,KEY_G,KEY_H,KEY_I,KEY_J,KEY_K,KEY_L,KEY_M,KEY_N,KEY_O,KEY_P,KEY_Q,KEY_R,KEY_S,KEY_T,KEY_U,KEY_V,KEY_W,KEY_X,KEY_Y,KEY_Z};
    return k[c - 'a'];
}
static bool keyFor(char c, Uint16& key, bool& shift) {
    shift = false;
    if (c >= 'a' && c <= 'z') { key = letterKey(c); return true; }
    if (c >= 'A' && c <= 'Z') { key = letterKey(c + 32); shift = true; return true; }
    switch (c) { case ' ': key = KEY_SPACE; return true; case ',': key = KEY_COMMA; return true; case '.': key = KEY_DOT; return true;
                 case '\n': key = KEY_RETURN; return true; case '\'': key = KEY_QUOTE; return true; }
    return false;
}
int main(int argc, char** argv) {
    double maxAvg = argc > 2 ? atof(argv[2]) : 3000, maxP999 = argc > 3 ? atof(argv[3]) : 50000;
    std::string all; FILE* f = fopen(argv[1], "r"); char line[512];
    while (fgets(line, sizeof line, f)) {
        line[strcspn(line, "\n")] = 0;
        if (line[0] == 'W') { char* k = line + 2; char* t = strchr(k, '\t'); if (t) *t = 0; all += k; }
        else if (line[0] == 'N') all += '\n'; else all += std::string(line + 2);
    }
    std::vector<std::pair<Uint16, bool>> keys;
    for (char c : all) { Uint16 k; bool s; if (keyFor(c, k, s)) keys.push_back({k, s}); }
    std::vector<double> t; t.reserve(keys.size() * 3);
    for (int round = 0; round < 3; round++) {
        SeviDriver d; vLanguage = 1;
        for (auto& k : keys) {
            auto a = std::chrono::steady_clock::now();
            VKEdit e = d.press(k.first, k.second, 0);
            auto b = std::chrono::steady_clock::now();
            if (e.deleteCount < 0) return 2;
            if (round > 0) t.push_back(std::chrono::duration<double, std::nano>(b - a).count());
        }
    }
    double sum = 0; for (double x : t) sum += x;
    std::sort(t.begin(), t.end());
    double avg = sum / t.size(), p99 = t[t.size() * 99 / 100], p999 = t[t.size() * 999 / 1000], mx = t.back();
    printf("keys %zu  avg %.0f ns  p99 %.0f ns  p99.9 %.0f ns  max %.0f ns  (budget avg %.0f, p99.9 %.0f)\n", t.size(), avg, p99, p999, mx, maxAvg, maxP999);
    return (avg > maxAvg || p999 > maxP999) ? 1 : 0;
}
