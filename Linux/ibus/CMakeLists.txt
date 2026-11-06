# ibus-engine-sevi (+ later sevi-settings). Version = project.yml MARKETING_VERSION, like the other platforms.
find_package(PkgConfig REQUIRED)
pkg_check_modules(IBUS REQUIRED IMPORTED_TARGET ibus-1.0 glib-2.0 gio-2.0 gstreamer-1.0)
pkg_check_modules(X11 IMPORTED_TARGET x11)
include(GNUInstallDirs)

add_executable(ibus-engine-sevi Engine.cpp Core.cpp Prefs.cpp)
target_include_directories(ibus-engine-sevi PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
target_compile_definitions(ibus-engine-sevi PRIVATE SEVI_VERSION="${SEVI_VERSION}" SEVI_DATADIR="${CMAKE_INSTALL_FULL_DATADIR}/sevi")
target_link_libraries(ibus-engine-sevi PRIVATE sevi_engine PkgConfig::IBUS)
if(X11_FOUND)
  target_compile_definitions(ibus-engine-sevi PRIVATE SEVI_HAVE_X11)
  target_link_libraries(ibus-engine-sevi PRIVATE PkgConfig::X11)
endif()
configure_file(sevi.xml.in ${CMAKE_CURRENT_BINARY_DIR}/sevi.xml @ONLY)
install(TARGETS ibus-engine-sevi DESTINATION ${CMAKE_INSTALL_LIBEXECDIR})
install(FILES ${CMAKE_CURRENT_BINARY_DIR}/sevi.xml DESTINATION ${CMAKE_INSTALL_DATADIR}/ibus/component)
# shared data: the switch sounds (the Mac's .caf files as .wav) and the emoji list of the picker
install(FILES ../sounds/SwitchToVietnamese.wav ../sounds/SwitchToEnglish.wav DESTINATION ${CMAKE_INSTALL_DATADIR}/sevi/sounds)
install(FILES ${ROOT}/App/Resources/Emoji.json DESTINATION ${CMAKE_INSTALL_DATADIR}/sevi)

add_subdirectory(../settings ${CMAKE_CURRENT_BINARY_DIR}/settings)
