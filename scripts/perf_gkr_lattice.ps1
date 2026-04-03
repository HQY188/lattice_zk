<#
兼容旧入口：性能测试（Ours(Orion)/Libra(Raw)，基于 `expander-exec` + `data/`）

说明：
- 旧脚本基于 `cargo test -p gkr ...` 解析输出；为了让 Ours/Libra/Virgo 用同一套“数据集 prove+verify”口径，
  这里改为直接调用 `scripts/perf_all.ps1`（并默认跳过 Virgo）。

用法（仓库根目录）：
  .\scripts\perf_gkr_lattice.ps1 -Runs 5 -OutputCsv perf_gkr_lattice.csv

输出字段可在 `scripts/perf_all.ps1` 中查看。
#>
param(
    [int] $Runs = 5,
    [string] $OutputCsv = "perf_gkr_lattice.csv",
    [int] $Threads = 1,
    [string[]] $Datasets = @("m31", "bn254", "gf2"),
    [switch] $SkipBuild = $false
)

$ErrorActionPreference = "Stop"
$Root = if ($PSScriptRoot) { Join-Path $PSScriptRoot ".." } else { ".." }
$Root = (Resolve-Path $Root).Path

$perfAll = Join-Path $Root "scripts\perf_all.ps1"
if (-not (Test-Path $perfAll)) { throw "Missing script: $perfAll" }

$outDir = Join-Path $Root "_perf_compat"
& $perfAll -Runs $Runs -Threads $Threads -Datasets $Datasets -OutDir $outDir -SkipVirgo -SkipBuild:$SkipBuild
if ($LASTEXITCODE -ne 0) { throw "perf_all.ps1 failed" }

$src = Join-Path $outDir "expander_exec.csv"
if (-not (Test-Path $src)) { throw "Missing expected output: $src" }

$dst = $OutputCsv
if (-not [System.IO.Path]::IsPathRooted($dst)) { $dst = Join-Path $Root $dst }
Copy-Item -Force $src $dst
Write-Host "Wrote CSV: $dst"
