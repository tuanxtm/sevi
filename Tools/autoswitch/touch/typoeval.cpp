// Auto-correct on real English typos (GitHub Typo Corpus, Birkbeck...: sevi-data/public), no touch data
// (key centres only). usage: typoeval pairs.tsv [mode 0/1] [keep.txt]
// pairs.tsv: typo<TAB>correction[<TAB>count]. Prints: fixed right, changed to something else, left alone.
#include "TouchModel.h"
#include "Engine.h"
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
int vLanguage = 0, vSwitchKeyStatus = 0, vCheckSpelling = 1, vUseModernOrthography = 1, vRestoreIfWrongSpelling = 1,
    vFixRecommendBrowser = 1, vUseSmartSwitchKey = 1, vOtherLanguage = 1, vAutoSwitchByTyping = 1;
int main(int argc, char** argv) {
    if (getenv("TOUCH_DELETE")) TouchModel::deleteCost = atof(getenv("TOUCH_DELETE"));
    if (getenv("TOUCH_FLOOR_LONG")) TouchModel::correctFloorLong = atof(getenv("TOUCH_FLOOR_LONG"));
    static std::string keepEnv;
    if (getenv("TOUCH_KEEP")) { FILE* kf = fopen(getenv("TOUCH_KEEP"), "r"); char kb[256]; while (kf && fgets(kb, sizeof kb, kf)) keepEnv += kb; if (kf) fclose(kf); TouchModel::keepWords = keepEnv.c_str(); }

    int mode = argc > 2 ? atoi(argv[2]) : 0;
    std::string keep;
    if (argc > 3) { FILE* k = fopen(argv[3], "r"); char b[256]; while (fgets(b, sizeof b, k)) keep += b; fclose(k); TouchModel::keepWords = keep.c_str(); }
    const float W = 402, side = 6.67f, gap = 6, key = (W - 2 * side - 9 * gap) / 10, h = 43, pitch = 54;
    std::vector<TMKey> keys;
    const char* rows[3] = {"qwertyuiop", "asdfghjkl", "zxcvbnm"};
    float x0[3] = {side, side + (key + gap) / 2, side + (key + gap) * 1.5f};
    for (int r = 0; r < 3; r++) for (int i = 0; rows[r][i]; i++) keys.push_back({rows[r][i], x0[r] + i * (key + gap), r * pitch, key, h});
    FILE* f = fopen(argv[1], "r"); char line[512];
    long pairs = 0, right = 0, wrong = 0, left = 0, known = 0; long wRight = 0, wWrong = 0, wLeft = 0, wAll = 0;
    bool show = getenv("TYPO_SHOW");
    while (fgets(line, sizeof line, f)) {
        line[strcspn(line, "\r\n")] = 0;
        char* t = strchr(line, '\t'); if (!t) continue; *t = 0;
        char* corr = t + 1; char* c3 = strchr(corr, '\t'); long cnt = 1; if (c3) { *c3 = 0; cnt = atol(c3 + 1); if (cnt < 1) cnt = 1; }
        bool ok = true; for (char* p = line; *p; p++) if (*p < 'a' || *p > 'z') ok = false;
        for (char* p = corr; *p; p++) if (*p < 'a' || *p > 'z') ok = false;
        if (!ok || strlen(line) < 3 || strlen(line) > 20) continue;
        TMCandidate cand[8];
        int nc = TouchModel::candidates(line, nullptr, nullptr, mode, keys.data(), (int)keys.size(), cand, 8);
        bool isWord = false; for (int i = 0; i < nc; i++) if (!strcmp(cand[i].letters, line)) isWord = true;
        if (isWord) { known++; continue; }      // the "typo" is itself a word: no keyboard can know
        pairs++; wAll += cnt;
        if (TouchModel::shouldCorrect(line, cand, nc)) {
            if (!strcmp(cand[0].letters, corr)) { right++; wRight += cnt; }
            else { wrong++; wWrong += cnt; if (show) fprintf(stderr, "WRONG %s -> %s (meant %s)\n", line, cand[0].letters, corr); }
        } else { left++; wLeft += cnt; if (show) fprintf(stderr, "LEFT %s (meant %s)\n", line, corr); }
    }
    printf("typos %ld (skipped %ld that are words): fixed %.1f%%, wrong fix %.1f%%, left %.1f%% | by count: fixed %.1f%%, wrong %.1f%%, left %.1f%%\n",
           pairs, known, 100.0 * right / pairs, 100.0 * wrong / pairs, 100.0 * left / pairs, 100.0 * wRight / wAll, 100.0 * wWrong / wAll, 100.0 * wLeft / wAll);
}
