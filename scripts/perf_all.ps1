<#
统一性能测试脚本：Ours / Libra / Virgo

Ours & Libra：
- 基于本仓库 `bin/expander-exec`
- 对 `data/` 下同一电路/见证做 prove+verify
- 记录 prove/verify wall-clock 时间 + proof 文件大小

Virgo：
- 优先通过 WSL 运行 `Virgo/tests/SHA256` 的 `build.py` + `run.py`
- 解析其 `LOG/SHA256_*.txt`（每行 5 列，最后一列为 proof_size）

【执行流程（从上到下）】
1) 解析 -OutDir，在仓库下创建输出目录。
2) 若未 -SkipExpander：Release 预编译 expander-exec（可选 -SkipBuild 跳过）。
3) 对每个 Dataset（如 m31）：读 `data/circuit_<dataset>.txt` 与 `data/witness_<dataset>.txt`，对 PCS=Orion 与 PCS=Raw 各跑 -Runs 次：
   `cargo run ... expander-exec -- -f SHA256 -p <PCS> prove/verify`（电路文件决定域类型；这些文件是 Keccak 基准的二进制序列化，见 gkr/src/utils.rs）。
4) 将每行结果导出为 expander_exec.csv。
5) 若未 -SkipVirgo 且本机有 WSL：在 Virgo/tests/SHA256 下跑 Python 基准，解析 LOG 得到 virgo_sha256.csv。
6) 写 summary.md 汇总表。

【依赖】Run-Cargo 会调用 scripts\setup_env_win.ps1（若不存在需自行配置 cargo/MPI 环境）。

用法（仓库根目录）：
  .\scripts\perf_all.ps1 -Runs 3 -Threads 1 -Datasets m31,bn254,gf2 -OutDir perf_out
#>

param(
    [int] $Runs = 3,
    [int] $Threads = 1,
    [string[]] $Datasets = @('m31', 'bn254', 'gf2'),
    [string] $OutDir = 'perf_out',
    [switch] $SkipBuild = $false,
    [switch] $SkipVirgo = $false,
    [switch] $SkipExpander = $false
)

$ErrorActionPreference = 'Stop'

$Root = if ($PSScriptRoot) { Join-Path $PSScriptRoot '..' } else { '..' }
$Root = (Resolve-Path $Root).Path

function Ensure-Dir([string]$path) {
    if (-not (Test-Path $path)) { New-Item -ItemType Directory -Path $path | Out-Null }
}

function Assert-File([string]$path, [string]$hint) {
    if (-not (Test-Path $path)) { throw ('Missing file: ' + $path + '. ' + $hint) }
}

# 通过 setup_env_win.ps1 注入 Windows 下 cargo/MPI 等环境后再执行 cargo（与仓库其它脚本一致）
function Run-Cargo([string[]]$cargoArgs) {
    $setup = Join-Path $Root 'scripts\setup_env_win.ps1'
    Assert-File $setup 'Run from repo root, or ensure scripts/setup_env_win.ps1 exists.'
    if ($Threads -gt 0) { $env:RAYON_NUM_THREADS = [string]$Threads }
    & $setup @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw ('cargo failed: cargo ' + ($cargoArgs -join ' ')) }
}

function Stopwatch-Ms([scriptblock]$action) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    & $action
    $sw.Stop()
    return [long]$sw.Elapsed.TotalMilliseconds
}

# 将逻辑名（m31/bn254/gf2）映射到 data 下成对的路径
function Get-ExpanderDataset([string]$name) {
    $circuit = Join-Path $Root ('data\circuit_' + $name + '.txt')
    $witness = Join-Path $Root ('data\witness_' + $name + '.txt')
    Assert-File $circuit 'Expected file under data/.'
    Assert-File $witness 'Expected file under data/.'
    return [PSCustomObject]@{ Name = $name; Circuit = $circuit; Witness = $witness }
}

function Expander-Prebuild() {
    if ($SkipBuild) { return }
    Write-Host '[expander] prebuild expander-exec (exclude compile time)...'
    Run-Cargo @('build', '-p', 'bin', '--bin', 'expander-exec', '--release')
}

