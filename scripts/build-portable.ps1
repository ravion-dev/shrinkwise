param(
  [switch]$IncludeTools,
  [string]$ToolsSource = "",
  [switch]$Zip
)

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
  Write-Host "==> On yuz derleniyor" -ForegroundColor Cyan
  npm run build
  if ($LASTEXITCODE -ne 0) { throw "npm run build basarisiz" }

  Write-Host "==> Rust surumu derleniyor (release, bundle yok)" -ForegroundColor Cyan
  npm run tauri build -- --no-bundle
  if ($LASTEXITCODE -ne 0) { throw "tauri build basarisiz" }
} finally {
  Pop-Location
}

$exe = Join-Path $root "src-tauri\target\release\shrinkwise.exe"
if (-not (Test-Path $exe)) {
  $exe = Join-Path $root "src-tauri\target\release\ShrinkWise.exe"
}
if (-not (Test-Path $exe)) {
  throw "Derlenmis exe bulunamadi: src-tauri\target\release"
}

$outDir = Join-Path $root "dist-portable\ShrinkWise"
if (Test-Path $outDir) {
  Remove-Item $outDir -Recurse -Force
}
New-Item -ItemType Directory -Path $outDir -Force | Out-Null

Copy-Item $exe (Join-Path $outDir "ShrinkWise.exe") -Force

if (-not $ToolsSource -and (Test-Path (Join-Path $root "tools\bin\ffmpeg.exe"))) {
  $ToolsSource = (Resolve-Path (Join-Path $root "tools\bin")).Path
}
if (-not $ToolsSource -and (Test-Path (Join-Path $root "..\app\tools\bin"))) {
  $ToolsSource = (Resolve-Path (Join-Path $root "..\app\tools\bin")).Path
}

$toolBin = Join-Path $outDir "tools\bin"
New-Item -ItemType Directory -Path $toolBin -Force | Out-Null

if ($IncludeTools) {
  if (-not $ToolsSource -or -not (Test-Path $ToolsSource)) {
    throw "Arac kaynagi bulunamadi. -ToolsSource ile ffmpeg.exe klasorunu belirtin."
  }
  $names = @("ffmpeg.exe", "ffprobe.exe", "7z.exe", "7z.dll", "7zr.exe")
  $copied = 0
  foreach ($name in $names) {
    $srcFile = Join-Path $ToolsSource $name
    if (Test-Path $srcFile) {
      Copy-Item $srcFile (Join-Path $toolBin $name) -Force
      $copied++
    }
  }
  # 7z.exe tek basina calismaz; yardimci DLL varsa her zaman ekle.
  $sevenZipDll = Join-Path $ToolsSource "7z.dll"
  if ((Test-Path (Join-Path $toolBin "7z.exe")) -and (Test-Path $sevenZipDll) -and
      -not (Test-Path (Join-Path $toolBin "7z.dll"))) {
    Copy-Item $sevenZipDll (Join-Path $toolBin "7z.dll") -Force
  }
  foreach ($sub in @("realesrgan-ncnn-vulkan", "waifu2x-ncnn-vulkan")) {
    $srcDir = Join-Path $ToolsSource $sub
    if (Test-Path $srcDir -PathType Container) {
      Copy-Item -Path $srcDir -Destination $toolBin -Recurse -Force
      $copied++
    }
  }
  if ($copied -eq 0) {
    throw "$ToolsSource icinde ffmpeg.exe/ffprobe.exe bulunamadi."
  }
  Write-Host "==> $copied arac pakete eklendi" -ForegroundColor Green
} else {
  Write-Host "==> Araclar eklenmedi; kullanici Ayarlar > Araclar'dan kurabilir." -ForegroundColor Yellow
}

$readme = @"
ShrinkWise v3.0.0 - Portable Build
==================================

Double-click ShrinkWise.exe to run.
No installation, registry entries or AppData usage.

Folders:
  settings.json   Settings (created on first launch)
  tools\bin       FFmpeg, FFprobe, 7-Zip, Ghostscript, qpdf and AI Upscalers
  temp\           Temporary working files
  logs\           Execution logs

If external tools (FFmpeg, Real-ESRGAN, etc.) are missing:
  Open the app > Settings > Tools > "One-click install".
  The SHA-256 checksum is verified automatically during download.
"@
Set-Content -Path (Join-Path $outDir "README.txt") -Value $readme -Encoding UTF8

$notice = Join-Path $root "THIRD-PARTY.txt"
if (Test-Path $notice) {
  Copy-Item $notice (Join-Path $outDir "THIRD-PARTY.txt") -Force
}

$size = (Get-ChildItem $outDir -Recurse -File | Measure-Object -Property Length -Sum).Sum
Write-Host ("==> Hazir: {0} ({1:N1} MB)" -f $outDir, ($size / 1MB)) -ForegroundColor Green

if ($Zip) {
  $zipPath = Join-Path $root "dist-portable\ShrinkWise-portable.zip"
  if (Test-Path $zipPath) { Remove-Item $zipPath -Force }
  Compress-Archive -Path (Join-Path $outDir "*") -DestinationPath $zipPath -CompressionLevel Optimal
  Write-Host ("==> Zip: {0}" -f $zipPath) -ForegroundColor Green
}
