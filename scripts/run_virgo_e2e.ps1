# End-to-end Virgo zk_proof: two fixed test points that succeed without Expander Keccak conversion.
# 1) Prefer Virgo/tests/SHA256 SHA256_64_merkle_{1,2}_*.txt (official Virgo samples).
# 2) Else compile Virgo/tests/matmul/gen.cpp and run mat size 2 and 4 (two small circuits).
#
# 【逐步】找 zk_proof -> 若有 SHA256 官方样例则跑 merkle_1/2 各一次并写 log到 results/；
# 否则用 g++ 编译 gen.cpp，生成 mat_2 / mat_4 电路与 meta，再各跑 zk_proof。
# 与仓库 GKR/Lattice 测试用的 data/ Keccak 文件无直接关系。
#
# Usage (repo root):
#   .\scripts\run_virgo_e2e.ps1
#   .\scripts\run_virgo_e2e.ps1 -VirgoRoot D:\path\to\Virgo

param(
  [string]$VirgoRoot = ""
)

$ErrorActionPreference = "Stop"

function Get-RepoRoot {
  if ($PSScriptRoot) {
    return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
  }
  return (Resolve-Path (Join-Path (Get-Location).Path "..")).Path
}

function Find-GPlusPlus {
  $w = Get-Command g++ -ErrorAction SilentlyContinue
  if ($w) { return $w.Source }
  foreach ($base in @(
      'C:\msys64\mingw64\bin\g++.exe',
      'C:\msys64\ucrt64\bin\g++.exe',
      'D:\mingw64\bin\g++.exe'
    )) {
    if (Test-Path $base) { return $base }
  }
  return $null
}

$repoRoot = Get-RepoRoot
if ([string]::IsNullOrWhiteSpace($VirgoRoot)) {
  $VirgoRoot = Join-Path $repoRoot "Virgo"
}
$VirgoRoot = (Resolve-Path $VirgoRoot).Path

$zk = $null
foreach ($p in @(
    (Join-Path $VirgoRoot "build-mingw\zk_proof.exe"),
    (Join-Path $VirgoRoot "build-mingw\zk_proof"),
    (Join-Path $VirgoRoot "build-linux\zk_proof.exe"),
    (Join-Path $VirgoRoot "build-linux\zk_proof"),
    (Join-Path $VirgoRoot "tests\SHA256\zk_proof.exe"),
    (Join-Path $VirgoRoot "tests\SHA256\zk_proof"),
    (Join-Path $VirgoRoot "zk_proof.exe"),
    (Join-Path $VirgoRoot "zk_proof")
  )) {
  if (Test-Path $p) {
    $zk = (Resolve-Path $p).Path
    break
  }
}
if (-not $zk) {
  Write-Error "zk_proof not found. Build Virgo first: .\scripts\build_virgo.ps1"
  exit 1
}

$resultsDir = Join-Path $repoRoot "results"
if (!(Test-Path $resultsDir)) {
  New-Item -ItemType Directory -Path $resultsDir | Out-Null
}

function Invoke-One {
  param([string]$Name, [string]$Circuit, [string]$Meta, [string]$Log)
  Write-Host ""
  Write-Host "[virgo-e2e] $Name"
  Write-Host "  circuit=$Circuit"
  Write-Host "  meta=$Meta"
  Write-Host "  log=$Log"
  & $zk $Circuit $Meta $Log
  if ($LASTEXITCODE -ne 0) {
    Write-Error "[virgo-e2e] FAILED: $Name (exit $LASTEXITCODE)"
  }
  $tail = Get-Content $Log -Tail 3 -ErrorAction SilentlyContinue
  if ($tail) { Write-Host ($tail -join "`n") }
}

$shaDir = Join-Path $VirgoRoot "tests\SHA256"
$m1c = Join-Path $shaDir "SHA256_64_merkle_1_circuit.txt"
$m1m = Join-Path $shaDir "SHA256_64_merkle_1_meta.txt"
$m2c = Join-Path $shaDir "SHA256_64_merkle_2_circuit.txt"
$m2m = Join-Path $shaDir "SHA256_64_merkle_2_meta.txt"

if ((Test-Path $m1c) -and (Test-Path $m1m) -and (Test-Path $m2c) -and (Test-Path $m2m)) {
  Write-Host "[virgo-e2e] Mode: SHA256 merkle_1 + merkle_2 (official Virgo test inputs)."
  Invoke-One -Name "merkle_1" -Circuit $m1c -Meta $m1m -Log (Join-Path $resultsDir "virgo_e2e_sha256_merkle1.log")
  Invoke-One -Name "merkle_2" -Circuit $m2c -Meta $m2m -Log (Join-Path $resultsDir "virgo_e2e_sha256_merkle2.log")
  Write-Host ""
  Write-Host "[virgo-e2e] OK (2/2 SHA256)."
  exit 0
}

Write-Host "[virgo-e2e] SHA256 files not under tests\SHA256; falling back to matmul gen (sizes 2 and 4)."
Write-Host "[virgo-e2e] To use SHA256 instead, generate them under Virgo (see Virgo/tests/SHA256/build.py on Unix)."

$gpp = Find-GPlusPlus
if (-not $gpp) {
  Write-Error "g++ not found. Install MinGW or MSYS2 mingw-w64, or generate SHA256 test files."
  exit 1
}

$mmDir = Join-Path $VirgoRoot "tests\matmul"
$genCpp = Join-Path $mmDir "gen.cpp"
if (-not (Test-Path $genCpp)) {
  Write-Error "Missing $genCpp"
  exit 1
}

$genExe = Join-Path $mmDir "gen.exe"
Push-Location $mmDir
try {
  & $gpp "gen.cpp" -o $genExe -O3
  if ($LASTEXITCODE -ne 0) { Write-Error "g++ gen.cpp failed" }
  & $genExe "2" "mat_2_circuit.txt" "mat_2_meta.txt"
  if ($LASTEXITCODE -ne 0) { Write-Error "gen.exe size 2 failed" }
  & $genExe "4" "mat_4_circuit.txt" "mat_4_meta.txt"
  if ($LASTEXITCODE -ne 0) { Write-Error "gen.exe size 4 failed" }
}
finally {
  Pop-Location
}

$mat2c = Join-Path $mmDir "mat_2_circuit.txt"
$mat2m = Join-Path $mmDir "mat_2_meta.txt"
$mat4c = Join-Path $mmDir "mat_4_circuit.txt"
$mat4m = Join-Path $mmDir "mat_4_meta.txt"

Invoke-One -Name "matmul n=2" -Circuit $mat2c -Meta $mat2m -Log (Join-Path $resultsDir "virgo_e2e_matmul_2.log")
Invoke-One -Name "matmul n=4" -Circuit $mat4c -Meta $mat4m -Log (Join-Path $resultsDir "virgo_e2e_matmul_4.log")

Write-Host ""
Write-Host "[virgo-e2e] OK (2/2 matmul)."
exit 0
