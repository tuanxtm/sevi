// The emoji picker on Windows, 1:1 with App/Views/EmojiPicker.swift: a hotkey (off by default, set in the main
// window) opens a borderless popup in the middle of the screen with the mouse. Typing searches in English,
// Vietnamese and Telex keys (App/Model/EmojiSearch.swift), arrows move, Enter types the emoji into the app that
// was in front, Esc clears the search then closes. The popup never takes the focus: the keys reach it through
// the keyboard hook (Core.cpp), so that app keeps its cursor.
#pragma once
#include <windows.h>
#include <string>
#include <vector>

namespace sevi { namespace emoji {

void install(HINSTANCE inst);   // at launch: the hotkey handler, the index loaded in the background
void toggle();
void open();
void close();
bool isOpen();
void languageChanged(int lang);
void settingsChanged();         // EmojiPicker / EmojiHotkey changed in the main window or the onboarding

/// A small Telex reader for search text ("nguwaj" -> "ngựa"); exposed for tests.
std::wstring telexConvert(const std::wstring& s);

}} // namespace
