<#
Raw PCS（Libra 口径）+ GKR：对 new_data/（可改）下的 MatMul 等 *circuit*.txt 跑 gkr_correctness_libra_case，
并从测试输出解析 Proving time / Verification time / Proof size（与 gkr/src/tests/gkr_correctness.rs 一致）。

与 run_lattiswift_matmul.ps1 的区别：PCS 为 Raw，且支持 m31、bn254、gf2、goldilocks、babybear（见单测 match分支）。

用法（仓库根）：
  .\scripts\run_libra_matmul.ps1
  .\scripts\run_libra_matmul.ps1 -DataDir .\new_data -OutputCsv matmul_libra_metrics.csv
#>
param(
  [string]$DataDir = "",
  [string]$OutputCsv = "",
  [string]$DefaultField = "m31",
  [string]$CircuitGlob = "*circuit*.txt",
  [switch]$IncludeEmpty,
  [switch]$Dev,
  [switch]$SkipVcVars
)

$ErrorActionPreference = "Stop"

$script:LibraValidFields = @("m31", "babybear", "bn254", "gf2", "goldilocks")

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
      foreach ($ver in @("2022", "18")) {
        $bat = Join-Path $root "Microsoft Visual Studio\$ver\$ed\VC\Auxiliary\Build\vcvars64.bat"
        if (Test-Path -LiteralPath $bat) {
          return $bat
        }
      }
    }
  }
  return $null
}

function Import-MsvcDevEnvironment {
  if ($SkipVcVars.IsPresent) {
    Write-Host "[libra-matmul] -SkipVcVars: not running vcvars64.bat"
    return
  }
  if ($env:LATTISWIFT_SKIP_VCVARS) {
    Write-Host "[libra-matmul] LATTISWIFT_SKIP_VCVARS set; skipping vcvars64.bat"
    return
  }
  if ($env:INCLUDE -and $env:VCINSTALLDIR) {
    Write-Host "[libra-matmul] INCLUDE and VCINSTALLDIR already set; skipping vcvars64.bat"
    return
  }
  $vcvars = Find-VcVars64Bat
  if (-not $vcvars) {
    Write-Warning "[libra-matmul] Could not find vcvars64.bat (mpi-sys bindgen may need MSVC)."
    return
  }
  Write-Host ("[libra-matmul] loading MSVC/WinSDK env: {0}" -f $vcvars)
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
      Write-Host ("[libra-matmul] LIBCLANG_PATH = {0}" -f $env:LIBCLANG_PATH)
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
          Write-Host ("[libra-matmul] auto LIBCLANG_PATH = {0}" -f $dir)
          return
        }
      }
    }
  }
  Write-Warning "[libra-matmul] libclang.dll not found; set LIBCLANG_PATH if bindgen fails."
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
    ProveUs    = $proveUs
    VerifyUs = $verifyUs
    ProofBytes = $proofBytes
  }
}

function Infer-FieldFromName([string]$lowerName) {
  if ($lowerName -match "bn254") { return "bn254" }
  if ($lowerName -match "babybear") { return "babybear" }
  if ($lowerName -match "goldilocks") { return "goldilocks" }
  if ($lowerName -match "gf2") { return "gf2" }
  if ($lowerName -match "m31") { return "m31" }
  return ""
}

$repoRoot = Get-RepoRoot
if ([string]::IsNullOrWhiteSpace($DataDir)) {
  $DataDir = Join-Path $repoRoot "new_data"
}
if (-not (Test-Path -LiteralPath $DataDir)) {
  throw "DataDir not found: $DataDir"
}
$DataDir = (Resolve-Path $DataDir).Path

if ($DefaultField -notin $script:LibraValidFields) {
  throw ("-DefaultField must be one of: {0}" -f ($script:LibraValidFields -join ", "))
}

Import-MsvcDevEnvironment
Set-LibClangPathIfNeeded

$circuits = Get-ChildItem -LiteralPath $DataDir -File -Filter $CircuitGlob | Sort-Object Name
if ($circuits.Count -eq 0) {
  throw "No circuit files under $DataDir (pattern: $CircuitGlob)"
}

$cargoArgs = @("test", "-p", "gkr", "--release", "gkr_correctness_libra_case", "--", "--nocapture")
if ($Dev.IsPresent) {
  $cargoArgs = @("test", "-p", "gkr", "gkr_correctness_libra_case", "--", "--nocapture")
}

Write-Host ("[libra-matmul] DataDir = {0}" -f $DataDir)
Write-Host ("[libra-matmul] circuits = {0}" -f $circuits.Count)

$rows = @()
$ok = 0
$skipped = 0

foreach ($cfile in $circuits) {
  $witnessName = $cfile.Name -replace "circuit", "witness"
  $wPath = Join-Path $DataDir $witnessName
  if (-not (Test-Path -LiteralPath $wPath)) {
    Write-Warning ("[libra-matmul] skip (no witness): {0}" -f $cfile.Name)
    $skipped++
    continue
  }
  if (-not $IncludeEmpty.IsPresent -and ($cfile.Length -eq 0 -or (Get-Item $wPath).Length -eq 0)) {
    Write-Warning ("[libra-matmul] skip (empty): {0}" -f $cfile.Name)
    $skipped++
    continue
  }

  $lower = $cfile.Name.ToLowerInvariant()
  $field = Infer-FieldFromName $lower
  if ([string]::IsNullOrWhiteSpace($field)) {
    $field = $DefaultField
  }

  if ($field -notin $script:LibraValidFields) {
    Write-Warning ("[libra-matmul] skip (unknown field '{0}'): {1}" -f $field, $cfile.Name)
    $skipped++
    continue
  }

  $env:GKR_TEST_CIRCUIT_PATH = $cfile.FullName
  $env:GKR_TEST_WITNESS_PATH = (Resolve-Path $wPath).Path
  $env:GKR_TEST_FIELD_TYPE = $field

  Write-Host ("`n[libra-matmul] === {0} | field={1} (Raw PCS) ===" -f $cfile.Name, $field)
  $run = Invoke-CargoCapture -RepoRoot $repoRoot -Arguments $cargoArgs
  if ($run.ExitCode -ne 0) {
    throw ("cargo failed (exit {0}) on {1}" -f $run.ExitCode, $cfile.Name)
  }
  $m = Parse-GkrMetrics -Lines ($run.Lines.ToArray())
  $rows += [PSCustomObject]@{
    Circuit     = $cfile.Name
    Field       = $field
    PCS         = "Raw"
    ProveUs     = $m.ProveUs
    VerifyUs    = $m.VerifyUs
    ProofBytes  = $m.ProofBytes
    CircuitPath = $cfile.FullName
    WitnessPath = $env:GKR_TEST_WITNESS_PATH
  }
  $ok++
}

Write-Host ("`n[libra-matmul] DONE: ran {0}; skipped {1}" -f $ok, $skipped)

$rows | Format-Table Circuit, Field, PCS, ProveUs, VerifyUs, ProofBytes -AutoSize

if ($OutputCsv) {
  $dst = $OutputCsv
  if (-not [System.IO.Path]::IsPathRooted($dst)) {
    $dst = Join-Path $repoRoot $dst
  }
  $rows | Export-Csv -Path $dst -NoTypeInformation -Encoding UTF8
  Write-Host ("Wrote {0}" -f $dst)
}
