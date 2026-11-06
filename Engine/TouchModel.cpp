//
//  TouchModel.cpp
//  Sevi
//

#include "TouchModel.h"
#include "Engine.h"
#include <cmath>
#include <cstring>
#include "KeepWords.inc"

float TouchModel::sigma = 0.35f;
float TouchModel::sigmaY = 0.25f;   // real touches scatter less up/down in key heights (D25: Google TSI, Aalto data)
float TouchModel::weight = 0.5f;
float TouchModel::sureZone = 0.8f;
float TouchModel::modeShare = 0.8f;
float TouchModel::correctMargin = 1.0f;
float TouchModel::correctFloor = -2.0f;
float TouchModel::deleteCost = -7.0f;   // -6 broke matcha→match, repost→repos (VietMix); -7 keeps 30%+ of real typos fixed
float TouchModel::correctFloorLong = -4.0f;   // tuned on GitHub/Birkbeck typos vs ViLexNorm/ViCTSD (D26)

/** P(next letter | prefix), mixing both languages by mode; smoothed so no letter is impossible. */
static void prior(const char* prefix, int vietnamese, double p[26]) {
    double vn[27], en[27];
    vAutoSwitchNextKeyMass(1, prefix, vn);
    vAutoSwitchNextKeyMass(0, prefix, en);
    double tv = 0, te = 0;
    for (int i = 0; i < 26; i++) { tv += vn[i]; te += en[i]; }
    double wv = vietnamese ? TouchModel::modeShare : 1 - TouchModel::modeShare, we = 1 - wv;
    if (tv <= 0) { wv = 0; we = 1; }
    if (te <= 0) { we = 0; wv = tv > 0 ? 1 : 0; }
    double total = 0;
    for (int i = 0; i < 26; i++) {
        double v = (tv > 0 ? vn[i] / tv : 0) * wv + (te > 0 ? en[i] / te : 0) * we;
        p[i] = v * 0.97 + 0.03 / 26;                       // smoothing: typos and unknown words stay possible
        total += p[i];
    }
    for (int i = 0; i < 26; i++) p[i] /= total;
}

char TouchModel::pick(float x, float y, const char* prefix, int vietnamese, const TMKey* keys, int n) {
    int nearest = -1; float bestD = 1e30f;
    for (int i = 0; i < n; i++) {
        const TMKey& k = keys[i];
        float cx = k.x + k.w / 2, cy = k.y + k.h / 2;
        float dx = (x - cx) / k.w, dy = (y - cy) / k.h;
        if (fabsf(dx) * 2 <= sureZone && fabsf(dy) * 2 <= sureZone) return k.letter;   // clearly this key
        float d = dx * dx + dy * dy;
        if (d < bestD) { bestD = d; nearest = i; }
    }
    if (nearest < 0) return 0;
    if (weight <= 0) return keys[nearest].letter;
    double p[26];
    prior(prefix, vietnamese, p);
    double best = -1e300; char letter = keys[nearest].letter;
    for (int i = 0; i < n; i++) {
        const TMKey& k = keys[i];
        float cx = k.x + k.w / 2, cy = k.y + k.h / 2;
        float dx = (x - cx) / (k.w * sigma), dy = (y - cy) / (k.h * sigmaY);
        double spatial = -0.5 * (dx * dx + dy * dy);          // log of a 2D Gaussian
        if (spatial < -8) continue;                            // far keys never win
        int li = k.letter - 'a';
        if (li < 0 || li >= 26) continue;
        double score = spatial + weight * log(p[li]);
        if (score > best) { best = score; letter = k.letter; }
    }
    return letter;
}

// ---- Auto-correct (D26) --------------------------------------------------------------------------

static const TMKey* keyFor(char c, const TMKey* keys, int n) { for (int i = 0; i < n; i++) if (keys[i].letter == c) return &keys[i]; return nullptr; }

/** log P(touch at x,y | meant key c) up to a constant; without a touch, how close c is to the typed key. */
static double spatial(char typed, char c, float x, float y, bool haveTouch, const TMKey* keys, int n) {
    const TMKey* k = keyFor(c, keys, n);
    if (!k) return -20;
    float px = x, py = y;
    if (!haveTouch) { const TMKey* t = keyFor(typed, keys, n); if (!t) return -20; px = t->x + t->w / 2; py = t->y + t->h / 2; }
    float dx = (px - (k->x + k->w / 2)) / (k->w * TouchModel::sigma), dy = (py - (k->y + k->h / 2)) / (k->h * TouchModel::sigmaY);
    return -0.5 * (dx * dx + dy * dy);
}

static void addCandidate(const char* letters, double editCost, int mode, TMCandidate* out, int& count, int max) {
    for (int v = 0; v < 2; v++) {
        unsigned short text[48]; int len = 0;
        int s = vAutoSwitchWordScore(v, letters, text, &len);
        if (s <= -1000 || len <= 0 || len > 47) continue;
        double score = s / 10.0 * 2.302585 + editCost + (v == mode ? 0 : -1.5);
        // keep the best per rendered text
        int dup = -1;
        for (int i = 0; i < count; i++) if (out[i].textLen == len && !memcmp(out[i].text, text, len * 2)) { dup = i; break; }
        if (dup >= 0) { if (out[dup].score >= score) continue; }
        TMCandidate c;
        strncpy(c.letters, letters, 31); c.letters[31] = 0;
        memcpy(c.text, text, len * 2); c.textLen = len; c.vietnamese = v; c.score = score;
        if (dup >= 0) { out[dup] = c; continue; }
        if (count < max) out[count++] = c;
        else { int worst = 0; for (int i = 1; i < count; i++) if (out[i].score < out[worst].score) worst = i; if (out[worst].score < score) out[worst] = c; }
    }
}