# 单次：prove（计时）-> 读 proof 字节数 -> verify（计时）
function Expander-RunOnce([string]$impl, [string]$pcs, [object]$ds, [int]$runIndex, [string]$outDirAbs) {
    $proofDir = Join-Path $outDirAbs 'proofs'
    Ensure-Dir $proofDir
    $proofPath = Join-Path $proofDir ($impl + '_' + $pcs + '_' + $ds.Name + '_run' + $runIndex + '.bin')

    # -f SHA256：Fiat-Shamir 用 SHA256；电路本身仍由 -c 指定的二进制文件决定（Keccak 基准）
    $common = @('run', '-p', 'bin', '--bin', 'expander-exec', '--release', '--', '-f', 'SHA256', '-p', $pcs)
    $proveArgs = $common + @('prove', '-c', $ds.Circuit, '-w', $ds.Witness, '-o', $proofPath)
    $verifyArgs = $common + @('verify', '-c', $ds.Circuit, '-w', $ds.Witness, '-i', $proofPath)

    $proveMs = Stopwatch-Ms { Run-Cargo $proveArgs }
    $proofBytes = (Get-Item $proofPath).Length
    $verifyMs = Stopwatch-Ms { Run-Cargo $verifyArgs }

    return [PSCustomObject]@{
        Impl = $impl
        PCS = $pcs
        Dataset = $ds.Name
        RunIndex = $runIndex
        Threads = $Threads
        ProveMs = $proveMs
        VerifyMs = $verifyMs
        ProofBytes = [long]$proofBytes
        CircuitPath = $ds.Circuit
        WitnessPath = $ds.Witness
    }
}

function Run-ExpanderBench([string]$outDirAbs) {
    Expander-Prebuild
    $rows = @()
    $impls = @(
        @{ Impl = 'Ours(Orion)';  PCS = 'Orion' },
        @{ Impl = 'Libra(Raw)';   PCS = 'Raw' }
    )

    foreach ($dsName in $Datasets) {
        $ds = Get-ExpanderDataset $dsName
        for ($i = 0; $i -lt $Runs; $i++) {
            foreach ($it in $impls) {
                $rows += Expander-RunOnce $it.Impl $it.PCS $ds $i $outDirAbs
            }
        }
    }
    return $rows
}

function Test-HasWsl {
    try {
        & wsl -e bash -lc 'echo ok' 2>$null | Out-Null
        return $LASTEXITCODE -eq 0
    } catch {
        return $false
    }
}

function Run-VirgoSha256() {
    if ($SkipVirgo) { return @() }
    if (-not (Test-HasWsl)) {
        Write-Warning 'Virgo benchmark skipped: WSL not available.'
        return @()
    }

    $virgoShaDirWin = Join-Path $Root 'Virgo\tests\SHA256'
    Assert-File (Join-Path $virgoShaDirWin 'build.py') 'Virgo tests/SHA256 not found.'

    $rootNorm = ($Root -replace '\\', '/')
    if ($rootNorm -notmatch '^([A-Za-z]):/(.*)$') { throw ('Unexpected Root path for WSL conversion: ' + $Root) }
    $drive = $Matches[1].ToLower()
    $rest = $Matches[2]
    $shaWsl = ('/mnt/' + $drive + '/' + $rest + '/Virgo/tests/SHA256')

    Write-Host '[virgo] build circuits + compile (WSL)...'
    & wsl -e bash -lc ('cd ' + [string]$shaWsl + ' && python3 build.py')
    if ($LASTEXITCODE -ne 0) { throw 'Virgo build.py failed in WSL.' }

    Write-Host '[virgo] run SHA256 suite (WSL)...'
    & wsl -e bash -lc ('cd ' + [string]$shaWsl + ' && python3 run.py')
    if ($LASTEXITCODE -ne 0) { throw 'Virgo run.py failed in WSL.' }

    $logDir = Join-Path $virgoShaDirWin 'LOG'
    if (-not (Test-Path $logDir)) { throw ('Virgo LOG directory not found: ' + $logDir) }

    $pick = @(1, 4, 8)
    $rows = @()
    foreach ($k in $pick) {
        $p = Join-Path $logDir ('SHA256_' + $k + '.txt')
        Assert-File $p 'Virgo output missing.'
        $line = (Get-Content $p -Raw).Trim()
        $parts = $line -split '\s+'
        if ($parts.Length -lt 5) { throw ('Unexpected Virgo log format in ' + $p + ': ' + $line) }
        $rows += [PSCustomObject]@{
            Impl = 'Virgo'
            Suite = 'SHA256_64_merkle'
            Case = $k
            ProveSec = [double]$parts[0]
            VerifySec = [double]$parts[1]
            ProofBytes = [int]$parts[4]
            LogPath = $p
        }
    }
    return $rows
}

