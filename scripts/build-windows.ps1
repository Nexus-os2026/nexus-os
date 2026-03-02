# For building on Windows
Write-Host "Building NexusOS for Windows..."
cargo build --release
Set-Location app
npm install
npm run tauri build
Write-Host "Installer at: app\\src-tauri\\target\\release\\bundle\\nsis\\"
Write-Host "NexusOS_1.0.0_x64-setup.exe is ready!"
