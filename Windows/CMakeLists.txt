# Sevi engine + test tools, cross-platform (MSVC on Windows, clang on macOS for validation).
# Configure:  cmake -S Windows -B Windows/build -G Ninja -DCMAKE_BUILD_TYPE=Release
# Build:      cmake --build Windows/build
# The test binaries land where Tools/autoswitch/check.sh and the Python scripts expect them.
cmake_minimum_required(VERSION 3.21)
project(Sevi LANGUAGES C CXX)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

set(ROOT ${CMAKE_CURRENT_LIST_DIR}/..)
set(ENGINE ${ROOT}/Engine)
set(AS ${ROOT}/Tools/autoswitch)
option(SEVI_RUST_DESKTOP "Build the complete Rust Windows desktop host" ON)
if(WIN32 AND SEVI_RUST_DESKTOP)
  include(${ROOT}/Windows/RustDesktop.cmake)
  return()
endif()

set(ENGINE_SRC ${ENGINE}/Engine.cpp ${ENGINE}/Vietnamese.cpp ${ENGINE}/SmartSwitchKey.cpp)
option(SEVI_RUST_ENGINE "Use the Rust typing engine" ON)
if(SEVI_RUST_ENGINE)
  include(${ROOT}/cmake/RustEngine.cmake)
  set(ENGINE_SRC ${ENGINE}/RustBridge.cpp)
endif()

add_library(sevi_engine STATIC ${ENGINE_SRC} ${ENGINE}/Driver.cpp ${ENGINE}/TouchModel.cpp)
target_include_directories(sevi_engine PUBLIC ${ENGINE} ${ENGINE}/platforms)
if(MSVC)
  target_compile_options(sevi_engine PUBLIC /utf-8 /W0 /EHsc /O2 /bigobj)
  target_compile_definitions(sevi_engine PUBLIC _SILENCE_ALL_CXX17_DEPRECATION_WARNINGS NOMINMAX WIN32_LEAN_AND_MEAN _CRT_SECURE_NO_WARNINGS)
else()
  target_compile_options(sevi_engine PUBLIC -w -O2)
endif()

# A separate engine build with AS_TESTING for the simulator (check.sh compiles sim with -DAS_TESTING).
add_library(sevi_engine_testing STATIC ${ENGINE_SRC})
target_include_directories(sevi_engine_testing PUBLIC ${ENGINE} ${ENGINE}/platforms)
target_compile_definitions(sevi_engine_testing PUBLIC AS_TESTING)
if(MSVC)
  target_compile_options(sevi_engine_testing PUBLIC /utf-8 /W0 /EHsc /O2 /bigobj)
  target_compile_definitions(sevi_engine_testing PUBLIC _SILENCE_ALL_CXX17_DEPRECATION_WARNINGS NOMINMAX WIN32_LEAN_AND_MEAN _CRT_SECURE_NO_WARNINGS)
else()
  target_compile_options(sevi_engine_testing PUBLIC -w -O2)
endif()

if(SEVI_RUST_ENGINE)
  foreach(lib sevi_engine sevi_engine_testing)
    target_compile_definitions(${lib} PUBLIC SEVI_RUST_ENGINE)
    target_link_libraries(${lib} PUBLIC sevi_rust)
  endforeach()
endif()

function(sevi_tool name src outdir lib)
  add_executable(${name} ${src})
  target_link_libraries(${name} PRIVATE ${lib})
  set_target_properties(${name} PROPERTIES RUNTIME_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/tools"
    RUNTIME_OUTPUT_DIRECTORY_RELEASE "${CMAKE_CURRENT_BINARY_DIR}/tools"
    RUNTIME_OUTPUT_DIRECTORY_DEBUG "${CMAKE_CURRENT_BINARY_DIR}/tools")
endfunction()

sevi_tool(harness  ${AS}/test/harness.cpp   ${AS}/test   sevi_engine)
sevi_tool(sim      ${AS}/sim/sim.cpp         ${AS}/sim    sevi_engine_testing)
sevi_tool(drvlazy  ${AS}/driver/drvlazy.cpp  ${AS}/driver sevi_engine)
sevi_tool(bench    ${AS}/driver/bench.cpp    ${AS}/driver sevi_engine)
sevi_tool(touchsim ${AS}/touch/touchsim.cpp  ${AS}/touch  sevi_engine)

# Windows app (added in Phase 2)
if(WIN32 AND EXISTS ${ROOT}/Windows/App/CMakeLists.txt)
  add_subdirectory(${ROOT}/Windows/App App)
endif()
