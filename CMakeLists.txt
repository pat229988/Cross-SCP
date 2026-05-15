# SPDX-License-Identifier: AGPL-3.0-or-later

cmake_minimum_required(VERSION 3.24)

project(CrossSCP VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

option(CROSSSCP_BUILD_GUI "Build the Qt desktop shell" OFF)
option(CROSSSCP_PACKAGE_DEB "Build a Debian package with CPack on Linux" OFF)

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
set(CPACK_PACKAGE_CONTACT "CrossSCP contributors")
set(CPACK_PACKAGE_HOMEPAGE_URL "https://pat229988.github.io/Cross-SCP/")

if(UNIX AND NOT APPLE AND CROSSSCP_BUILD_GUI)
  install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/packaging/linux/org.crossscp.CrossSCP.desktop"
    DESTINATION share/applications
  )
  install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/packaging/linux/org.crossscp.CrossSCP.metainfo.xml"
    DESTINATION share/metainfo
  )
  install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/apps/crossscp-gui/resources/icons/crossscp.svg"
    DESTINATION share/icons/hicolor/scalable/apps
    RENAME org.crossscp.CrossSCP.svg
  )
  install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/apps/crossscp-gui/resources/icons/crossscp-256.png"
    DESTINATION share/icons/hicolor/256x256/apps
    RENAME org.crossscp.CrossSCP.png
  )
  install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/apps/crossscp-gui/resources/icons/crossscp-512.png"
    DESTINATION share/icons/hicolor/512x512/apps
    RENAME org.crossscp.CrossSCP.png
  )
endif()

if(APPLE)
  set(CPACK_GENERATOR "DragNDrop")
  set(CPACK_DMG_VOLUME_ICON "${CMAKE_CURRENT_SOURCE_DIR}/apps/crossscp-gui/resources/icons/CrossSCP.icns")
elseif(WIN32)
  set(CPACK_GENERATOR "ZIP")
elseif(CROSSSCP_PACKAGE_DEB)
  set(CPACK_GENERATOR "DEB")
  set(CPACK_DEBIAN_PACKAGE_NAME "crossscp")
  set(CPACK_DEBIAN_PACKAGE_MAINTAINER "CrossSCP contributors")
  set(CPACK_DEBIAN_PACKAGE_SECTION "net")
  set(CPACK_DEBIAN_PACKAGE_PRIORITY "optional")
  set(CPACK_DEBIAN_FILE_NAME DEB-DEFAULT)
  set(CPACK_DEBIAN_PACKAGE_DEPENDS "libc6, libstdc++6")
else()
  set(CPACK_GENERATOR "TGZ")
endif()

include(CPack)
