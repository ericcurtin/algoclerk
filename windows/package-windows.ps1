# Build a Windows zip of AlgoClerk (run in PowerShell on Windows).
#   rustup target add x86_64-pc-windows-msvc
#   windows/package-windows.ps1
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches.Groups[1].Value
cargo build --release -p algoclerk-app --bin algoclerk
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { "target" }
New-Item -ItemType Directory -Force -Path dist | Out-Null
$stage = "dist/AlgoClerk-$version-windows-x86_64"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item "$target/release/algoclerk.exe" "$stage/AlgoClerk.exe"
Copy-Item LICENSE, NOTICE, README.md, CREDITS.md $stage
Compress-Archive -Path "$stage/*" -DestinationPath "$stage.zip" -Force
Write-Host "Built $stage.zip"
