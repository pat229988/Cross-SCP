# SPDX-License-Identifier: AGPL-3.0-or-later

set(CROSSSCP_GUI_VERSION ${PROJECT_VERSION})

qt_policy(SET QTP0001 NEW)
if(Qt6_VERSION VERSION_GREATER_EQUAL 6.8.0)
  qt_policy(SET QTP0004 NEW)
endif()

qt_add_executable(crossscp-gui
  cpp/main.cpp
  cpp/AppBackend.cpp
  cpp/AppBackend.h
  cpp/LocalFileModel.cpp
  cpp/LocalFileModel.h
  cpp/RemoteFileModel.cpp
  cpp/RemoteFileModel.h
  cpp/TransferQueueModel.cpp
  cpp/TransferQueueModel.h
  resources/icons/CrossSCP.icns
)

set_source_files_properties(resources/icons/CrossSCP.icns PROPERTIES
  MACOSX_PACKAGE_LOCATION "Resources"
)

qt_add_qml_module(crossscp-gui
  URI CrossSCP
  VERSION 1.0
  QML_FILES
    qml/Main.qml
  RESOURCES
    resources/icons/crossscp.svg
    resources/icons/crossscp-full.png
    resources/icons/crossscp-256.png
    resources/icons/crossscp-512.png
    resources/icons/crossscp-1024.png
    resources/actions/upload.png
    resources/actions/download.png
    resources/actions/new-folder.png
    resources/actions/delete.svg
)

target_compile_features(crossscp-gui PRIVATE cxx_std_20)
target_link_libraries(crossscp-gui PRIVATE Qt6::Core Qt6::Gui Qt6::Qml Qt6::Quick)

option(CROSSSCP_GUI_BUILD_RUST_CLI "Build and stage the Rust CLI bridge for the Qt GUI" ON)
option(CROSSSCP_GUI_ENABLE_SFTP "Build the staged Rust CLI bridge with live SFTP support" ON)
if(CROSSSCP_GUI_BUILD_RUST_CLI)
  find_program(CARGO_EXECUTABLE cargo REQUIRED)
  set(CROSSSCP_CARGO_PROFILE_ARGS "")
  set(CROSSSCP_CARGO_FEATURE_ARGS "")
  set(CROSSSCP_CARGO_TARGET_DIR "debug")
  if(CMAKE_BUILD_TYPE STREQUAL "Release" OR CMAKE_BUILD_TYPE STREQUAL "RelWithDebInfo" OR CMAKE_BUILD_TYPE STREQUAL "MinSizeRel")
    set(CROSSSCP_CARGO_PROFILE_ARGS "--release")
    set(CROSSSCP_CARGO_TARGET_DIR "release")
  endif()
  if(CROSSSCP_GUI_ENABLE_SFTP)
    set(CROSSSCP_CARGO_FEATURE_ARGS "--features" "ssh2-backend")
  endif()
  add_custom_target(crossscp-rust-cli ALL
    COMMAND ${CARGO_EXECUTABLE} build -p crossscp-cli ${CROSSSCP_CARGO_PROFILE_ARGS} ${CROSSSCP_CARGO_FEATURE_ARGS}
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
    COMMENT "Building Rust CLI bridge with GUI runtime features"
    VERBATIM
  )
  add_dependencies(crossscp-gui crossscp-rust-cli)

  if(APPLE)
    set(CROSSSCP_GUI_CLI_DEST "$<TARGET_BUNDLE_DIR:crossscp-gui>/Contents/MacOS")
  else()
    set(CROSSSCP_GUI_CLI_DEST "$<TARGET_FILE_DIR:crossscp-gui>")
  endif()
  add_custom_command(TARGET crossscp-gui POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
      "${CMAKE_SOURCE_DIR}/target/${CROSSSCP_CARGO_TARGET_DIR}/crossscp"
      "${CROSSSCP_GUI_CLI_DEST}/crossscp-cli"
    COMMENT "Staging Rust CLI bridge beside the Qt GUI without colliding with CrossSCP.app executable"
    VERBATIM
  )
endif()

set_target_properties(crossscp-gui PROPERTIES
  OUTPUT_NAME "CrossSCP"
  MACOSX_BUNDLE TRUE
  MACOSX_BUNDLE_BUNDLE_NAME "CrossSCP"
  MACOSX_BUNDLE_GUI_IDENTIFIER "org.crossscp.CrossSCP"
  MACOSX_BUNDLE_ICON_FILE "CrossSCP.icns"
  MACOSX_BUNDLE_INFO_PLIST "${CMAKE_CURRENT_SOURCE_DIR}/packaging/macos/Info.plist.in"
  WIN32_EXECUTABLE TRUE
)

if(APPLE)
  get_filename_component(CROSSSCP_QT_CMAKE_PARENT "${Qt6_DIR}" DIRECTORY)
  get_filename_component(CROSSSCP_QT_LIB_DIR "${CROSSSCP_QT_CMAKE_PARENT}" DIRECTORY)
  set_target_properties(crossscp-gui PROPERTIES
    MACOSX_RPATH TRUE
    BUILD_RPATH "${CROSSSCP_QT_LIB_DIR}"
    INSTALL_RPATH "@executable_path/../Frameworks"
  )
endif()

if(WIN32)
  target_sources(crossscp-gui PRIVATE packaging/windows/crossscp.rc)
endif()

install(TARGETS crossscp-gui
  BUNDLE DESTINATION .
  RUNTIME DESTINATION bin
)
