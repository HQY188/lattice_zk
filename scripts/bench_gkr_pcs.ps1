<#
兼容旧入口：单次对照（Ours(Orion) vs Libra(Raw)）

旧脚本对照的是 Raw vs Lattice PCS（基于 gkr correctness test）。现在统一口径到 `data/` + `expander-exec` 的 prove+verify：
- Ours(Orion)  : PCS=Orion
- Libra(Raw)   : PCS=Raw

【本脚本逐步在做什么】
1) 解析仓库根目录，定位 scripts\perf_all.ps1。
2) 调用 perf_all.ps1：Runs=1，默认只测 expander-exec（-SkipVirgo），在 _bench_compat 下产出原始 CSV。
3) 从 _bench_compat\expander_exec.csv 读取结果；若指定 -OutputCsv 则复制到目标路径，否则在控制台表格打印。

【用的电路】与 perf_all 相同：`data/circuit_<dataset>.txt` + `data/witness_<dataset>.txt`（内容为 Keccak 基准电路的二进制序列化，非 Virgo 文本电路；FS 哈希由 expander-exec 的 -f 指定，perf_all 里为 SHA256）。

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
# 1) 仓库根：脚本所在目录的上一级
$Root = if ($PSScriptRoot) { Join-Path $PSScriptRoot ".." } else { ".." }
$Root = (Resolve-Path $Root).Path

$perfAll = Join-Path $Root "scripts\perf_all.ps1"
if (-not (Test-Path $perfAll)) { throw "Missing script: $perfAll" }

# 2) 委托统一性能脚本（单次运行、跳过 Virgo）
$outDir = Join-Path $Root "_bench_compat"
& $perfAll -Runs 1 -Threads $Threads -Datasets $Datasets -OutDir $outDir -SkipVirgo -SkipBuild:$SkipBuild
if ($LASTEXITCODE -ne 0) { throw "perf_all.ps1 failed" }

# 3) 汇总输出：复制或展示 CSV
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
