# SPDX-License-Identifier: AGPL-3.0-or-later

cmake_minimum_required(VERSION 3.24)

project(CrossSCP VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

option(CROSSSCP_BUILD_GUI "Build the Qt desktop shell" OFF)

if(CROSSSCP_BUILD_GUI)
  find_package(Qt6 REQUIRED COMPONENTS Core Gui Qml Quick)
  add_subdirectory(apps/crossscp-gui)
else()
  message(STATUS "CrossSCP Qt GUI disabled. Enable with -DCROSSSCP_BUILD_GUI=ON after Qt is installed.")
endif()

set(CPACK_PACKAGE_NAME "CrossSCP")
set(CPACK_PACKAGE_VENDOR "CrossSCP contributors")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "Cross-platform file transfer client")
set(CPACK_PACKAGE_VERSION ${PROJECT_VERSION})
set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_CURRENT_SOURCE_DIR}/LICENSE")
set(CPACK_PACKAGE_CHECKSUM SHA256)

if(APPLE)
  set(CPACK_GENERATOR "DragNDrop")
  set(CPACK_DMG_VOLUME_ICON "${CMAKE_CURRENT_SOURCE_DIR}/apps/crossscp-gui/resources/icons/CrossSCP.icns")
elseif(WIN32)
  set(CPACK_GENERATOR "ZIP")
else()
  set(CPACK_GENERATOR "TGZ")
endif()

include(CPack)
