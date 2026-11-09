# Cargo builds the native Rust app and live runner; preserve installer paths.
cmake_minimum_required(VERSION 3.21)
project(Sevi LANGUAGES NONE)
set(ROOT ${CMAKE_CURRENT_LIST_DIR}/..)
include(${ROOT}/Windows/RustDesktop.cmake)
