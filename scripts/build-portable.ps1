# Builds the portable FocusDeck: release exe + `portable` marker, zipped.
# Data then lives in .\data next to the exe (see src-tauri/src/paths.rs).
# Needs WebView2 Runtime on the target machine (built into Windows 11).
#
#   npm run release:portable
#
# Output: release\FocusDeck-<version>-portable.zip
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = (Get-Content package.json -Raw | ConvertFrom-Json).version
Write-Host "FocusDeck $version — portable build"

# Frontend + release exe, no installer.
npx tauri build --no-bundle
if ($LASTEXITCODE -ne 0) { throw "tauri build failed" }

$exe = Join-Path $root "src-tauri\target\release\focusdeck.exe"
if (-not (Test-Path $exe)) { throw "not found: $exe" }

$out = Join-Path $root "release"
$stage = Join-Path $out "FocusDeck-$version-portable"
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item $exe (Join-Path $stage "FocusDeck.exe") -Force
Set-Content -Path (Join-Path $stage "portable") -Value "Данные FocusDeck хранятся в папке data рядом с программой." -Encoding utf8
Copy-Item (Join-Path $root "README.md") $stage -Force
Copy-Item (Join-Path $root "CHANGELOG.md") $stage -Force

$zip = Join-Path $out "FocusDeck-$version-portable.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip
$hash = (Get-FileHash $zip -Algorithm SHA256).Hash
Set-Content -Path "$zip.sha256" -Value "$hash  $(Split-Path -Leaf $zip)" -Encoding ascii
Write-Host "Done: $zip"
Write-Host "SHA-256: $hash"
