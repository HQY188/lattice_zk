<#
用 Virgo 的 zk_proof 可执行文件跑证明（与 Expander 侧 data 或 Virgo 自带用例对接）。

【三种入口优先级】
1) 若设置 VIRGO_M31_* / VIRGO_BABYBEAR_*（circuit/meta/log 路径）：直接 zk_proof；并设 EXPANDER_FIELD + witness 路径供 expander_to_virgo 转换。
2) 否则若存在 data/circuit_m31.txt 或 circuit_babybear.txt：要求已编译 expander_to_virgo，通过环境变量 EXPANDER_TO_VIRGO 在 zk_proof 内把 Expander 二进制电路转为 Virgo 文本（circuit 参数仍指向 .txt 二进制路径，meta 用占位）。
3) -UseSha256Fallback：改用 Virgo/tests/SHA256 下预生成的 SHA256 Merkle 文本电路（与 data/ 的 Keccak 基准不是同一文件格式/语义）。

【本脚本逐步】查找 zk_proof 与 expander_to_virgo -> 设 EXPANDER_TO_VIRGO（若有）-> 按上述分支调用 zk_proof。

【与 perf_all / lattice 测试的区别】此处电路可能是 Virgo 文本格式或经转换；run_lattiswift/run_libra 用的是 gkr 直接加载的 data/*.txt（RecursiveCircuit 序列化）。
#>
param(
  [string]$VirgoRoot = "",
  [string]$ZkProofExe = "",
  [switch]$UseSha256Fallback,
  # Clear VIRGO_* env vars so data/ auto-run is used (avoids huge Keccak paths in env).
  [switch]$SkipEnv
)

$ErrorActionPreference = "Stop"

if ($SkipEnv) {
  foreach ($k in @('VIRGO_M31_CIRCUIT','VIRGO_M31_META','VIRGO_M31_LOG','VIRGO_BABYBEAR_CIRCUIT','VIRGO_BABYBEAR_META','VIRGO_BABYBEAR_LOG')) {
    Remove-Item "Env:$k" -ErrorAction SilentlyContinue
  }
  Write-Host "[virgo] SkipEnv: cleared VIRGO_* circuit env vars."
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

if ([string]::IsNullOrWhiteSpace($ZkProofExe)) {
  foreach ($p in @(
      (Join-Path $VirgoRoot "build-mingw\zk_proof.exe"),
      (Join-Path $VirgoRoot "build-mingw\zk_proof"),
      (Join-Path $VirgoRoot "build-linux\zk_proof.exe"),
      (Join-Path $VirgoRoot "build-linux\zk_proof"),
      (Join-Path $VirgoRoot "zk_proof.exe"),
      (Join-Path $VirgoRoot "zk_proof"),
      (Join-Path $VirgoRoot "build\zk_proof.exe"),
      (Join-Path $VirgoRoot "build\zk_proof"),
      (Join-Path $VirgoRoot "tests\SHA256\zk_proof.exe"),
      (Join-Path $VirgoRoot "tests\SHA256\zk_proof")
    )) {
    if (Test-Path $p) {
      $ZkProofExe = (Resolve-Path $p).Path
      break
    }
  }
}

if (-not $ZkProofExe) {
  Write-Error "zk_proof not found. Build Virgo (cmake) or set -ZkProofExe to the executable."
  exit 1
}

Write-Host ("[virgo] zk_proof: {0}" -f $ZkProofExe)

$converter = $null
foreach ($p in @(
    (Join-Path $repoRoot "target\release\expander_to_virgo.exe"),
    (Join-Path $repoRoot "target\debug\expander_to_virgo.exe"),
    (Join-Path $repoRoot "target\release\expander_to_virgo"),
    (Join-Path $repoRoot "target\debug\expander_to_virgo")
  )) {
  if (Test-Path $p) {
    $converter = (Resolve-Path $p).Path
    break
  }
}
if ($converter) {
  $env:EXPANDER_TO_VIRGO = $converter
  Write-Host ("[virgo] EXPANDER_TO_VIRGO: {0}" -f $converter)
}

$resultsDir = Join-Path $repoRoot "results"
if (!(Test-Path $resultsDir)) {
  New-Item -ItemType Directory -Path $resultsDir | Out-Null
}

# Meta path is ignored for Expander binary input (placeholder only).
$dummyMeta = "dummy_meta.txt"

function Invoke-ZkProof {
  param([string]$Label, [string]$Circuit, [string]$Meta, [string]$Log)
  Write-Host ""
  Write-Host ("[virgo] {0}" -f $Label)
  & $ZkProofExe $Circuit $Meta $Log
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}

function Invoke-ExpanderFieldTest {
  param(
    [string]$Field,
    [string]$CircuitPath,
    [string]$WitnessPath,
    [string]$LogPath
  )
  $env:EXPANDER_FIELD = $Field
  if (Test-Path $WitnessPath) {
    $env:EXPANDER_WITNESS = $WitnessPath
    Write-Host ("[virgo] EXPANDER_FIELD={0}, EXPANDER_WITNESS={1}" -f $Field, $WitnessPath)
  }
  else {
    Remove-Item Env:EXPANDER_WITNESS -ErrorAction SilentlyContinue
    Write-Host ("[virgo] EXPANDER_FIELD={0} (no witness file)" -f $Field)
  }
  $label = ("data/{0} (Expander binary -> Virgo)" -f (Split-Path $CircuitPath -Leaf))
  Invoke-ZkProof -Label $label -Circuit $CircuitPath -Meta $dummyMeta -Log $LogPath
}

# Optional env: VIRGO_M31_* / VIRGO_BABYBEAR_* (circuit, meta, log). EXPANDER_FIELD is set below for expander_to_virgo.
$used = $false
if ($env:VIRGO_M31_CIRCUIT -and $env:VIRGO_M31_META -and $env:VIRGO_M31_LOG) {
  $env:EXPANDER_FIELD = 'm31'
  $witM31 = Join-Path $repoRoot 'data\witness_m31.txt'
  if (Test-Path $witM31) { $env:EXPANDER_WITNESS = (Resolve-Path $witM31).Path }
  Invoke-ZkProof -Label "m31 (env)" -Circuit $env:VIRGO_M31_CIRCUIT -Meta $env:VIRGO_M31_META -Log $env:VIRGO_M31_LOG
  $used = $true
}
if ($env:VIRGO_BABYBEAR_CIRCUIT -and $env:VIRGO_BABYBEAR_META -and $env:VIRGO_BABYBEAR_LOG) {
  $env:EXPANDER_FIELD = 'babybear'
  $witBb = Join-Path $repoRoot 'data\witness_babybear.txt'
  if (Test-Path $witBb) { $env:EXPANDER_WITNESS = (Resolve-Path $witBb).Path }
  Invoke-ZkProof -Label "babybear (env)" -Circuit $env:VIRGO_BABYBEAR_CIRCUIT -Meta $env:VIRGO_BABYBEAR_META -Log $env:VIRGO_BABYBEAR_LOG
  $used = $true
}

if ($used) {
  Write-Host "[virgo] OK (env)."
  exit 0
}

if ($UseSha256Fallback) {
  $sha = Join-Path $VirgoRoot "tests\SHA256"
  $m1c = Join-Path $sha "SHA256_64_merkle_1_circuit.txt"
  $m1m = Join-Path $sha "SHA256_64_merkle_1_meta.txt"
  $m2c = Join-Path $sha "SHA256_64_merkle_2_circuit.txt"
  $m2m = Join-Path $sha "SHA256_64_merkle_2_meta.txt"
  if (!(Test-Path $m1c) -or !(Test-Path $m1m) -or !(Test-Path $m2c) -or !(Test-Path $m2m)) {
    Write-Error ("SHA256 test files missing under {0}. Run Virgo/tests/SHA256 build first." -f $sha)
    exit 1
  }
  Write-Warning "Fallback: Virgo SHA256 merkle_1/2 (not Expander data files)."
  Invoke-ZkProof -Label "merkle_1" -Circuit $m1c -Meta $m1m -Log (Join-Path $resultsDir "virgo_fallback_merkle1.log")
  Invoke-ZkProof -Label "merkle_2" -Circuit $m2c -Meta $m2m -Log (Join-Path $resultsDir "virgo_fallback_merkle2.log")
  Write-Host "[virgo] OK (fallback)."
  exit 0
}

$m31Circuit = Join-Path $repoRoot "data\circuit_m31.txt"
$bbCircuit = Join-Path $repoRoot "data\circuit_babybear.txt"
$m31Witness = Join-Path $repoRoot "data\witness_m31.txt"
$bbWitness = Join-Path $repoRoot "data\witness_babybear.txt"
$m31Log = Join-Path $resultsDir "virgo_data_m31.log"
$bbLog = Join-Path $resultsDir "virgo_data_babybear.log"

$wantM31 = Test-Path $m31Circuit
$wantBb = Test-Path $bbCircuit

if ($wantM31 -or $wantBb) {
  if (-not $converter) {
    Write-Error ("Found data/circuit_*.txt but expander_to_virgo not built. Run: cargo build -p expander_to_virgo --release")
    exit 1
  }
  if ($wantM31) {
    Invoke-ExpanderFieldTest -Field "m31" -CircuitPath $m31Circuit -WitnessPath $m31Witness -LogPath $m31Log
  }
  if ($wantBb) {
    Invoke-ExpanderFieldTest -Field "babybear" -CircuitPath $bbCircuit -WitnessPath $bbWitness -LogPath $bbLog
  }
  Write-Host ""
  Write-Host "[virgo] OK (data/: m31 and/or babybear via expander_to_virgo)."
  exit 0
}

Write-Host ""
Write-Host "Set VIRGO_M31_CIRCUIT, VIRGO_M31_META, VIRGO_M31_LOG (and babybear same), or run with -UseSha256Fallback after building Virgo/tests/SHA256."
Write-Host "Or: cargo build -p expander_to_virgo --release and add data/circuit_m31.txt and/or data/circuit_babybear.txt."
exit 1