int TouchModel::candidates(const char* typed, const float* tx, const float* ty, int vietnamese, const TMKey* keys, int n, TMCandidate* out, int max) {
    int L = (int)strlen(typed), count = 0;
    if (L < 1 || L > 24) return 0;
    char w[32];
    bool touch = tx && ty;
    // the word as typed
    addCandidate(typed, 0, vietnamese, out, count, max);
    for (int i = 0; i < L; i++) {
        float x = touch ? tx[i] : 0, y = touch ? ty[i] : 0;
        double here = spatial(typed[i], typed[i], x, y, touch, keys, n);
        // substitution: the touch meant a nearby key
        for (int c = 'a'; c <= 'z'; c++) {
            if (c == typed[i]) continue;
            double sp = spatial(typed[i], (char)c, x, y, touch, keys, n);
            double cost = sp - here - 1.0;
            if (cost < -9) continue;
            strcpy(w, typed); w[i] = (char)c;
            addCandidate(w, cost, vietnamese, out, count, max);
        }
        // deletion: an extra key (never down to a 1–2 letter word: usd -> us, kpi -> ki)
        if (L > 3) {
            int j = 0; for (int k = 0; k < L; k++) if (k != i) w[j++] = typed[k]; w[j] = 0;
            addCandidate(w, TouchModel::deleteCost, vietnamese, out, count, max);
        }
        // transposition
        if (i + 1 < L && typed[i] != typed[i + 1]) {
            strcpy(w, typed); w[i] = typed[i + 1]; w[i + 1] = typed[i];
            addCandidate(w, -4.0, vietnamese, out, count, max);
        }
    }
    // insertion: a missed key
    for (int i = 0; i <= L && L < 23; i++) {
        for (int c = 'a'; c <= 'z'; c++) {
            memcpy(w, typed, i); w[i] = (char)c; memcpy(w + i + 1, typed + i, L - i); w[L + 1] = 0;
            addCandidate(w, -6.5, vietnamese, out, count, max);
        }
    }
    for (int i = 1; i < count; i++) for (int j = i; j > 0 && out[j].score > out[j - 1].score; j--) { TMCandidate t = out[j]; out[j] = out[j - 1]; out[j - 1] = t; }
    return count;
}

/** An English inflection of a dictionary word (testers, balancer, hodling stays out): not a typo. */
static bool inflected(const char* w) {
    static const char* suf[] = {"s", "es", "ed", "d", "ing", "er", "ers", "ly", "est"};
    int L = (int)strlen(w);
    char b[32];
    for (const char* s : suf) {
        int n = (int)strlen(s);
        if (L - n < 3 || L >= 31 || strcmp(w + L - n, s)) continue;
        memcpy(b, w, L - n); b[L - n] = 0;
        if (vAutoSwitchWordScore(0, b, nullptr, nullptr) > -1000) return true;
        if (!strcmp(s, "ing") || !strcmp(s, "ed") || !strcmp(s, "er")) { b[L - n] = 'e'; b[L - n + 1] = 0; if (vAutoSwitchWordScore(0, b, nullptr, nullptr) > -1000) return true; }
    }
    return false;
}

const char* TouchModel::keepWords = "";

bool TouchModel::shouldCorrect(const char* typed, const TMCandidate* c, int count) {
    if (count == 0 || !strcmp(c[0].letters, typed)) return false;          // best is what was typed
    for (int i = 0; i < count; i++) if (!strcmp(c[i].letters, typed)) return false;   // the typed word is a real word: keep it
    if (strlen(typed) < 3) return false;                                   // too little to go on (ok, ko, j…)
    // a real stem + ending (githubs, tweeted): keep it, unless the fix is the same word spelled right (occured -> occurred)
    if (inflected(typed)) {
        size_t L = strlen(typed), cl = strlen(c[0].letters);
        bool sameEnding = cl >= 2 && L >= 2 && !strcmp(c[0].letters + cl - 2, typed + L - 2) && !strncmp(c[0].letters, typed, 3);
        if (!sameEnding) return false;
    }
    if (vAutoSwitchIsVietnameseSpelling(typed)) return false;               // a rare but valid syllable (thạnh): keep it
    // words people really type (ntn, cta, xog, brands…): never "fixed"
    for (int lo = 0, hi = kKeepWordsCount - 1; lo <= hi; ) {
        int mid = (lo + hi) / 2, cmp = strcmp(kKeepWords[mid], typed);
        if (cmp == 0) return false;
        if (cmp < 0) lo = mid + 1; else hi = mid - 1;
    }
    // more words to keep, for experiments (one per line)
    size_t tl = strlen(typed);
    for (const char* p = keepWords; *p; ) {
        const char* e = strchr(p, '\n'); size_t n = e ? (size_t)(e - p) : strlen(p);
        if (n == tl && !strncmp(p, typed, n)) return false;
        p += n + (e ? 1 : 0);
    }
    double margin = count > 1 ? c[0].score - c[1].score : 10;
    // a long word that is no word in either language (nor kept) is almost surely a slip: trust rarer fixes
    size_t L = strlen(typed);
    float floor = L >= 6 ? TouchModel::correctFloorLong : L == 5 ? (TouchModel::correctFloor + TouchModel::correctFloorLong) / 2 : TouchModel::correctFloor;
    return c[0].score > floor && margin > TouchModel::correctMargin;
}
