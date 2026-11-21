$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$nodeDir = "C:\Program Files\nodejs"
if (Test-Path $nodeDir) { $env:Path = "$nodeDir;$env:Path" }
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $cargoBin) { $env:Path = "$cargoBin;$env:Path" }

if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
  throw "npm bulunamadi. Node.js kurun veya PATH'e ekleyin."
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  throw "cargo bulunamadi. Rust kurun veya %USERPROFILE%\.cargo\bin klasorunu PATH'e ekleyin."
}

$userDir = $env:USERPROFILE
$userDirLower = $userDir.Substring(0, 1).ToLower() + $userDir.Substring(1)
$userDirFwd = $userDir -replace '\\', '/'
$userDirLowerFwd = $userDirLower -replace '\\', '/'
$env:RUSTFLAGS = "--remap-path-prefix=$userDir=/user --remap-path-prefix=$userDirLower=/user --remap-path-prefix=$userDirFwd=/user --remap-path-prefix=$userDirLowerFwd=/user -C link-arg=/PDBALTPATH:shrinkwise.pdb"

Push-Location $root
try {
  Write-Host "==> Windows NSIS installer derleniyor (ShrinkWise + Uninstaller)..." -ForegroundColor Cyan
  npm run tauri build -- --bundles nsis
  if ($LASTEXITCODE -ne 0) { throw "tauri build (nsis) basarisiz" }
} finally {
  Pop-Location
}

$nsisDir = Join-Path $root "src-tauri\target\release\bundle\nsis"
$installer = Get-ChildItem -Path $nsisDir -Filter "*-setup.exe" -ErrorAction SilentlyContinue |
  Sort-Object LastWriteTime -Descending |
  Select-Object -First 1

if (-not $installer) {
  throw "NSIS installer bulunamadi: $nsisDir"
}

$outDir = Join-Path $root "dist-installer"
if (-not (Test-Path $outDir)) {
  New-Item -ItemType Directory -Path $outDir -Force | Out-Null
}
$dest = Join-Path $outDir $installer.Name
Copy-Item $installer.FullName $dest -Force

Write-Host ("==> Installer hazir: {0} ({1:N1} MB)" -f $dest, ($installer.Length / 1MB)) -ForegroundColor Green
