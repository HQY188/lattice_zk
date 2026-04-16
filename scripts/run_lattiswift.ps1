<#
Lattice PCS（lattice_poly_commit）+ GKR 正确性：通过 cargo test 跑单测 gkr_correctness_lattiswift_case。

【逐步在做什么】
1) 解析仓库根；默认 DataDir =仓库/data。
2) （Windows）尝试加载 vcvars64.bat、自动找 LIBCLANG_PATH，以便 mpi-sys/bindgen 能编译。
3) 按开关设置 Lattice并行相关环境变量（LATTICE_MLE_COMMIT_*、RAYON_NUM_THREADS）。
4) 固定跑两个用例：circuit_m31.txt、circuit_babybear.txt（及对应 witness），设置   GKR_TEST_FIELD_TYPE、GKR_TEST_CIRCUIT_PATH、GKR_TEST_WITNESS_PATH。
5) 在仓库根执行 cargo test -p gkr ... gkr_correctness_lattiswift_case（见 gkr/src/tests/gkr_correctness.rs）。

【用的电路】与 Libra 相同来源的 data下 Keccak 基准二进制（gkr/src/utils.rs 中 KECCAK_M31_CIRCUIT / KECCAK_BABYBEAR_CIRCUIT）；
本脚本只选 m31、babybear 两种域（Lattice PCS 编码与 SIMD 路径限制）。PCS 类型在测试代码里为 PolynomialCommitmentType::Lattice。
#>
param(
  [string]$DataDir = "",
  [switch]$NoCapture,
  [switch]$IncludeEmpty,
  # Use `cargo test` without `--release` (faster compile; still needs mpi-sys + libclang once).
  [switch]$Dev,
  # Do not run vcvars64 (use when you already launched from "x64 Native Tools Command Prompt").
  [switch]$SkipVcVars,
  # Turn off lattice MLE Rayon paths (matrix T, row commit, compute_u, dot_product); for A/B timing.
  [switch]$SequentialLattice,
  # Rayon thread count for lattice MLE pool and global pool (default 8; overrides parent-shell RAYON_NUM_THREADS).
  [int]$LatticeThreads = 8
)

$ErrorActionPreference = "Stop"

function Get-RepoRoot {
  if ($PSScriptRoot) {
    return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
  }
  return (Resolve-Path (Join-Path (Get-Location).Path "..")).Path
}

function Find-VcVars64Bat {
  $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
  if (Test-Path -LiteralPath $vswhere) {
    $install = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
    if ($install) {
      $bat = Join-Path $install "VC\Auxiliary\Build\vcvars64.bat"
      if (Test-Path -LiteralPath $bat) {
        return $bat
      }
    }
  }
  foreach ($root in @($env:ProgramFiles, ${env:ProgramFiles(x86)})) {
    if ([string]::IsNullOrWhiteSpace($root)) { continue }
    foreach ($ed in @("Community", "Professional", "Enterprise", "BuildTools")) {
      $bat = Join-Path $root "Microsoft Visual Studio\2022\$ed\VC\Auxiliary\Build\vcvars64.bat"
      if (Test-Path -LiteralPath $bat) {
        return $bat
      }
    }
  }
  return $null
}

