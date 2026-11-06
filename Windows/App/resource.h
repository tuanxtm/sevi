#pragma once
#define IDI_APP 101
#define IDI_TRAY_VI 102
#define IDI_TRAY_EN 103
#define IDR_SOUND_VI 201
#define IDR_SOUND_EN 202
#define IDR_EMOJI_JSON 203
#ifndef SEVI_VERSION
#define SEVI_VERSION "0.1.0"
#endif
#define SEVI_W2(x) L##x
#define SEVI_W(x) SEVI_W2(x)
#define SEVI_VERSION_W SEVI_W(SEVI_VERSION)
