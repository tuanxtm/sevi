// Fat-finger simulator: types every word of a corpus as touches scattered around the key centers of
// the iOS layout (402 pt iPhone, measured from the system keyboard) and compares nearest-key decoding
// with TouchModel. usage: touchsim units.txt [noise] [weight] [sigma] [sure]
//   noise: finger scatter as a fraction of the key size (standard deviation). Prints key and word error rates.
#include "TouchModel.h"
#include "Engine.h"
#include "Driver.h"
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cmath>
#include <random>
#include <string>
#include <vector>
#include <chrono>
int vLanguage = 1, vSwitchKeyStatus = 0, vCheckSpelling = 1, vUseModernOrthography = 1, vRestoreIfWrongSpelling = 1,
    vFixRecommendBrowser = 1, vUseSmartSwitchKey = 1, vOtherLanguage = 1, vAutoSwitchByTyping = 1;

int main(int argc, char** argv) {
    if (getenv("TOUCH_DELETE")) TouchModel::deleteCost = atof(getenv("TOUCH_DELETE"));
    if (getenv("TOUCH_FLOOR_LONG")) TouchModel::correctFloorLong = atof(getenv("TOUCH_FLOOR_LONG"));
    static std::string keepEnv;
    if (getenv("TOUCH_KEEP")) { FILE* kf = fopen(getenv("TOUCH_KEEP"), "r"); char kb[256]; while (kf && fgets(kb, sizeof kb, kf)) keepEnv += kb; if (kf) fclose(kf); TouchModel::keepWords = keepEnv.c_str(); }

    double noise = argc > 2 ? atof(argv[2]) : 0.30;
    if (argc > 3) TouchModel::weight = atof(argv[3]);
    if (argc > 4) TouchModel::sigma = atof(argv[4]);
    if (argc > 5) TouchModel::sureZone = atof(argv[5]);
    if (argc > 6) TouchModel::modeShare = atof(argv[6]);
    TouchModel::sigmaY = argc > 7 ? atof(argv[7]) : TouchModel::sigmaY;
    // up/down scatter: real touch data (Google TSI, Aalto; sevi-data/public) is ~1.8 mm both ways,
    // which is 0.30 key width and 0.20 key height on an iPhone
    double noiseY = getenv("TOUCH_NOISE_Y") ? atof(getenv("TOUCH_NOISE_Y")) : noise * 2 / 3;
    bool realMode = !getenv("TOUCH_LABEL_MODE");   // mode at each word = what the engine is in (lazy typist), not the label
    const float W = 402, side = 6.67f, gap = 6, key = (W - 2 * side - 9 * gap) / 10, h = 43, pitch = 54;
    std::vector<TMKey> keys;
    const char* rows[3] = {"qwertyuiop", "asdfghjkl", "zxcvbnm"};
    float x0[3] = {side, side + (key + gap) / 2, side + (key + gap) * 1.5f};
    for (int r = 0; r < 3; r++) for (int i = 0; rows[r][i]; i++) keys.push_back({rows[r][i], x0[r] + i * (key + gap), r * pitch, key, h});
    auto center = [&](char c, float& x, float& y) { for (auto& k : keys) if (k.letter == c) { x = k.x + k.w / 2; y = k.y + k.h / 2; return true; } return false; };
    auto nearest = [&](float x, float y) { char best = 0; float bd = 1e30f; for (auto& k : keys) { float dx = (x - (k.x + k.w / 2)) / k.w, dy = (y - (k.y + k.h / 2)) / k.h; float d = dx * dx + dy * dy; if (d < bd) { bd = d; best = k.letter; } } return best; };

    std::mt19937 rng(42);
    std::normal_distribution<float> nx(0, noise * key), ny(0, noiseY * h);
    SeviDriver drv; vLanguage = 1;
    auto keyOf = [](char c, Uint16& key, bool& shift) {
        static const Uint16 L[26] = {KEY_A,KEY_B,KEY_C,KEY_D,KEY_E,KEY_F,KEY_G,KEY_H,KEY_I,KEY_J,KEY_K,KEY_L,KEY_M,KEY_N,KEY_O,KEY_P,KEY_Q,KEY_R,KEY_S,KEY_T,KEY_U,KEY_V,KEY_W,KEY_X,KEY_Y,KEY_Z};
        shift = c >= 'A' && c <= 'Z'; char l = shift ? c + 32 : c;
        if (l >= 'a' && l <= 'z') { key = L[l - 'a']; return true; }
        if (c == ' ') { key = KEY_SPACE; return true; } if (c == ',') { key = KEY_COMMA; return true; } if (c == '.') { key = KEY_DOT; return true; }
        return false; };
    FILE* f = fopen(argv[1], "r"); char line[512];
    double pickNs = 0;
    bool doCorrect = getenv("TOUCH_CORRECT") != nullptr;
    long wordErrC = 0, corrections = 0, goodFix = 0, badFix = 0;
    long keysTotal = 0, keyErrN = 0, keyErrM = 0, words = 0, wordErrN = 0, wordErrM = 0, rescued = 0, broken = 0;
    while (fgets(line, sizeof line, f)) {
        if (line[0] != 'W') continue;
        line[strcspn(line, "\n")] = 0;
        char* k = line + 2; char* t = strchr(k, '\t'); if (!t) continue; *t = 0;
        int vn = realMode ? vLanguage : t[1] == '1';
        std::string word;
        for (char* p = k; *p; p++) { char c = (*p >= 'A' && *p <= 'Z') ? *p + 32 : *p; if (c >= 'a' && c <= 'z') word += c; }
        if (word.size() < 1) continue;
        words++;
        std::string decN, decM;
        std::vector<float> txs, tys;
        for (char c : word) {
            float cx, cy; center(c, cx, cy);
            float x = cx + nx(rng), y = cy + ny(rng);
            char a = nearest(x, y);
            auto t0 = std::chrono::steady_clock::now();
            char b = TouchModel::pick(x, y, decM.c_str(), vn, keys.data(), (int)keys.size());
            pickNs += std::chrono::duration<double, std::nano>(std::chrono::steady_clock::now() - t0).count();
            decN += a; decM += b; txs.push_back(x); tys.push_back(y);
            keysTotal++; if (a != c) keyErrN++; if (b != c) keyErrM++;
        }
        // the typist types the intended word (with a space): keeps the engine's mode realistic for the next word
        for (char* p = k; *p; p++) { Uint16 kk; bool sh; if (keyOf(*p, kk, sh)) drv.press(kk, sh, 0); }
        { Uint16 kk; bool sh; keyOf(' ', kk, sh); drv.press(kk, sh, 0); }
        bool okN = decN == word, okM = decM == word;
        int caps = 0; for (char* p = k; *p; p++) if (*p >= 'A' && *p <= 'Z') caps++;
        if (doCorrect) {
            std::string fin = decM;
            if (caps >= 2) { if (fin != word) wordErrC++; goto done; }
            TMCandidate cand[8];
            int nc = TouchModel::candidates(decM.c_str(), txs.data(), tys.data(), vn, keys.data(), (int)keys.size(), cand, 8);
            if (TouchModel::shouldCorrect(decM.c_str(), cand, nc)) { fin = cand[0].letters; corrections++; if (okM) { badFix++; if (getenv("TOUCH_SHOW")) fprintf(stderr, "BROKE %s -> %s\n", decM.c_str(), fin.c_str()); } else if (fin == word) goodFix++; }
            if (fin != word) wordErrC++;
        }
        done:;
        if (!okN) wordErrN++; if (!okM) wordErrM++;
        if (!okN && okM) rescued++; if (okN && !okM) broken++;
    }
    printf("noise %.2f weight %.2f sigma %.2f sure %.2f | keys %ld: nearest %.2f%% model %.2f%% | words %ld: nearest %.2f%% wrong, model %.2f%% wrong (rescued %ld, broken %ld)\n",
           noise, TouchModel::weight, TouchModel::sigma, TouchModel::sureZone, keysTotal, 100.0 * keyErrN / keysTotal, 100.0 * keyErrM / keysTotal,
           words, 100.0 * wordErrN / words, 100.0 * wordErrM / words, rescued, broken);
    printf("pick: %.0f ns per touch on average\n", pickNs / keysTotal);
    if (doCorrect) printf("auto-correct: %.2f%% wrong words after correction; %ld corrections: %ld fixed a wrong word, %ld broke a right one\n",
                          100.0 * wordErrC / words, corrections, goodFix, badFix);
}
