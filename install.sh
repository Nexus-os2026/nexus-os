#!/usr/bin/env bash
set -euo pipefail

REPO="${NEXUS_OS_GITHUB_REPO:-nexaiceo/nexus-os}"
API_URL="${NEXUS_OS_RELEASE_API:-https://api.github.com/repos/${REPO}/releases/latest}"
INSTALL_PATH="/usr/local/bin/nexus-os"
TMP_DIR=""
DMG_MOUNT=""

cleanup() {
  if [ -n "${DMG_MOUNT}" ] && command -v hdiutil >/dev/null 2>&1; then
    hdiutil detach "${DMG_MOUNT}" >/dev/null 2>&1 || true
  fi
  if [ -n "${TMP_DIR}" ] && [ -d "${TMP_DIR}" ]; then
    rm -rf "${TMP_DIR}"
  fi
}

trap cleanup EXIT

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

detect_platform() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "${os}" in
    Linux) os="linux" ;;
    Darwin) os="darwin" ;;
    *)
      echo "Unsupported OS: ${os}" >&2
      exit 1
      ;;
  esac

  case "${arch}" in
    x86_64|amd64) arch="x86_64" ;;
    arm64|aarch64) arch="arm64" ;;
    *)
      echo "Unsupported architecture: ${arch}" >&2
      exit 1
      ;;
  esac

  printf '%s-%s\n' "${os}" "${arch}"
}

release_json() {
  curl -fsSL \
    -H "Accept: application/vnd.github+json" \
    -H "X-GitHub-Api-Version: 2022-11-28" \
    "${API_URL}"
}

asset_urls() {
  printf '%s' "$1" \
    | grep -oE '"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]+"' \
    | sed -E 's/.*"([^"]+)"/\1/'
}

select_asset_url() {
  local target="$1"
  local url file

  while IFS= read -r url; do
    file="${url##*/}"
    case "${target}" in
      linux-x86_64)
        case "${file}" in
          nexus-os|nexus-os-linux-x86_64|nexus-os_linux_x86_64|nexus-os-linux-amd64|nexus-os_linux_amd64|nexus-os-*-linux-x86_64|nexus-os-*-linux-amd64|nexus-os*.tar.gz|nexus-os*.tgz|nexus-os_*_amd64.deb)
            printf '%s\n' "${url}"
            return 0
            ;;
        esac
        ;;
      darwin-x86_64|darwin-arm64)
        case "${file}" in
          nexus-os-darwin-x86_64|nexus-os_darwin_x86_64|nexus-os-darwin-arm64|nexus-os_darwin_arm64|nexus-os-*-darwin-*|nexus-os*.tar.gz|nexus-os*.tgz|NexusOS_*.dmg)
            printf '%s\n' "${url}"
            return 0
            ;;
        esac
        ;;
    esac
  done

  return 1
}

copy_into_place() {
  local source="$1"
  if [ -w "$(dirname "${INSTALL_PATH}")" ]; then
    install -m 0755 "${source}" "${INSTALL_PATH}"
  else
    need_cmd sudo
    sudo install -m 0755 "${source}" "${INSTALL_PATH}"
  fi
}

extract_tarball_binary() {
  local archive="$1"
  local output_dir="$2"
  mkdir -p "${output_dir}"
  tar -xzf "${archive}" -C "${output_dir}"
  find "${output_dir}" -type f \( -name 'nexus-os' -o -name 'NexusOS' \) | head -n 1
}

extract_deb_binary() {
  local archive="$1"
  local output_dir="$2"
  mkdir -p "${output_dir}"
  if command -v dpkg-deb >/dev/null 2>&1; then
    dpkg-deb -x "${archive}" "${output_dir}" >/dev/null
  else
    need_cmd ar
    local deb_data
    deb_data="$(cd "${output_dir}" && ar t "${archive}" | grep '^data.tar' | head -n 1)"
    if [ -z "${deb_data}" ]; then
      echo "Unable to locate data archive inside ${archive}" >&2
      exit 1
    fi
    (
      cd "${output_dir}"
      ar p "${archive}" "${deb_data}" | tar -xf -
    )
  fi
  find "${output_dir}" -type f -path '*/bin/nexus-os' | head -n 1
}

extract_dmg_binary() {
  local archive="$1"
  need_cmd hdiutil
  DMG_MOUNT="$(hdiutil attach -nobrowse -readonly "${archive}" | awk '/\/Volumes\// {print substr($0, index($0, "/Volumes/"))}' | tail -n 1)"
  if [ -z "${DMG_MOUNT}" ]; then
    echo "Unable to mount ${archive}" >&2
    exit 1
  fi
  find "${DMG_MOUNT}" -type f \( -path '*/Contents/MacOS/nexus-os' -o -path '*/Contents/MacOS/NexusOS' \) | head -n 1
}

install_binary() {
  local target json url asset_name asset_path extracted

  need_cmd curl
  need_cmd install
  need_cmd tar

  target="$(detect_platform)"
  json="$(release_json)"
  url="$(asset_urls "${json}" | select_asset_url "${target}" || true)"
  if [ -z "${url}" ]; then
    echo "No compatible release asset found for ${target} in ${REPO}" >&2
    exit 1
  fi

  TMP_DIR="$(mktemp -d)"
  asset_name="${url##*/}"
  asset_path="${TMP_DIR}/${asset_name}"
  curl -fsSL "${url}" -o "${asset_path}"

  case "${asset_name}" in
    *.tar.gz|*.tgz)
      extracted="$(extract_tarball_binary "${asset_path}" "${TMP_DIR}/extract")"
      ;;
    *.deb)
      extracted="$(extract_deb_binary "${asset_path}" "${TMP_DIR}/extract")"
      ;;
    *.dmg)
      extracted="$(extract_dmg_binary "${asset_path}")"
      ;;
    *)
      extracted="${asset_path}"
      ;;
  esac

  if [ -z "${extracted}" ] || [ ! -f "${extracted}" ]; then
    echo "Unable to locate nexus-os binary in ${asset_name}" >&2
    exit 1
  fi

  copy_into_place "${extracted}"
  echo "Nexus OS installed. Run: nexus-os start"
}

install_binary
