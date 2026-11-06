# Preserve the existing app/installer output paths; Cargo owns the executable.
find_program(SEVI_CARGO cargo REQUIRED)
if(CMAKE_GENERATOR_PLATFORM MATCHES "ARM64" OR CMAKE_SYSTEM_PROCESSOR MATCHES "ARM64|aarch64" OR "$ENV{VSCMD_ARG_TGT_ARCH}" STREQUAL "arm64")
  set(SEVI_RUST_TARGET aarch64-pc-windows-msvc)
else()
  set(SEVI_RUST_TARGET x86_64-pc-windows-msvc)
endif()
set(SEVI_CARGO_TARGET "${CMAKE_CURRENT_BINARY_DIR}/cargo")
file(GLOB_RECURSE SEVI_RUST_SOURCES CONFIGURE_DEPENDS
  "${ROOT}/crates/sevi-engine/src/*" "${ROOT}/crates/sevi-runtime/src/*"
  "${ROOT}/crates/sevi-windows/src/*" "${ROOT}/Windows/App/Sounds/*")
file(GLOB_RECURSE SEVI_SHARED_UI_SOURCES CONFIGURE_DEPENDS
  "${ROOT}/crates/sevi-ui/src/*" "${ROOT}/crates/sevi-ui/assets/*"
  "${ROOT}/crates/sevi-ui-protocol/src/*")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/App/Sevi.exe" "${CMAKE_CURRENT_BINARY_DIR}/App/sevi_livetest.exe"
  COMMAND "${SEVI_CARGO}" build --locked --release --package sevi-windows --bins
    --target "${SEVI_RUST_TARGET}" --target-dir "${SEVI_CARGO_TARGET}"
  COMMAND ${CMAKE_COMMAND} -E make_directory "${CMAKE_CURRENT_BINARY_DIR}/App"
  COMMAND ${CMAKE_COMMAND} -E copy_if_different "${SEVI_CARGO_TARGET}/${SEVI_RUST_TARGET}/release/sevi.exe" "${CMAKE_CURRENT_BINARY_DIR}/App/Sevi.exe"
  COMMAND ${CMAKE_COMMAND} -E copy_if_different "${SEVI_CARGO_TARGET}/${SEVI_RUST_TARGET}/release/sevi-windows-livetest.exe" "${CMAKE_CURRENT_BINARY_DIR}/App/sevi_livetest.exe"
  WORKING_DIRECTORY "${ROOT}" DEPENDS ${SEVI_RUST_SOURCES} ${SEVI_SHARED_UI_SOURCES} "${ROOT}/Cargo.toml" "${ROOT}/Cargo.lock"
    "${ROOT}/crates/sevi-windows/Cargo.toml" "${ROOT}/crates/sevi-windows/build.rs"
    "${ROOT}/.cargo/config.toml"
    "${ROOT}/Windows/App/Sevi.rc" "${ROOT}/Windows/App/Sevi.manifest" "${ROOT}/Windows/App/Sevi.ico"
  COMMENT "Building the Rust native Windows desktop application" VERBATIM)
add_custom_target(sevi ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/App/Sevi.exe")
add_custom_target(sevi_livetest DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/App/sevi_livetest.exe")
