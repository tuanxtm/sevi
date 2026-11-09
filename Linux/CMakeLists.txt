# Cargo builds the Rust desktop applications; CMake supplies Linux packaging.
# Configure:  cmake -S Linux -B Linux/build -G Ninja -DCMAKE_BUILD_TYPE=Release
# Build:      cmake --build Linux/build
cmake_minimum_required(VERSION 3.21)
project(Sevi LANGUAGES NONE)
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()
# Install to /usr like the packages do: the IBus component file bakes the libexec path in, so a /usr/local
# default would make the .deb/.rpm point at a binary that is not there.
if(CMAKE_INSTALL_PREFIX_INITIALIZED_TO_DEFAULT)
  set(CMAKE_INSTALL_PREFIX "/usr" CACHE PATH "install prefix" FORCE)
endif()
set(ROOT ${CMAKE_CURRENT_LIST_DIR}/..)
# One version for every platform: Cargo.toml's workspace version.
if(NOT SEVI_VERSION)
  file(STRINGS "${ROOT}/Cargo.toml" _mv REGEX "^version = ")
  string(REGEX REPLACE "^version = \"([^\"]+)\".*" "\\1" SEVI_VERSION "${_mv}")
endif()
message(STATUS "Sevi version ${SEVI_VERSION}")

include(${ROOT}/Linux/RustDesktop.cmake)

# ---- packages: .deb (Ubuntu/Debian) and .rpm (Fedora) via CPack -----------------------------------------
# The Arch .pkg.tar.zst is repacked from the .deb in CI (Linux/packaging/make-pacman.sh), not a CPack generator.
set(CPACK_PACKAGE_NAME "sevi")
set(CPACK_PACKAGE_VERSION "${SEVI_VERSION}")
set(CPACK_PACKAGE_CONTACT "Tony Dinh <patent@tdinh.me>")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "Sevi - Vietnamese input (Telex) with automatic English/Vietnamese switching")
set(CPACK_PACKAGE_HOMEPAGE_URL "https://vikey.app")
set(CPACK_PACKAGING_INSTALL_PREFIX "/usr")
set(CPACK_DEBIAN_PACKAGE_DEPENDS "ibus (>= 1.5), libpango-1.0-0, libpangocairo-1.0-0, libcairo2, libegl1, libgl1, libxkbcommon0, libwayland-client0, libx11-6, libgstreamer1.0-0, gstreamer1.0-plugins-base, gstreamer1.0-plugins-good, libnotify-bin, fonts-noto-color-emoji, curl")
set(CPACK_DEBIAN_PACKAGE_SECTION "utils")
set(CPACK_DEBIAN_PACKAGE_SHLIBDEPS ON)
if(CMAKE_SYSTEM_PROCESSOR MATCHES "^(x86_64|amd64|AMD64)$")
  set(CPACK_DEBIAN_PACKAGE_ARCHITECTURE "amd64")
elseif(CMAKE_SYSTEM_PROCESSOR MATCHES "^(aarch64|arm64|ARM64)$")
  set(CPACK_DEBIAN_PACKAGE_ARCHITECTURE "arm64")
endif()
set(CPACK_DEBIAN_FILE_NAME DEB-DEFAULT)
set(CPACK_DEBIAN_PACKAGE_CONTROL_EXTRA "${ROOT}/Linux/packaging/postinst")
set(CPACK_RPM_PACKAGE_REQUIRES "ibus >= 1.5, pango, cairo, mesa-libEGL, mesa-libGL, libxkbcommon, wayland-libs, libX11, gstreamer1, gstreamer1-plugins-base, gstreamer1-plugins-good, libnotify, google-noto-emoji-color-fonts, curl")
set(CPACK_RPM_PACKAGE_LICENSE "GPL-3.0")
set(CPACK_RPM_FILE_NAME RPM-DEFAULT)
set(CPACK_RPM_POST_INSTALL_SCRIPT_FILE "${ROOT}/Linux/packaging/postinst")
set(CPACK_GENERATOR "DEB;RPM")
include(CPack)
