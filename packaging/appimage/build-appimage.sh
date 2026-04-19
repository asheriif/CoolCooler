#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"

HOST_ARCH="$(uname -m)"
if [[ "${HOST_ARCH}" != "x86_64" ]]; then
    echo "Only x86_64 AppImage builds are configured. Host arch: ${HOST_ARCH}" >&2
    exit 1
fi

APPDIR="${APPDIR:-${REPO_ROOT}/target/appimage/AppDir}"
TOOLS_DIR="${REPO_ROOT}/target/appimage/tools"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${REPO_ROOT}/target/appimage/cargo-target}"
DIST_DIR="${REPO_ROOT}/dist"
LINUXDEPLOY="${LINUXDEPLOY:-${TOOLS_DIR}/linuxdeploy-x86_64.AppImage}"
LINUXDEPLOY_URL="https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"

fetch_linuxdeploy() {
    if [[ -x "${LINUXDEPLOY}" ]]; then
        return
    fi

    mkdir -p "${TOOLS_DIR}"
    if command -v curl >/dev/null 2>&1; then
        curl -L --fail -o "${LINUXDEPLOY}" "${LINUXDEPLOY_URL}"
    elif command -v wget >/dev/null 2>&1; then
        wget -O "${LINUXDEPLOY}" "${LINUXDEPLOY_URL}"
    else
        echo "Install curl or wget, or set LINUXDEPLOY=/path/to/linuxdeploy-x86_64.AppImage" >&2
        exit 1
    fi
    chmod +x "${LINUXDEPLOY}"
}

fetch_linuxdeploy

export APPIMAGE_EXTRACT_AND_RUN=1
export LINUXDEPLOY_OUTPUT_VERSION="${LINUXDEPLOY_OUTPUT_VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "${REPO_ROOT}/Cargo.toml" | head -n1)}"
if [[ -z "${LINUXDEPLOY_OUTPUT_VERSION}" ]]; then
    echo "Could not determine AppImage version from workspace Cargo.toml" >&2
    exit 1
fi
export CARGO_TARGET_DIR

rm -rf "${APPDIR}"
mkdir -p "${APPDIR}" "${DIST_DIR}"

cargo build --release --locked -p coolcooler-gui

"${LINUXDEPLOY}" \
    --appdir "${APPDIR}" \
    --executable "${CARGO_TARGET_DIR}/release/coolcooler-gui" \
    --exclude-library "libudev.so.1" \
    --desktop-file "${REPO_ROOT}/packaging/appimage/coolcooler.desktop" \
    --icon-file "${REPO_ROOT}/assets/icon.png" \
    --icon-filename "coolcooler"

install -Dm644 "${REPO_ROOT}/assets/icon.png" \
    "${APPDIR}/usr/share/icons/hicolor/512x512/apps/coolcooler.png"
cp "${REPO_ROOT}/assets/icon.png" "${APPDIR}/coolcooler.png"
ln -sfn "coolcooler.png" "${APPDIR}/.DirIcon"

(
    cd "${DIST_DIR}"
    "${LINUXDEPLOY}" \
    --appdir "${APPDIR}" \
    --exclude-library "libudev.so.1" \
    --desktop-file "${REPO_ROOT}/packaging/appimage/coolcooler.desktop" \
    --icon-file "${REPO_ROOT}/assets/icon.png" \
    --icon-filename "coolcooler" \
    --output appimage
)

echo "AppImage output written to ${DIST_DIR}"
