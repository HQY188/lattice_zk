# 性能测试：重复运行 GKR correctness（Lattice PCS）并导出耗时/Proof 大小
#
# 设计目标：对比不同实现的“端到端证明/验证时间”，并避免把第一次编译计入测量。
#
# 在仓库根目录运行：
#   .\scripts\perf_gkr_lattice.ps1 -Runs 5 -OutputCsv perf_gkr_lattice.csv
# 可选对照 Raw：
#   .\scripts\perf_gkr_lattice.ps1 -Runs 5 -OutputCsv perf_gkr_compare.csv -CompareRaw
#
param(
    [int] $Runs = 5,
    [string] $OutputCsv = "perf_gkr_lattice.csv",
    [switch] $CompareRaw = $false,
    [int] $TestThreads = 1,
    [switch] $SkipBuild = $false
)

$ErrorActionPreference = "Stop"

$Root = if ($PSScriptRoot) { Join-Path $PSScriptRoot ".." } else { ".." }
$Root = (Resolve-Path $Root).Path
Push-Location $Root

function Remove-LatticeDebugEnv {
    # 避免开启调试导致输出过多、影响解析和性能测量
    Remove-Item Env:LATTICE_MLE_DEBUG -ErrorAction SilentlyContinue
}

function Extract-Int64ByRegex([string]$text, [string]$pattern) {
    $m = [regex]::Match($text, $pattern, [System.Text.RegularExpressions.RegexOptions]::Singleline)
    if ($m.Success) {
        return [long]$m.Groups[1].Value
    }
    return $null
}

function Run-AndParse([string]$testName, [string]$pcsLabel) {
    # 复测时保证 debug 关闭；若你确实要 debug，把它改成你期望的值即可
    Remove-LatticeDebugEnv

    Write-Host "Running: $testName (PCS=$pcsLabel)"
    # Rust 编译/测试输出会大量写 stderr（包括 warning），PowerShell 在 Stop 模式下会把 stderr 转成 ErrorRecord 并终止脚本。
    # 因此这里临时切到 Continue，让 warning 不会中断；但仍用 $LASTEXITCODE 判断真正失败。
    $prevErrorAction = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $out = cargo test -p gkr --release $testName -- --nocapture "--test-threads=$TestThreads" 2>&1 | Out-String
    $exitCode = $LASTEXITCODE
    $ErrorActionPreference = $prevErrorAction
    if ($exitCode -ne 0) {
        throw "cargo test failed for $testName (PCS=$pcsLabel), exitCode=$exitCode"
    }

    $proving_us = Extract-Int64ByRegex $out "Proving time:\s+(\d+)\s*(?:μs|us)"
    $proof_bytes = Extract-Int64ByRegex $out "Proof generated\.\s*Size:\s+(\d+)\s*bytes"
    $verify_us = Extract-Int64ByRegex $out "Verification time:\s+(\d+)\s*(?:μs|us)"
    $par_verify_us = Extract-Int64ByRegex $out "Multi-core Verification time:\s+(\d+)\s*(?:μs|us)"

    if (-not $proving_us -or -not $verify_us) {
        Write-Warning "Parse maybe failed for $testName (PCS=$pcsLabel). Fields: proving=$proving_us verify=$verify_us"
    }

    [PSCustomObject]@{
        PCS = $pcsLabel
        Test = $testName
        ProvingUs = $proving_us
        ProofBytes = $proof_bytes
        VerifyUs = $verify_us
        ParVerifyUs = $par_verify_us
    }
}

if (-not $SkipBuild) {
    Write-Host "Pre-building (cargo test --no-run) to exclude compile time from measurements..."
    cargo test -p gkr --release gkr_correctness_lattice --no-run 2>&1 | Out-String | Out-Null
    if ($CompareRaw) {
        cargo test -p gkr --release gkr_correctness_raw --no-run 2>&1 | Out-String | Out-Null
    }
}

$rows = @()

for ($i = 0; $i -lt $Runs; $i++) {
    Write-Host "================ Run $($i+1)/$Runs ================"
    $lat = Run-AndParse "gkr_correctness_lattice" "Lattice"
    $lat | Add-Member -NotePropertyName RunIndex -NotePropertyValue $i -Force
    $rows += $lat

    if ($CompareRaw) {
        $raw = Run-AndParse "gkr_correctness_raw" "Raw"
        $raw | Add-Member -NotePropertyName RunIndex -NotePropertyValue $i -Force
        $rows += $raw
    }
}

# CSV
$csvPath = $OutputCsv
if (-not [System.IO.Path]::IsPathRooted($csvPath)) {
    $csvPath = Join-Path $Root $csvPath
}
$rows | Export-Csv -Path $csvPath -NoTypeInformation -Encoding UTF8
Write-Host "Wrote CSV: $csvPath"

Write-Host "================ Summary ================"
foreach ($pcs in @("Lattice") + ($(if ($CompareRaw) { "Raw" } else { @() }))) {
    $sub = $rows | Where-Object { $_.PCS -eq $pcs } | Sort-Object ProvingUs
    $proving = $sub.ProvingUs | Where-Object { $_ -ne $null }
    if ($proving.Count -gt 0) {
        $avgProv = ($proving | Measure-Object -Average).Average
        $medProv = $proving[([int]([math]::Floor($proving.Count/2)))]
        Write-Host ("{0}: ProvingUs avg={1} median={2}" -f $pcs, ([long]$avgProv), $medProv)
    }
}

Pop-Location

