<#
Lattice PCS + GKR：对 new_data/（可改）下的 MatMul（或任意 *circuit*.txt）跑 gkr_correctness_lattiswift_case，
并从测试输出解析 prover / verifier 耗时与 proof 字节数（与 gkr/src/tests/gkr_correctness.rs 中 root_println 一致）。

用法（仓库根）：
  .\scripts\run_lattiswift_matmul.ps1
  .\scripts\run_lattiswift_matmul.ps1 -DataDir .\new_data -OutputCsv matmul_lattice_metrics.csv

说明：
- 仅支持 GKR_TEST_FIELD_TYPE 为 m31、babybear（与 lattiswift 单测一致）；其它域名从文件名推断后会跳过。
- 需与本仓库其它 gkr 测试相同环境：MPI、bindgen / LIBCLANG、（可选）vcvars64。
- 多线程：默认开启 Lattice MLE 并行，并设置 RAYON_NUM_THREADS（GKR 内 par_verify 等也用 Rayon）。
  - `-LatticeThreads 0`（默认）：逻辑核数 = [Environment]::ProcessorCount。
  - 显式例如 `-LatticeThreads 16` 可封顶；`-SequentialLattice` 会关掉 Lattice 并行并取消上述线程变量。
- 多个电路文件会**依次**各跑一遍 cargo，电路之间仍是串行的；单电路内部的耗时主要受上述线程数影响。
#>
param(
  [string]$DataDir = "",
  [string]$OutputCsv = "",
  # 当文件名里不含 m31/babybear 等关键字时，用此域（例如 expander-gen 输出为 matmul_n3_circuit.txt）
  [string]$DefaultField = "m31",
  [string]$CircuitGlob = "*circuit*.txt",
  [switch]$IncludeEmpty,
  [switch]$Dev,
  [switch]$SkipVcVars,
  [switch]$SequentialLattice,
  #0 = 使用全部逻辑处理器；>0 = 固定线程数（Lattice专用池 + RAYON_NUM_THREADS）
  [int]$LatticeThreads = 0
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

function Import-MsvcDevEnvironment {
  if ($SkipVcVars.IsPresent) {
    Write-Host "[lattiswift-matmul] -SkipVcVars: not running vcvars64.bat"
    return
  }
  if ($env:LATTISWIFT_SKIP_VCVARS) {
    Write-Host "[lattiswift-matmul] LATTISWIFT_SKIP_VCVARS set; skipping vcvars64.bat"
    return
  }
  if ($env:INCLUDE -and $env:VCINSTALLDIR) {
    Write-Host "[lattiswift-matmul] INCLUDE and VCINSTALLDIR already set; skipping vcvars64.bat"
    return
  }
  $vcvars = Find-VcVars64Bat
  if (-not $vcvars) {
    Write-Warning @"
[lattiswift-matmul] Could not find vcvars64.bat. mpi-sys bindgen often needs MSVC + Windows SDK.
"@
    return
  }
  Write-Host ("[lattiswift-matmul] loading MSVC/WinSDK env: {0}" -f $vcvars)
  $prevEap = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  try {
    $lines = cmd.exe /c "call `"$vcvars`" >nul 2>&1 && set" 2>&1
    foreach ($line in $lines) {
      if ($line -is [System.Management.Automation.ErrorRecord]) { continue }
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

function Set-LibClangPathIfNeeded {
  if ($env:LIBCLANG_PATH) {
    $dll = Join-Path $env:LIBCLANG_PATH "libclang.dll"
    if (Test-Path -LiteralPath $dll) {
      Write-Host ("[lattiswift-matmul] LIBCLANG_PATH = {0}" -f $env:LIBCLANG_PATH)
      return
    }
  }
  foreach ($root in @($env:ProgramFiles, ${env:ProgramFiles(x86)})) {
    if ([string]::IsNullOrWhiteSpace($root)) { continue }
    foreach ($ed in @("Community", "Professional", "Enterprise", "BuildTools")) {
      foreach ($dir in @(
          (Join-Path $root "LLVM\bin"),
          (Join-Path $root "Microsoft Visual Studio\2022\$ed\VC\Tools\Llvm\x64\bin")
        )) {
        if ([string]::IsNullOrWhiteSpace($dir)) { continue }
        $dll = Join-Path $dir "libclang.dll"
        if (Test-Path -LiteralPath $dll) {
          $env:LIBCLANG_PATH = $dir
          Write-Host ("[lattiswift-matmul] auto LIBCLANG_PATH = {0}" -f $dir)
          return
        }
      }
    }
  }
  Write-Warning "[lattiswift-matmul] libclang.dll not found; set LIBCLANG_PATH if bindgen fails."
}

function Set-LattiswiftParallelEnv {
  param([int]$ThreadBudget)
  if ($SequentialLattice.IsPresent) {
    $env:LATTICE_MLE_COMMIT_PARALLEL = "0"
    Remove-Item Env:LATTICE_MLE_COMMIT_THREADS -ErrorAction SilentlyContinue
    Remove-Item Env:RAYON_NUM_THREADS -ErrorAction SilentlyContinue
    Write-Host "[lattiswift-matmul] SequentialLattice: lattice + Rayon pool env cleared"
    return
  }
  $n = [Math]::Max(1, $ThreadBudget)
  $env:LATTICE_MLE_COMMIT_PARALLEL = "1"
  $env:LATTICE_MLE_COMMIT_THREADS = "$n"
  $env:RAYON_NUM_THREADS = "$n"
  Write-Host ("[lattiswift-matmul] parallel: LATTICE_MLE_COMMIT_PARALLEL=1, LATTICE_MLE_COMMIT_THREADS={0}, RAYON_NUM_THREADS={0}" -f $n)
}

function Invoke-CargoCapture {
  param(
    [Parameter(Mandatory = $true)][string]$RepoRoot,
    [Parameter(Mandatory = $true)][string[]]$Arguments
  )
  $prevEap = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  $lines = [System.Collections.Generic.List[string]]::new()
  try {
    Push-Location $RepoRoot
    $raw = & cargo @Arguments 2>&1
    $code = $LASTEXITCODE
    foreach ($line in $raw) {
      if ($line -is [System.Management.Automation.ErrorRecord]) {
        $s = $line.ToString()
      }
      else {
        $s = [string]$line
      }
      $lines.Add($s)
      Write-Host $s
    }
    return @{ ExitCode = $code; Lines = $lines }
  }
  finally {
    Pop-Location
    $ErrorActionPreference = $prevEap
  }
}

function Parse-GkrMetrics {
  param([string[]]$Lines)
  $text = $Lines -join "`n"
  $proveUs = $null
  $verifyUs = $null
  $proofBytes = $null
  # 不依赖 μs 字符编码（Windows 控制台可能是 us）
  if ($text -match 'Proving time:\s*(\d+)') {
    $proveUs = [long]$Matches[1]
  }
  if ($text -match 'Proof generated\.\s*Size:\s*(\d+)\s*bytes') {
    $proofBytes = [long]$Matches[1]
  }
  if ($text -match 'Verification time:\s*(\d+)') {
    $verifyUs = [long]$Matches[1]
  }
  return [PSCustomObject]@{
    ProveUs = $proveUs
    VerifyUs   = $verifyUs
    ProofBytes = $proofBytes
  }
}

$repoRoot = Get-RepoRoot
if ([string]::IsNullOrWhiteSpace($DataDir)) {
  $DataDir = Join-Path $repoRoot "new_data"
}
if (-not (Test-Path -LiteralPath $DataDir)) {
  throw "DataDir not found: $DataDir (generate circuits into new_data/ or pass -DataDir)"
}
$DataDir = (Resolve-Path $DataDir).Path

Import-MsvcDevEnvironment
Set-LibClangPathIfNeeded

$effectiveThreads = if ($LatticeThreads -le 0) {
  [Math]::Max(1, [Environment]::ProcessorCount)
}
else {
  $LatticeThreads
}
if ($LatticeThreads -le 0) {
  Write-Host ("[lattiswift-matmul] -LatticeThreads 0 -> using logical processors: {0}" -f $effectiveThreads)
}
Set-LattiswiftParallelEnv -ThreadBudget $effectiveThreads

$circuits = Get-ChildItem -LiteralPath $DataDir -File -Filter $CircuitGlob | Sort-Object Name
if ($circuits.Count -eq 0) {
  throw "No circuit files under $DataDir (pattern: $CircuitGlob)"
}

$cargoArgs = @("test", "-p", "gkr", "--release", "gkr_correctness_lattiswift_case", "--", "--nocapture")
if ($Dev.IsPresent) {
  $cargoArgs = @("test", "-p", "gkr", "gkr_correctness_lattiswift_case", "--", "--nocapture")
}

Write-Host ("[lattiswift-matmul] DataDir = {0}" -f $DataDir)
Write-Host ("[lattiswift-matmul] circuits = {0}" -f $circuits.Count)

$rows = @()
$ok = 0
$skipped = 0

foreach ($cfile in $circuits) {
  $witnessName = $cfile.Name -replace "circuit", "witness"
  $wPath = Join-Path $DataDir $witnessName
  if (-not (Test-Path -LiteralPath $wPath)) {
    Write-Warning ("[lattiswift-matmul] skip (no witness): {0}" -f $cfile.Name)
    $skipped++
    continue
  }
  if (-not $IncludeEmpty.IsPresent -and ($cfile.Length -eq 0 -or (Get-Item $wPath).Length -eq 0)) {
    Write-Warning ("[lattiswift-matmul] skip (empty): {0}" -f $cfile.Name)
    $skipped++
    continue
  }

  $lower = $cfile.Name.ToLowerInvariant()
  $field = ""
  if ($lower -match "babybear") { $field = "babybear" }
  elseif ($lower -match "m31") { $field = "m31" }
  elseif ($lower -match "bn254") { $field = "bn254" }
  elseif ($lower -match "gf2") { $field = "gf2" }
  elseif ($lower -match "goldilocks") { $field = "goldilocks" }

  if ([string]::IsNullOrWhiteSpace($field) -and $DefaultField -in @("m31", "babybear")) {
    $field = $DefaultField
  }

  if ($field -notin @("m31", "babybear")) {
    Write-Warning ("[lattiswift-matmul] skip (lattiswift only m31/babybear; inferred '{0}' from {1})" -f $field, $cfile.Name)
    $skipped++
    continue
  }

  $env:GKR_TEST_CIRCUIT_PATH = $cfile.FullName
  $env:GKR_TEST_WITNESS_PATH = (Resolve-Path $wPath).Path
  $env:GKR_TEST_FIELD_TYPE = $field

  Write-Host ("`n[lattiswift-matmul] === {0} | field={1} ===" -f $cfile.Name, $field)
  $run = Invoke-CargoCapture -RepoRoot $repoRoot -Arguments $cargoArgs
  if ($run.ExitCode -ne 0) {
    throw ("cargo failed (exit {0}) on {1}" -f $run.ExitCode, $cfile.Name)
  }
  $m = Parse-GkrMetrics -Lines ($run.Lines.ToArray())
  $rows += [PSCustomObject]@{
    Circuit      = $cfile.Name
    Field        = $field
    ProveUs      = $m.ProveUs
    VerifyUs     = $m.VerifyUs
    ProofBytes   = $m.ProofBytes
    CircuitPath  = $cfile.FullName
    WitnessPath  = $env:GKR_TEST_WITNESS_PATH
  }
  $ok++
}

Write-Host ("`n[lattiswift-matmul] DONE: ran {0}; skipped {1}" -f $ok, $skipped)

$rows | Format-Table Circuit, Field, ProveUs, VerifyUs, ProofBytes -AutoSize

if ($OutputCsv) {
  $dst = $OutputCsv
  if (-not [System.IO.Path]::IsPathRooted($dst)) {
    $dst = Join-Path $repoRoot $dst
  }
  $rows | Export-Csv -Path $dst -NoTypeInformation -Encoding UTF8
  Write-Host ("Wrote {0}" -f $dst)
}
