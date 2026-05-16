#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

APP_ID="org.crossscp.CrossSCP"
APP_NAME="CrossSCP"
REMOVE_CONFIG=0
ASSUME_YES=0
DRY_RUN=0

usage() {
  cat <<'USAGE'
Usage: scripts/uninstall-linux.sh [options]

Completely removes CrossSCP Linux beta installs and stale desktop launchers.
Useful when switching between RPM/DEB and Flatpak builds, especially on KDE.

Options:
  --remove-config   Also remove user config/cache/data for CrossSCP
  -y, --yes         Do not prompt before removal
  --dry-run         Print what would be removed without changing anything
  -h, --help        Show this help

Examples:
  bash scripts/uninstall-linux.sh
  bash scripts/uninstall-linux.sh --remove-config
  bash scripts/uninstall-linux.sh --dry-run
USAGE
}

log() { printf '%s\n' "$*"; }
warn() { printf 'Warning: %s\n' "$*" >&2; }

run() {
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    printf '[dry-run]'
    printf ' %q' "$@"
    printf '\n'
  else
    "$@"
  fi
}

confirm() {
  if [[ "${ASSUME_YES}" -eq 1 || "${DRY_RUN}" -eq 1 ]]; then
    return 0
  fi
  printf 'Remove CrossSCP packages, Flatpaks, and stale desktop launchers? [y/N] '
  read -r answer
  case "${answer}" in
    y|Y|yes|YES) return 0 ;;
    *) log "Cancelled."; exit 0 ;;
  esac
}

refresh_desktop_cache() {
  log "Refreshing desktop launcher cache..."
  if command -v update-desktop-database >/dev/null 2>&1; then
    [[ -d "${HOME}/.local/share/applications" ]] && run update-desktop-database "${HOME}/.local/share/applications" || true
    [[ -d "${HOME}/.local/share/flatpak/exports/share/applications" ]] && run update-desktop-database "${HOME}/.local/share/flatpak/exports/share/applications" || true
    [[ -d "/var/lib/flatpak/exports/share/applications" ]] && run update-desktop-database "/var/lib/flatpak/exports/share/applications" || true
  fi

  if command -v kbuildsycoca6 >/dev/null 2>&1; then
    run kbuildsycoca6 --noincremental || true
  elif command -v kbuildsycoca5 >/dev/null 2>&1; then
    run kbuildsycoca5 --noincremental || true
  fi

  if command -v xdg-desktop-menu >/dev/null 2>&1; then
    run xdg-desktop-menu forceupdate || true
  fi
}

remove_flatpak() {
  if ! command -v flatpak >/dev/null 2>&1; then
    return 0
  fi

  if flatpak info --user "${APP_ID}" >/dev/null 2>&1; then
    log "Removing user Flatpak: ${APP_ID}"
    run flatpak uninstall --user -y "${APP_ID}"
  fi

  if flatpak info --system "${APP_ID}" >/dev/null 2>&1; then
    log "Removing system Flatpak: ${APP_ID}"
    run sudo flatpak uninstall -y "${APP_ID}"
  fi
}

remove_native_packages() {
  if command -v rpm >/dev/null 2>&1; then
    rpm_packages=()
    while IFS= read -r package; do
      [[ -n "${package}" ]] && rpm_packages+=("${package}")
    done < <(rpm -qa | grep -Ei '^(crossscp|CrossSCP)' || true)
    if [[ "${#rpm_packages[@]}" -gt 0 ]]; then
      log "Removing RPM packages: ${rpm_packages[*]}"
      if command -v dnf >/dev/null 2>&1; then
        run sudo dnf remove -y "${rpm_packages[@]}"
      else
        run sudo rpm -e "${rpm_packages[@]}"
      fi
    fi
  fi

  if command -v dpkg-query >/dev/null 2>&1; then
    deb_packages=()
    while IFS= read -r package; do
      [[ -n "${package}" ]] && deb_packages+=("${package}")
    done < <(dpkg-query -W -f='${binary:Package}\n' 2>/dev/null | grep -Ei '^(crossscp|CrossSCP)$' || true)
    if [[ "${#deb_packages[@]}" -gt 0 ]]; then
      log "Removing DEB packages: ${deb_packages[*]}"
      run sudo apt remove -y "${deb_packages[@]}"
    fi
  fi
}

remove_desktop_entries() {
  log "Removing stale non-Flatpak CrossSCP desktop entries..."
  local entries=(
    "${HOME}/.local/share/applications/${APP_ID}.desktop"
    "${HOME}/.local/share/applications/CrossSCP.desktop"
    "${HOME}/.local/share/applications/crossscp.desktop"
    "/usr/share/applications/${APP_ID}.desktop"
    "/usr/share/applications/CrossSCP.desktop"
    "/usr/share/applications/crossscp.desktop"
  )

  local path
  for path in "${entries[@]}"; do
    if [[ -e "${path}" ]]; then
      if [[ "${path}" == /usr/* ]]; then
        run sudo rm -f "${path}"
      else
        run rm -f "${path}"
      fi
    fi
  done
}

remove_config() {
  if [[ "${REMOVE_CONFIG}" -ne 1 ]]; then
    return 0
  fi

  log "Removing user CrossSCP config/cache/data..."
  local paths=(
    "${HOME}/.config/CrossSCP"
    "${HOME}/.config/crossscp"
    "${HOME}/.local/share/CrossSCP"
    "${HOME}/.local/share/crossscp"
    "${HOME}/.cache/CrossSCP"
    "${HOME}/.cache/crossscp"
  )
  local path
  for path in "${paths[@]}"; do
    [[ -e "${path}" ]] && run rm -rf "${path}"
  done
}

show_remaining() {
  log "Remaining CrossSCP-related entries, if any:"
  if command -v flatpak >/dev/null 2>&1; then
    flatpak list | grep -i crossscp || true
  fi
  if command -v rpm >/dev/null 2>&1; then
    rpm -qa | grep -i crossscp || true
  fi
  if command -v dpkg-query >/dev/null 2>&1; then
    dpkg-query -W -f='${binary:Package}\n' 2>/dev/null | grep -i crossscp || true
  fi
  find "${HOME}/.local/share/applications" "/usr/share/applications" \
    "${HOME}/.local/share/flatpak/exports/share/applications" "/var/lib/flatpak/exports/share/applications" \
    \( -iname '*crossscp*.desktop' -o -iname '*CrossSCP*.desktop' \) 2>/dev/null || true
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --remove-config) REMOVE_CONFIG=1 ;;
    -y|--yes) ASSUME_YES=1 ;;
    --dry-run) DRY_RUN=1 ;;
    -h|--help) usage; exit 0 ;;
    *) warn "Unknown option: $1"; usage; exit 2 ;;
  esac
  shift
done

if [[ "$(uname -s)" != "Linux" && "${DRY_RUN}" -ne 1 ]]; then
  warn "This uninstall script is intended for Linux."
  exit 1
fi

confirm
remove_flatpak
remove_native_packages
remove_desktop_entries
remove_config
refresh_desktop_cache
show_remaining

log "Done. If KDE/GNOME still shows an old launcher, log out and log back in."
