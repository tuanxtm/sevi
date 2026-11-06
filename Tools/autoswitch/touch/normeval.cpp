// Auto-correct against real normalised social text (ViLexNorm: sevi-data/public/evals/vilexnorm_pairs.tsv).
// Each line: keys typed (Telex) <TAB> the intended word <TAB> 1 if typed as intended. Typed in Vietnamese mode,
// no touch data. Counts: right words broken, wrong words fixed to the intended one, other changes.
#include "TouchModel.h"
#include "Engine.h"
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
#include <map>
#include <algorithm>
int vLanguage = 1, vSwitchKeyStatus = 0, vCheckSpelling = 1, vUseModernOrthography = 1, vRestoreIfWrongSpelling = 1,
    vFixRecommendBrowser = 1, vUseSmartSwitchKey = 1, vOtherLanguage = 1, vAutoSwitchByTyping = 1;
static std::string utf8(const unsigned short* t, int n) {
    std::string s;
    for (int i = 0; i < n; i++) { unsigned c = t[i];
        if (c < 0x80) s += (char)c; else if (c < 0x800) { s += (char)(0xC0 | c >> 6); s += (char)(0x80 | (c & 63)); }
        else { s += (char)(0xE0 | c >> 12); s += (char)(0x80 | ((c >> 6) & 63)); s += (char)(0x80 | (c & 63)); } }
    for (auto& ch : s) if (ch >= 'A' && ch <= 'Z') ch += 32;
    return s;
}
int main(int argc, char** argv) {
    if (getenv("TOUCH_DELETE")) TouchModel::deleteCost = atof(getenv("TOUCH_DELETE"));
    if (getenv("TOUCH_FLOOR_LONG")) TouchModel::correctFloorLong = atof(getenv("TOUCH_FLOOR_LONG"));
    static std::string keepEnv;
    if (getenv("TOUCH_KEEP")) { FILE* kf = fopen(getenv("TOUCH_KEEP"), "r"); char kb[256]; while (kf && fgets(kb, sizeof kb, kf)) keepEnv += kb; if (kf) fclose(kf); TouchModel::keepWords = keepEnv.c_str(); }
    const float W = 402, side = 6.67f, gap = 6, key = (W - 2 * side - 9 * gap) / 10, h = 43, pitch = 54;
    std::vector<TMKey> keys; const char* rows[3] = {"qwertyuiop", "asdfghjkl", "zxcvbnm"};
    float x0[3] = {side, side + (key + gap) / 2, side + (key + gap) * 1.5f};
    for (int r = 0; r < 3; r++) for (int i = 0; rows[r][i]; i++) keys.push_back({rows[r][i], x0[r] + i * (key + gap), r * pitch, key, h});
    FILE* f = fopen(argv[1], "r"); char line[512];
    long right = 0, wrongIn = 0, broke = 0, fixed = 0, other = 0, wrongChanged = 0;
    std::map<std::string, int> brokeList;
    while (fgets(line, sizeof line, f)) {
        line[strcspn(line, "\r\n")] = 0;
        char* a = strchr(line, '\t'); if (!a) continue; *a = 0; char* b = strchr(a + 1, '\t'); if (!b) continue; *b = 0;
        std::string want = a + 1; bool same = b[1] == '1';
        if (same) right++; else wrongIn++;
        if (strlen(line) > 24) continue;
        TMCandidate c[8]; int n = TouchModel::candidates(line, nullptr, nullptr, 1, keys.data(), (int)keys.size(), c, 8);
        if (!TouchModel::shouldCorrect(line, c, n)) continue;
        std::string got = utf8(c[0].text, c[0].textLen);
        if (same) { broke++; brokeList[std::string(line) + " -> " + got]++; }
        else if (got == want) fixed++;
        else { other++; }
    }
    printf("vilexnorm words: %ld typed right, %ld not | broke %ld (%.2f%% of right words) | fixed %ld (%.1f%% of the rest), changed to something else %ld\n",
           right, wrongIn, broke, 100.0 * broke / right, fixed, 100.0 * fixed / wrongIn, other);
    if (getenv("TOUCH_SHOW")) { std::vector<std::pair<int, std::string>> v; for (auto& kv : brokeList) v.push_back({kv.second, kv.first}); std::sort(v.rbegin(), v.rend()); for (int i = 0; i < 25 && i < (int)v.size(); i++) fprintf(stderr, "%4d %s\n", v[i].first, v[i].second.c_str()); }
}
