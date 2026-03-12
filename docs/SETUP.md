# Nexus OS Setup Guide

Detailed platform-specific instructions for building and running Nexus OS.

## Prerequisites

| Tool | Version | Required | Purpose |
|------|---------|----------|---------|
| Rust | 1.94+ stable | Yes | Backend compilation |
| Node.js | 22+ | Yes | Frontend build |
| npm | 10+ | Yes | Package management |
| Ollama | Latest | Recommended | Local LLM inference |
| Python | 3.11+ | Optional | Voice assistant |

## Platform Setup

### Linux (Debian/Ubuntu 22.04+)

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install Node.js 22
curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash -
sudo apt-get install -y nodejs

# Install Tauri system dependencies
sudo apt-get install -y \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libssl-dev \
  pkg-config \
  build-essential

# Add WASM target for sandbox
rustup target add wasm32-wasip1
```

### Linux (Fedora 38+)

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install Node.js 22
sudo dnf install -y nodejs

# Install Tauri system dependencies
sudo dnf install -y \
  gtk3-devel \
  webkit2gtk4.1-devel \
  libappindicator-gtk3-devel \
  librsvg2-devel \
  openssl-devel \
  pkg-config

rustup target add wasm32-wasip1
```

### Linux (Arch)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

sudo pacman -S nodejs npm webkit2gtk-4.1 gtk3 libappindicator-gtk3 \
  librsvg openssl pkg-config base-devel

rustup target add wasm32-wasip1
```

### macOS

```bash
# Install Xcode Command Line Tools
xcode-select --install

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install Node.js (via Homebrew)
brew install node@22

rustup target add wasm32-wasip1
```

macOS includes WebKit natively — no additional Tauri dependencies needed.

### Windows

1. Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) — select "Desktop development with C++"
2. Install [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (included in Windows 11)
3. Install Rust from [rustup.rs](https://rustup.rs)
4. Install [Node.js 22+](https://nodejs.org)
5. Open a new terminal and run:

```powershell
rustup target add wasm32-wasip1
```

## Build Nexus OS

```bash
git clone https://gitlab.com/nexaiceo/nexus-os.git
cd nexus-os

# Build all Rust crates
cargo build --workspace

# Build the desktop frontend
cd app
npm install
npm run build

# Run the desktop app in development mode
cargo tauri dev
```

### Verify the Build

```bash
# Format check
cargo fmt --all -- --check

# Lint check
cargo clippy --workspace --all-targets -- -D warnings

# Run all 1,941 tests
cargo test --workspace
```

## Ollama Setup (Recommended)

Ollama enables local LLM inference — your data stays on your machine.

### Install Ollama

**Linux:**
```bash
curl -fsSL https://ollama.ai/install.sh | sh
```

**macOS:**
```bash
brew install ollama
```

**Windows:**
Download from [ollama.ai](https://ollama.ai) and run the installer.

### Start Ollama

```bash
ollama serve
```

Ollama runs on `http://localhost:11434` by default.

### Download a Model

The Setup Wizard will recommend models based on your hardware. To download manually:

```bash
# Small model (2GB, good for 8GB RAM systems)
ollama pull qwen3.5:4b

# Medium model (4GB, good for 16GB RAM / 6GB VRAM)
ollama pull qwen3.5:8b

# Large model (8GB, needs 16GB+ VRAM)
ollama pull qwen3.5:14b
```

### Verify Ollama Connection

```bash
# Check Ollama is running
curl http://localhost:11434/api/tags

# Test inference
curl http://localhost:11434/api/generate -d '{"model": "qwen3.5:4b", "prompt": "Hello"}'
```

When you launch Nexus OS, the Setup Wizard will automatically detect Ollama and guide you through model selection.

## Python Voice Setup (Optional)

The voice assistant requires Python dependencies:

```bash
cd voice
python3 -m venv .venv
source .venv/bin/activate  # Linux/macOS
# .venv\Scripts\activate   # Windows

pip install -r requirements.txt

# Verify
python3 -m pytest -v
```

## Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `OLLAMA_HOST` | `http://localhost:11434` | Ollama server URL |
| `NEXUS_LOG` | `info` | Log level (trace, debug, info, warn, error) |
| `CARGO_BUILD_JOBS` | CPU count | Parallel compilation jobs (use 2 for low-memory systems) |

## Troubleshooting

### Build Errors

**"pkg-config not found"**
```bash
# Debian/Ubuntu
sudo apt-get install pkg-config

# Fedora
sudo dnf install pkg-config

# macOS
brew install pkg-config
```

**"webkit2gtk not found"**
```bash
# Debian/Ubuntu — install the 4.1 version specifically
sudo apt-get install libwebkit2gtk-4.1-dev
```

**"linker cc not found" (Windows)**
Install Visual Studio Build Tools with the C++ workload.

**Out of memory during compilation**
```bash
# Limit parallel jobs
export CARGO_BUILD_JOBS=2
cargo build --workspace
```

### Ollama Issues

**"connection refused" on port 11434**
```bash
# Start the Ollama server
ollama serve

# Or check if it's running
curl http://localhost:11434
```

**Model download stalls**
```bash
# Check disk space — models need several GB
df -h

# Retry the download
ollama pull qwen3.5:4b
```

**GPU not detected by Ollama**
- NVIDIA: Install CUDA toolkit and nvidia-container-toolkit
- AMD: Install ROCm
- Apple Silicon: Ollama uses Metal automatically — no extra setup

### Frontend Issues

**"npm install fails"**
```bash
# Clear cache and retry
rm -rf app/node_modules app/package-lock.json
cd app && npm install
```

**"vite build fails"**
```bash
# Ensure Node.js 22+
node --version

# Clean build
cd app && npm run build
```

### Desktop App Issues

**"cargo tauri dev" fails to start**
- Ensure the frontend is built first: `cd app && npm run build`
- Check that all Tauri system dependencies are installed for your platform
- On Linux, ensure a display server is running (X11 or Wayland)
