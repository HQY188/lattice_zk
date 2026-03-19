# GKR + PCS 性能对照脚本
# 对照实验：同一电路 (M31x16, 32 keccak/proof) 下 Raw PCS vs Lattice PCS
# 输出：证明时间、验证时间、多核验证时间、Proof 大小；并运行 criterion 证明耗时 bench
#
# 用法（在仓库根目录）:
#   .\scripts\bench_gkr_pcs.ps1
#   .\scripts\bench_gkr_pcs.ps1 -OutputCsv bench_result.csv
#   .\scripts\bench_gkr_pcs.ps1 -SkipBench  # 只跑 test 计时，不跑 criterion

param(
    [string] $OutputCsv = "",   # 若指定则追加/写入 CSV
    [switch] $SkipBench = $false
)

$ErrorActionPreference = "Stop"
$Root = if ($PSScriptRoot) { Join-Path $PSScriptRoot ".." } else { ".." }
$Root = (Resolve-Path $Root).Path
Push-Location $Root

function Parse-TestOutput {
    param([string]$text, [string]$pcs)
    $proving_us = $null
    $proof_bytes = $null
    $verify_us = $null
    $par_verify_us = $null
    foreach ($line in ($text -split "`n")) {
        if ($line -match "Proving time:\s+(\d+)\s*μs")      { $proving_us = [long]$Matches[1] }
        if ($line -match "Proof generated\. Size:\s+(\d+)\s*bytes") { $proof_bytes = [long]$Matches[1] }
        if ($line -match "Verification time:\s+(\d+)\s*μs") { $verify_us = [long]$Matches[1] }
        if ($line -match "Multi-core Verification time:\s+(\d+)\s*μs") { $par_verify_us = [long]$Matches[1] }
    }
    [PSCustomObject]@{
        PCS = $pcs
        ProvingUs = $proving_us
        ProofBytes = $proof_bytes
        VerifyUs = $verify_us
        ParVerifyUs = $par_verify_us
    }
}

Write-Host "============== GKR + PCS 性能对照 =============="
Write-Host "Circuit: M31x16, 32 keccak instances per proof"
Write-Host ""

# Cargo 会把进度信息写到 stderr，PowerShell 在 Stop 模式下会当成错误；此处暂时改为 Continue 再执行 cargo
$prevErrorAction = $ErrorActionPreference
$ErrorActionPreference = "Continue"

# 1) Raw PCS 单次运行（test 输出含时间与 proof 大小）
Write-Host "[1/3] Running GKR correctness test (Raw PCS)..."
$rawOut = cargo test -p gkr --release gkr_correctness_raw -- --nocapture 2>&1 | Out-String
$raw = Parse-TestOutput -text $rawOut -pcs "Raw"
if (-not $raw.ProvingUs) { Write-Warning "Raw test output parse failed. Check test run." }

# 2) Lattice PCS 单次运行
Write-Host "[2/3] Running GKR correctness test (Lattice PCS)..."
$latOut = cargo test -p gkr --release gkr_correctness_lattice -- --nocapture 2>&1 | Out-String
$lat = Parse-TestOutput -text $latOut -pcs "Lattice"
if (-not $lat.ProvingUs) { Write-Warning "Lattice test output parse failed. Check test run." }

# 3) Criterion 证明耗时 bench（多轮取统计）
$benchOut = $null
if (-not $SkipBench) {
    Write-Host "[3/3] Running criterion bench (GKR proving M31x16 by PCS)..."
    $benchOut = cargo bench -p gkr "GKR proving M31x16" 2>&1 | Out-String
} else {
    Write-Host "[3/3] Skip criterion bench ( -SkipBench )."
}

$ErrorActionPreference = $prevErrorAction

# 汇总表
$rows = @($raw, $lat)
Write-Host ""
Write-Host "------------- 单次运行结果 (test) -------------"
$header = "{0,-10} {1,14} {2,14} {3,14} {4,14}" -f "PCS", "Proving(μs)", "Proof(bytes)", "Verify(μs)", "ParVerify(μs)"
Write-Host $header
foreach ($r in $rows) {
    $line = "{0,-10} {1,14} {2,14} {3,14} {4,14}" -f $r.PCS, $r.ProvingUs, $r.ProofBytes, $r.VerifyUs, $r.ParVerifyUs
    Write-Host $line
}

if ($benchOut) {
    Write-Host ""
    Write-Host "------------- Criterion 证明耗时 (多轮) -------------"
    Write-Host $benchOut
}

# CSV 输出
if ($OutputCsv) {
    $csvPath = $OutputCsv
    if (-not [System.IO.Path]::IsPathRooted($csvPath)) { $csvPath = Join-Path $Root $csvPath }
    $rows | Export-Csv -Path $csvPath -NoTypeInformation
    Write-Host ""
    Write-Host "Wrote CSV: $csvPath"
}

Write-Host ""
Write-Host "============== end =============="
Pop-Location
