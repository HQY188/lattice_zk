<#
兼容旧入口：单次对照（Ours(Orion) vs Libra(Raw)）

旧脚本对照的是 Raw vs Lattice PCS（基于 gkr correctness test）。现在统一口径到 `data/` + `expander-exec` 的 prove+verify：
- Ours(Orion)  : PCS=Orion
- Libra(Raw)   : PCS=Raw

用法（仓库根目录）:
  .\scripts\bench_gkr_pcs.ps1
  .\scripts\bench_gkr_pcs.ps1 -OutputCsv bench_result.csv
#>
param(
    [string] $OutputCsv = "",
    [int] $Threads = 1,
    [string[]] $Datasets = @("m31"),
    [switch] $SkipBuild = $false
)

$ErrorActionPreference = "Stop"
$Root = if ($PSScriptRoot) { Join-Path $PSScriptRoot ".." } else { ".." }
$Root = (Resolve-Path $Root).Path

$perfAll = Join-Path $Root "scripts\perf_all.ps1"
if (-not (Test-Path $perfAll)) { throw "Missing script: $perfAll" }

$outDir = Join-Path $Root "_bench_compat"
& $perfAll -Runs 1 -Threads $Threads -Datasets $Datasets -OutDir $outDir -SkipVirgo -SkipBuild:$SkipBuild
if ($LASTEXITCODE -ne 0) { throw "perf_all.ps1 failed" }

$src = Join-Path $outDir "expander_exec.csv"
if (-not (Test-Path $src)) { throw "Missing expected output: $src" }

if ($OutputCsv) {
    $dst = $OutputCsv
    if (-not [System.IO.Path]::IsPathRooted($dst)) { $dst = Join-Path $Root $dst }
    Copy-Item -Force $src $dst
    Write-Host "Wrote CSV: $dst"
} else {
    Import-Csv $src | Format-Table Impl,PCS,Dataset,ProveMs,VerifyMs,ProofBytes -AutoSize
}
