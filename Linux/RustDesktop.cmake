# Cargo owns both native desktop executables. CMake retains distribution packaging.
find_program(SEVI_CARGO cargo REQUIRED)
include(GNUInstallDirs)
set(SEVI_CARGO_TARGET "${CMAKE_CURRENT_BINARY_DIR}/cargo")
file(GLOB_RECURSE SEVI_RUST_SOURCES CONFIGURE_DEPENDS
  "${ROOT}/crates/sevi-engine/src/*" "${ROOT}/crates/sevi-runtime/src/*"
  "${ROOT}/crates/sevi-linux/src/*")
file(GLOB_RECURSE SEVI_SHARED_UI_SOURCES CONFIGURE_DEPENDS
  "${ROOT}/crates/sevi-ui/src/*" "${ROOT}/crates/sevi-ui/assets/*"
  "${ROOT}/crates/sevi-ui-protocol/src/*")
add_custom_command(
  OUTPUT "${SEVI_CARGO_TARGET}/release/ibus-engine-sevi" "${SEVI_CARGO_TARGET}/release/sevi-settings"
  COMMAND "${SEVI_CARGO}" build --locked --release --package sevi-linux
    --bin ibus-engine-sevi --target-dir "${SEVI_CARGO_TARGET}"
  COMMAND "${SEVI_CARGO}" build --locked --release --package sevi-linux
    --features settings-ui --bin sevi-settings --target-dir "${SEVI_CARGO_TARGET}"
  WORKING_DIRECTORY "${ROOT}"
  DEPENDS ${SEVI_RUST_SOURCES} ${SEVI_SHARED_UI_SOURCES} "${ROOT}/Cargo.toml" "${ROOT}/Cargo.lock"
    "${ROOT}/crates/sevi-linux/Cargo.toml" "${ROOT}/crates/sevi-runtime/Cargo.toml"
    "${ROOT}/crates/sevi-ui/Cargo.toml" "${ROOT}/crates/sevi-ui-protocol/Cargo.toml"
  COMMENT "Building the Rust IBus engine and shared settings application" VERBATIM)
add_custom_target(sevi-desktop ALL DEPENDS "${SEVI_CARGO_TARGET}/release/ibus-engine-sevi" "${SEVI_CARGO_TARGET}/release/sevi-settings")
install(PROGRAMS "${SEVI_CARGO_TARGET}/release/ibus-engine-sevi" DESTINATION "${CMAKE_INSTALL_LIBEXECDIR}")
install(PROGRAMS "${SEVI_CARGO_TARGET}/release/sevi-settings" DESTINATION "${CMAKE_INSTALL_BINDIR}")
configure_file("${ROOT}/Linux/ibus/sevi.xml.in" "${CMAKE_CURRENT_BINARY_DIR}/sevi.xml" @ONLY)
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/sevi.xml" DESTINATION "${CMAKE_INSTALL_DATADIR}/ibus/component")
install(FILES "${ROOT}/Linux/sounds/SwitchToVietnamese.wav" "${ROOT}/Linux/sounds/SwitchToEnglish.wav" DESTINATION "${CMAKE_INSTALL_DATADIR}/sevi/sounds")
install(FILES "${ROOT}/App/Resources/Emoji.json" DESTINATION "${CMAKE_INSTALL_DATADIR}/sevi")
install(FILES "${ROOT}/Linux/settings/sevi.desktop" DESTINATION "${CMAKE_INSTALL_DATADIR}/applications")
install(FILES "${ROOT}/site/assets/icon.png" DESTINATION "${CMAKE_INSTALL_DATADIR}/icons/hicolor/512x512/apps" RENAME sevi.png)
