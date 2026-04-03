# Build Virgo zk_proof on Windows with MinGW-w64 g++ (not MSVC).
# Requires: Git, CMake, MinGW g++ on PATH; git submodules; Virgo/lib/*.a
#
# Usage:
#   .\scripts\build_virgo.ps1
#   .\scripts\build_virgo.ps1 -VirgoRoot D:\path\to\Virgo
#   .\scripts\build_virgo.ps1 -Gxx 'C:\msys64\mingw64\bin\g++.exe'

param(
  [string]$VirgoRoot = "",
  [string]$Gxx = "",
  [string]$BuildDir = "build-mingw"
)

$ErrorActionPreference = "Stop"

# Avoid "[" in double-quoted strings (PowerShell parses [name] as type); avoid "\b" in paths (backspace in double quotes).
function LogInfo([string]$Message) {
  Write-Host ('[build_virgo] ' + $Message)
}

function Get-RepoRoot {
  if ($PSScriptRoot) {
    return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
  }
  return (Resolve-Path (Join-Path (Get-Location).Path "..")).Path
}

$repoRoot = Get-RepoRoot
if ([string]::IsNullOrWhiteSpace($VirgoRoot)) {
  $VirgoRoot = Join-Path $repoRoot "Virgo"
}
$VirgoRoot = (Resolve-Path $VirgoRoot).Path

LogInfo ('VirgoRoot=' + $VirgoRoot)

$gitDir = Join-Path $VirgoRoot ".git"
if (Test-Path $gitDir) {
  LogInfo 'git submodule update --init --recursive ...'
  Push-Location $VirgoRoot
  try {
    git submodule update --init --recursive
    if ($LASTEXITCODE -ne 0) {
      Write-Error 'git submodule failed. Run in Virgo: git submodule update --init --recursive'
    }
  }
  finally {
    Pop-Location
  }
}
else {
  Write-Warning '[build_virgo] No Virgo/.git; skipped submodule.'
}

$floInc = Join-Path $VirgoRoot 'include\flo-shani-aesni'
if (-not (Test-Path $floInc)) {
  Write-Error ('Missing: ' + $floInc + ' — run: git -C Virgo submodule update --init --recursive')
}

$libFlo = Join-Path $VirgoRoot 'lib\libflo-shani.a'
$xkcpSub = Join-Path $VirgoRoot 'include\XKCP\Standalone\CompactFIPS202\C\Keccak-more-compact.c'
if (-not (Test-Path $libFlo)) {
  Write-Error ('Missing: ' + $libFlo + ' — copy from upstream Virgo lib/ or build flo-shani-aesni.')
}
if (-not (Test-Path $xkcpSub)) {
  Write-Error ('Missing: ' + $xkcpSub + ' — run git submodule in Virgo (XKCP). SHA3 is embedded; libXKCP.a not required.')
}

$candidate = $null
if (-not [string]::IsNullOrWhiteSpace($Gxx)) {
  if (Test-Path $Gxx) { $candidate = (Resolve-Path $Gxx).Path }
}
if (-not $candidate) {
  $w = Get-Command g++ -ErrorAction SilentlyContinue
  if ($w) { $candidate = $w.Source }
}
if (-not $candidate) {
  # Single-quoted paths: in double quotes \b is BACKSPACE
  foreach ($base in @(
      'C:\msys64\mingw64\bin\g++.exe',
      'C:\msys64\ucrt64\bin\g++.exe',
      'C:\ProgramData\mingw64\mingw64\bin\g++.exe'
    )) {
    if (Test-Path $base) {
      $candidate = $base
      break
    }
  }
}
if (-not $candidate) {
  Write-Error 'MinGW g++ not found. Install MSYS2 mingw-w64-x86_64-gcc or use -Gxx path.'
}
LogInfo ('CXX=' + $candidate)

$gccPath = $null
if ($candidate -match 'g\+\+\.exe$') {
  $gccPath = $candidate -replace 'g\+\+\.exe$', 'gcc.exe'
}
elseif ($candidate -match 'g\+\+$') {
  $gccPath = $candidate -replace 'g\+\+$', 'gcc'
}
if ($gccPath -and -not (Test-Path $gccPath)) {
  Write-Warning ('[build_virgo] No matching gcc at ' + $gccPath + '; CMake CXX only')
  $gccPath = $null
}

$buildPath = Join-Path $VirgoRoot $BuildDir
if (-not (Test-Path $buildPath)) {
  New-Item -ItemType Directory -Path $buildPath | Out-Null
}

$cmakeArgs = @(
  '-S', $VirgoRoot,
  '-B', $buildPath,
  '-G', 'MinGW Makefiles',
  '-DCMAKE_BUILD_TYPE=Release',
  ('-DCMAKE_CXX_COMPILER=' + $candidate)
)
if ($gccPath) {
  $cmakeArgs += ('-DCMAKE_C_COMPILER=' + $gccPath)
}

LogInfo ('cmake ' + ($cmakeArgs -join ' '))
& cmake @cmakeArgs
if ($LASTEXITCODE -ne 0) {
  Write-Error 'CMake configure failed.'
}

LogInfo 'cmake --build ... --target zk_proof'
& cmake --build $buildPath --target zk_proof -j 8
if ($LASTEXITCODE -ne 0) {
  Write-Error 'Build failed.'
}

$outExe = Join-Path $buildPath 'zk_proof.exe'
if (Test-Path $outExe) {
  Write-Host ''
  LogInfo ('OK: ' + $outExe)
  Write-Host 'Run from repo root: .\scripts\run_virgo.ps1'
}
else {
  Write-Warning ('[build_virgo] Not found: ' + $outExe)
}
