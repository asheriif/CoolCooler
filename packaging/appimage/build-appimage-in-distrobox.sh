#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"
CONTAINER_NAME="${COOLCOOLER_DISTROBOX:-CoolCoolerAppImage}"

if [[ -n "${container:-}" || -n "${DISTROBOX_ENTER_PATH:-}" ]]; then
    exec "${SCRIPT_DIR}/build-appimage.sh"
fi

quoted_repo="$(printf "%q" "${REPO_ROOT}")"
exec distrobox enter "${CONTAINER_NAME}" -- bash -lc \
    "cd ${quoted_repo} && { [ ! -f \"\$HOME/.cargo/env\" ] || . \"\$HOME/.cargo/env\"; } && exec packaging/appimage/build-appimage.sh"
