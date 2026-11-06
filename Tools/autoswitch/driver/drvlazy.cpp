// Lazy-typist run through Engine/Driver (the iOS keyboard's logic): must match sim --lazy exactly.
// usage: drvlazy units.txt [--start en]
#include "Driver.h"
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>
#include <cstdlib>
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
    static const Uint16 digits[10] = {KEY_0,KEY_1,KEY_2,KEY_3,KEY_4,KEY_5,KEY_6,KEY_7,KEY_8,KEY_9};
    if (c >= '0' && c <= '9') { key = digits[c - '0']; return true; }
    switch (c) {
        case ' ': key = KEY_SPACE; return true;   case ',': key = KEY_COMMA; return true;
        case '.': key = KEY_DOT; return true;     case ';': key = KEY_SEMICOLON; return true;
        case ':': key = KEY_SEMICOLON; shift = true; return true;
        case '?': key = KEY_SLASH; shift = true; return true;
        case '!': key = KEY_1; shift = true; return true;
        case '\'': key = KEY_QUOTE; return true;  case '"': key = KEY_QUOTE; shift = true; return true;
        case '(': key = KEY_9; shift = true; return true;  case ')': key = KEY_0; shift = true; return true;
        case '-': key = KEY_MINUS; return true;   case '/': key = KEY_SLASH; return true;
        case '\n': key = KEY_RETURN; return true;
    }
    return false;
}
int main(int argc, char** argv) {
    SeviDriver d;
    vLanguage = (argc > 3 && !strcmp(argv[2], "--start") && !strcmp(argv[3], "en")) ? 0 : 1;
    std::u16string screen;
    FILE* f = fopen(argv[1], "r"); char line[512]; long n = 0;
    std::string all;
    while (fgets(line, sizeof line, f)) {
        line[strcspn(line, "\n")] = 0;
        if (line[0] == 'W') { char* k = line + 2; char* t = strchr(k, '\t'); if (t) *t = 0; all += k; }
        else if (line[0] == 'N') all += '\n';
        else all += std::string(line + 2);
    }
    for (char c : all) {
        Uint16 k; bool sh;
        if (!keyFor(c, k, sh)) continue;
        VKEdit e = d.press(k, sh, 0);
        for (int i = 0; i < e.deleteCount && !screen.empty(); i++) screen.pop_back();
        screen += e.text;
        n++;
    }
    if (argc > 4 || getenv("DRV_OUT")) { FILE* o = fopen(getenv("DRV_OUT"), "w"); std::string u;
        for (char16_t c : screen) { if (c < 0x80) u += (char)c; else if (c < 0x800) { u += (char)(0xC0 | (c >> 6)); u += (char)(0x80 | (c & 0x3F)); }
            else { u += (char)(0xE0 | (c >> 12)); u += (char)(0x80 | ((c >> 6) & 0x3F)); u += (char)(0x80 | (c & 0x3F)); } }
        fputs(u.c_str(), o); fclose(o); }
    // print a checksum of the final text
    unsigned long h = 1469598103934665603UL;
    for (char16_t c : screen) { h ^= c; h *= 1099511628211UL; }
    printf("keys %ld chars %zu hash %lx\n", n, screen.size(), h);
}
