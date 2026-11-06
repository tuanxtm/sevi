# Sevi.exe: tray app + hooks. Built only on Windows (MSVC).
# One version for every platform: Cargo.toml's workspace version, unless overridden.
unset(SEVI_VERSION CACHE)   # never a stale cached value
if(NOT SEVI_VERSION)
  file(STRINGS "${CMAKE_CURRENT_SOURCE_DIR}/../../Cargo.toml" _mv REGEX "^version = ")
  string(REGEX REPLACE "^version = \"([^\"]+)\".*" "\\1" SEVI_VERSION "${_mv}")
endif()
message(STATUS "Sevi version ${SEVI_VERSION}")
add_executable(sevi WIN32 main.cpp Core.cpp Prefs.cpp Site.cpp Settings.cpp Emoji.cpp Updater.cpp third_party/tweetnacl.c Sevi.rc)
set_source_files_properties(third_party/tweetnacl.c PROPERTIES COMPILE_FLAGS "/w")
set_target_properties(sevi PROPERTIES OUTPUT_NAME Sevi)
target_include_directories(sevi PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
target_compile_definitions(sevi PRIVATE UNICODE _UNICODE SEVI_VERSION="${SEVI_VERSION}")
target_link_libraries(sevi PRIVATE sevi_engine user32 shell32 gdi32 advapi32 imm32 psapi ole32 oleaut32 comctl32 gdiplus winhttp winmm d2d1 dwrite dwmapi normaliz)
target_link_options(sevi PRIVATE /MANIFEST:NO)
# Live test runner: a window with an edit control, types through SendInput while Sevi runs, reads the text back.
add_executable(sevi_livetest ../Tests/livetest.cpp Prefs.cpp)
target_include_directories(sevi_livetest PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
target_link_libraries(sevi_livetest PRIVATE sevi_engine user32 advapi32 shell32)
target_compile_definitions(sevi_livetest PRIVATE UNICODE _UNICODE)
