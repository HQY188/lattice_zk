<#
兼容旧入口：性能测试（Ours(Orion)/Libra(Raw)，基于 `expander-exec` + `data/`）

说明：
- 旧脚本基于 `cargo test -p gkr ...` 解析输出；为了让 Ours/Libra/Virgo 用同一套“数据集 prove+verify”口径，
  这里改为直接调用 `scripts/perf_all.ps1`（并默认跳过 Virgo）。

【本脚本逐步在做什么】
1) 定位仓库根与 perf_all.ps1。
2) 调用 perf_all.ps1（-SkipVirgo），把每次 prove/verify 的耗时与 proof 大小写入临时目录下的 expander_exec.csv。
3) 将 expander_exec.csv 复制为 -OutputCsv 指定路径（默认 perf_gkr_lattice.csv）。

【用的电路】`data/circuit_<name>.txt` / `data/witness_<name>.txt`，name 为 m31、bn254、gf2 等；电路语义为 Keccak 基准（见 gkr/src/utils.rs 的 KECCAK_* 路径），由 CI/ dev_env_data_setup 等拉取或生成。

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

# 与 bench_gkr_pcs类似，但 Runs 可配置，输出目录为 _perf_compat
$outDir = Join-Path $Root "_perf_compat"
& $perfAll -Runs $Runs -Threads $Threads -Datasets $Datasets -OutDir $outDir -SkipVirgo -SkipBuild:$SkipBuild
if ($LASTEXITCODE -ne 0) { throw "perf_all.ps1 failed" }

$src = Join-Path $outDir "expander_exec.csv"
if (-not (Test-Path $src)) { throw "Missing expected output: $src" }

$dst = $OutputCsv
if (-not [System.IO.Path]::IsPathRooted($dst)) { $dst = Join-Path $Root $dst }
Copy-Item -Force $src $dst
Write-Host "Wrote CSV: $dst"