function Write-Summary([object[]]$expanderRows, [object[]]$virgoRows, [string]$outDirAbs) {
    $md = @()
    $md += @'
## 性能测试汇总

'@
    $md += @'
- **Ours/Libra**: `bin/expander-exec` prove+verify，数据来自 `data/`
- **Virgo**: WSL 下运行 `Virgo/tests/SHA256`，解析其 LOG 文件

'@

    if ($expanderRows.Count -gt 0) {
        $md += @'
### Ours(Orion) vs Libra(Raw)（expander-exec）

| Impl | PCS | Dataset | Runs | Prove(ms) avg | Verify(ms) avg | Proof(bytes) avg |
|---|---|---:|---:|---:|---:|---:|
'@
        $groups = $expanderRows | Group-Object Impl,PCS,Dataset
        foreach ($g in $groups) {
            $r = $g.Group
            $avgProv = [long](($r | Measure-Object ProveMs -Average).Average)
            $avgVer = [long](($r | Measure-Object VerifyMs -Average).Average)
            $avgSize = [long](($r | Measure-Object ProofBytes -Average).Average)
            $key = $g.Name -split ','
            $md += ('| {0} | {1} | {2} | {3} | {4} | {5} | {6} |' -f $key[0].Trim(), $key[1].Trim(), $key[2].Trim(), $r.Count, $avgProv, $avgVer, $avgSize)
        }
        $md += ''
    }

    if ($virgoRows.Count -gt 0) {
        $md += @'
### Virgo（SHA256 suite）

| Case | Prove(s) | Verify(s) | Proof(bytes) |
|---:|---:|---:|---:|
'@
        foreach ($r in ($virgoRows | Sort-Object Case)) {
            $md += ('| {0} | {1} | {2} | {3} |' -f $r.Case, $r.ProveSec, $r.VerifySec, $r.ProofBytes)
        }
        $md += ''
    }

    $summaryPath = Join-Path $outDirAbs 'summary.md'
    ($md -join [System.Environment]::NewLine) | Set-Content -Path $summaryPath -Encoding UTF8
    Write-Host ('Wrote summary: ' + $summaryPath)
}

$outDirAbs = $OutDir
if (-not [System.IO.Path]::IsPathRooted($outDirAbs)) { $outDirAbs = Join-Path $Root $OutDir }
Ensure-Dir $outDirAbs

$expanderRows = @()
$virgoRows = @()

if (-not $SkipExpander) {
    Write-Host '== Running Ours/Libra (expander-exec) =='
    $expanderRows = Run-ExpanderBench $outDirAbs
    $expanderCsv = Join-Path $outDirAbs 'expander_exec.csv'
    $expanderRows | Export-Csv -Path $expanderCsv -NoTypeInformation -Encoding UTF8
    Write-Host ('Wrote CSV: ' + $expanderCsv)
}

$virgoRows = Run-VirgoSha256
if ($virgoRows.Count -gt 0) {
    $virgoCsv = Join-Path $outDirAbs 'virgo_sha256.csv'
    $virgoRows | Export-Csv -Path $virgoCsv -NoTypeInformation -Encoding UTF8
    Write-Host ('Wrote CSV: ' + $virgoCsv)
}

Write-Summary $expanderRows $virgoRows $outDirAbs

