# Native hosts use Cargo's static library during the staged desktop migration.
find_program(SEVI_CARGO cargo REQUIRED)
set(SEVI_CARGO_DIR "${CMAKE_CURRENT_BINARY_DIR}/cargo")
if(WIN32)
  set(SEVI_RUST_LIB "${SEVI_CARGO_DIR}/release/sevi_engine.lib")
else()
  set(SEVI_RUST_LIB "${SEVI_CARGO_DIR}/release/libsevi_engine.a")
endif()
file(GLOB_RECURSE SEVI_RUST_SOURCES CONFIGURE_DEPENDS "${ROOT}/crates/sevi-engine/src/*")
add_custom_command(OUTPUT "${SEVI_RUST_LIB}"
  COMMAND ${CMAKE_COMMAND} -E env "CARGO_TARGET_DIR=${SEVI_CARGO_DIR}"
    ${SEVI_CARGO} build --locked --release --package sevi-engine --features ffi
    --manifest-path "${ROOT}/Cargo.toml"
  DEPENDS ${SEVI_RUST_SOURCES} "${ROOT}/Cargo.lock" "${ROOT}/Cargo.toml"
    "${ROOT}/crates/sevi-engine/Cargo.toml" "${ROOT}/rust-toolchain.toml"
  WORKING_DIRECTORY "${ROOT}" VERBATIM)
add_custom_target(sevi_rust_build DEPENDS "${SEVI_RUST_LIB}")
add_library(sevi_rust STATIC IMPORTED GLOBAL)
set_target_properties(sevi_rust PROPERTIES IMPORTED_LOCATION "${SEVI_RUST_LIB}")
add_dependencies(sevi_rust sevi_rust_build)
if(WIN32)
  target_link_libraries(sevi_rust INTERFACE advapi32 bcrypt kernel32 ntdll userenv ws2_32)
elseif(UNIX AND NOT APPLE)
  target_link_libraries(sevi_rust INTERFACE dl pthread m)
endif()