# bindgen (mpi-sys) must parse MS MPI headers: needs Windows SDK + MSVC include paths (INCLUDE, etc.).
function Import-MsvcDevEnvironment {
  if ($SkipVcVars.IsPresent) {
    Write-Host "[lattiswift] -SkipVcVars: not running vcvars64.bat"
    return
  }
  if ($env:LATTISWIFT_SKIP_VCVARS) {
    Write-Host "[lattiswift] LATTISWIFT_SKIP_VCVARS set; not running vcvars64.bat"
    return
  }
  if ($env:INCLUDE -and $env:VCINSTALLDIR) {
    Write-Host "[lattiswift] INCLUDE and VCINSTALLDIR already set; skipping vcvars64.bat"
    return
  }
  $vcvars = Find-VcVars64Bat
  if (-not $vcvars) {
    Write-Warning @"
[lattiswift] Could not find vcvars64.bat. mpi-sys bindgen often needs MSVC + Windows SDK on PATH/INCLUDE.
  Install VS 2022 "Desktop development with C++", or run this script from "x64 Native Tools Command Prompt for VS 2022".
"@
    return
  }
  Write-Host ("[lattiswift] loading MSVC/WinSDK env: {0}" -f $vcvars)
  $prevEap = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  try {
    # `call` is required so vcvars64's SET persists in this cmd.exe process before `set` runs.
    $lines = cmd.exe /c "call `"$vcvars`" >nul 2>&1 && set" 2>&1
    foreach ($line in $lines) {
      if ($line -is [System.Management.Automation.ErrorRecord]) {
        continue
      }
      $s = [string]$line
      $eq = $s.IndexOf('=')
      if ($eq -lt 1) { continue }
      $name = $s.Substring(0, $eq)
      $value = $s.Substring($eq + 1)
      if ($name -match '^[A-Za-z_][A-Za-z0-9_]*$') {
        Set-Item -Path "env:$name" -Value $value
      }
    }
  }
  finally {
    $ErrorActionPreference = $prevEap
  }
}

# mpi-sys (bindgen) needs libclang. If LIBCLANG_PATH is unset, try common Windows locations.
function Set-LibClangPathIfNeeded {
  if ($env:LIBCLANG_PATH) {
    $dll = Join-Path $env:LIBCLANG_PATH "libclang.dll"
    if (Test-Path -LiteralPath $dll) {
      Write-Host ("[lattiswift] LIBCLANG_PATH already set: {0}" -f $env:LIBCLANG_PATH)
      return
    }
    Write-Warning ("[lattiswift] LIBCLANG_PATH is set but libclang.dll not found under it; will search.")
  }

  $candidates = @()
  foreach ($root in @($env:ProgramFiles, ${env:ProgramFiles(x86)})) {
    if ([string]::IsNullOrWhiteSpace($root)) { continue }
    $candidates += (Join-Path $root "LLVM\bin")
    foreach ($ed in @("Community", "Professional", "Enterprise", "BuildTools")) {
      $candidates += (Join-Path $root "Microsoft Visual Studio\2022\$ed\VC\Tools\Llvm\x64\bin")
      $candidates += (Join-Path $root "Microsoft Visual Studio\2022\$ed\VC\Tools\Llvm\bin")
    }
  }

  foreach ($dir in $candidates) {
    if ([string]::IsNullOrWhiteSpace($dir)) { continue }
    $dll = Join-Path $dir "libclang.dll"
    if (Test-Path -LiteralPath $dll) {
      $env:LIBCLANG_PATH = $dir
      Write-Host ("[lattiswift] auto LIBCLANG_PATH = {0}" -f $dir)
      return
    }
  }

  Write-Warning @"
[lattiswift] Could not find libclang.dll. mpi-sys bindgen will fail unless you:
  - Install LLVM for Windows (https://releases.llvm.org/) and set LIBCLANG_PATH to its bin folder, or
  - Install VS 2022 with C++ 'LLVM tools' / Clang, or
  - Set LIBCLANG_PATH manually before running this script.
"@
}

# Avoid PowerShell treating cargo stderr as ErrorRecord; return exit code.
function Invoke-CargoInRepo {
  param(
    [Parameter(Mandatory = $true)][string]$RepoRoot,
    [Parameter(Mandatory = $true)][string[]]$Arguments
  )
  $prevEap = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  try {
    Push-Location $RepoRoot
    $raw = & cargo @Arguments 2>&1
    $code = $LASTEXITCODE
    $raw | ForEach-Object {
      if ($_ -is [System.Management.Automation.ErrorRecord]) {
        $_.ToString()
      }
      else {
        $_
      }
    } | ForEach-Object { Write-Host $_ }
    return $code
  }
  finally {
    Pop-Location
    $ErrorActionPreference = $prevEap
  }
}

$repoRoot = Get-RepoRoot
if ([string]::IsNullOrWhiteSpace($DataDir)) {
  $DataDir = Join-Path $repoRoot "data"
}
$DataDir = (Resolve-Path $DataDir).Path

if (!(Test-Path $DataDir)) {
  throw "data directory not found: $DataDir"
}

Import-MsvcDevEnvironment
Set-LibClangPathIfNeeded

# Lattice PCS parallel paths (lattice_poly_commit): same env as manual runs.
# - LATTICE_MLE_COMMIT_PARALLEL: any value except 0/false/no enables (when size thresholds match).
# - LATTICE_MLE_COMMIT_THREADS: dedicated Rayon pool for those routines (default from -LatticeThreads).
# - RAYON_NUM_THREADS: global Rayon pool (other crates / fallback).
function Set-LattiswiftParallelEnv {
  if ($SequentialLattice.IsPresent) {
    $env:LATTICE_MLE_COMMIT_PARALLEL = "0"
    Remove-Item Env:LATTICE_MLE_COMMIT_THREADS -ErrorAction SilentlyContinue
    Remove-Item Env:RAYON_NUM_THREADS -ErrorAction SilentlyContinue
    Write-Host "[lattiswift] SequentialLattice: LATTICE_MLE_COMMIT_PARALLEL=0; unset COMMIT_THREADS / RAYON_NUM_THREADS"
    return
  }
  $n = $LatticeThreads
  if ($n -lt 1) { $n = 1 }
  $env:LATTICE_MLE_COMMIT_PARALLEL = "1"
  $env:LATTICE_MLE_COMMIT_THREADS = "$n"
  $env:RAYON_NUM_THREADS = "$n"
  Write-Host ("[lattiswift] parallel: LATTICE_MLE_COMMIT_PARALLEL=1, LATTICE_MLE_COMMIT_THREADS={0}, RAYON_NUM_THREADS={0}" -f $n)
}

Set-LattiswiftParallelEnv

$cases = @(
  @{ Field = "m31"; Circuit = "circuit_m31.txt" },
  @{ Field = "babybear"; Circuit = "circuit_babybear.txt" }
)

$cargoArgs = @("test", "-p", "gkr", "--release", "gkr_correctness_lattiswift_case", "--", "--nocapture")
if ($Dev.IsPresent) {
  $cargoArgs = @("test", "-p", "gkr", "gkr_correctness_lattiswift_case", "--", "--nocapture")
}

# -NoCapture: kept for CLI compatibility; tests always run with --nocapture.
if ($NoCapture) { }

Write-Host ("[lattiswift] dataDir = {0}" -f $DataDir)
Write-Host ("[lattiswift] cases  = {0} (m31, babybear)" -f $cases.Count)
Write-Host ("[lattiswift] profile = {0}" -f $(if ($Dev.IsPresent) { "dev" } else { "release" }))

$ok = 0
$skippedEmpty = 0
foreach ($case in $cases) {
  $cPath = Join-Path $DataDir $case.Circuit
  if (!(Test-Path $cPath)) {
    Write-Warning ("[lattiswift] skip (missing circuit): {0}" -f $case.Circuit)
    continue
  }
  $witnessName = $case.Circuit -replace "circuit", "witness"
  $wPath = Join-Path $DataDir $witnessName
  if (!(Test-Path $wPath)) {
    Write-Warning ("[lattiswift] skip (no witness): {0} -> {1}" -f $case.Circuit, $witnessName)
    continue
  }
  $c = Get-Item -LiteralPath $cPath
  $w = Get-Item -LiteralPath $wPath
  if (-not $IncludeEmpty.IsPresent -and ($c.Length -eq 0 -or $w.Length -eq 0)) {
    Write-Warning ("[lattiswift] skip (empty file): {0} ({1} bytes), {2} ({3} bytes)" -f $c.Name, $c.Length, $w.Name, $w.Length)
    $skippedEmpty++
    continue
  }

  $env:GKR_TEST_CIRCUIT_PATH = $c.FullName
  $env:GKR_TEST_WITNESS_PATH = (Resolve-Path $wPath).Path
  $env:GKR_TEST_FIELD_TYPE = $case.Field

  Write-Host ("`n[lattiswift] run({0}): {1} / {2}" -f $case.Field, $c.Name, (Split-Path -Leaf $wPath))
  $exit = Invoke-CargoInRepo -RepoRoot $repoRoot -Arguments $cargoArgs
  if ($exit -ne 0) {
    Write-Warning @"
[lattiswift] cargo failed (exit $exit). mpi-sys / bindgen on Windows usually needs:
  1) LIBCLANG_PATH -> folder with libclang.dll (e.g. LLVM bin), and
  2) MSVC + Windows SDK includes (INCLUDE, VCINSTALLDIR) — this script runs vcvars64.bat for that.
  Or open "x64 Native Tools Command Prompt for VS 2022", set LIBCLANG_PATH, then run the script with -SkipVcVars.
"@
    throw ("[lattiswift] FAILED: {0}" -f $c.Name)
  }
  $ok++
}

Write-Host ("`n[lattiswift] DONE: {0} case(s) passed; {1} empty skipped" -f $ok, $skippedEmpty)
