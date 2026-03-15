#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${NEXUS_OS_RELEASE_BASE_URL:-https://nexus-os.dev/releases}"
BINARY_NAME="nexus-os"

detect_target() {
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

  echo "${os}-${arch}"
}

install_binary() {
  local target url tmpfile install_dir
  target="$(detect_target)"
  case "${target}" in
    linux-x86_64|darwin-arm64) ;;
    *)
      echo "Unsupported release target: ${target}" >&2
      exit 1
      ;;
  esac

  url="${BASE_URL}/${target}/${BINARY_NAME}"
  tmpfile="$(mktemp)"
  install_dir="/usr/local/bin"

  echo "Downloading ${BINARY_NAME} for ${target} from ${url}"
  curl -fsSL "${url}" -o "${tmpfile}"
  chmod +x "${tmpfile}"

  if [ -w "${install_dir}" ]; then
    mv "${tmpfile}" "${install_dir}/${BINARY_NAME}"
  else
    sudo mv "${tmpfile}" "${install_dir}/${BINARY_NAME}"
  fi

  echo "Installed ${BINARY_NAME} to ${install_dir}/${BINARY_NAME}"
}

install_binary
