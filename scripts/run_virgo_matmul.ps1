<#
.SYNOPSIS
  将 new_data 下 Expander 二进制 matmul 电路（CIRCUIT6）经 expander_to_virgo 转为 Virgo 文本后，调用 zk_proof 统计 Prover / Verification / Proof size。

.DESCRIPTION
  Virgo 调用链（便于对照源码）:
    main_zk.cpp: prime_field::init -> zk_verifier::read_circuit -> zk_prover::get_circuit
    -> zk_verifier::verify(log) -> zk_prover::evaluate -> 各层 sumcheck -> VPD/poly_commit。

  zk_proof 必须从包含 fft_gkr 的目录启动（Windows 下为 fft_gkr.exe）。若存在 Virgo/build-mingw/zk_proof.exe 与 fft_gkr.exe，则优先在 Windows 上运行；否则可选用 WSL 与 Virgo/build-linux。

  new_data 的 matmul 若含「同一输出位置多条门」，expander_to_virgo 可能转换失败（见工具报错）；此时行内会记 ConvertFailed，需调整电路或转换器。

.PARAMETER RepoRoot
  仓库根目录（默认为本脚本上一级）。

.PARAMETER NewDataDir
  相对 RepoRoot，默认 new_data。

.PARAMETER CircuitGlob
  匹配电路文件名，默认 *matmul*circuit*.txt。

.PARAMETER UseWslZkProof
  为 $true 时使用 WSL 运行 RepoRoot/Virgo/build-linux/zk_proof（需已 cmake 构建 zk_proof 与 fft_gkr）。为 $false 时仅查找 Windows 版 zk_proof.exe（须自行保证 cwd 含 fft_gkr）。

.PARAMETER OutCsv
  可选，写入 UTF-8 CSV 结果表。

.PARAMETER SkipConversion
  跳过 expander_to_virgo；将匹配到的文件视为已是 Virgo 文本电路，并在同目录查找 {prefix}_meta.txt（prefix 为去掉 _circuit 后的基名）。用于 Virgo/tests/matmul 下 gen 生成的 mat_*。

.NOTES
  指标来源: zk_verifier::verify 写入日志文件首行五个数:
    col1=prover_sec, col2=verification_time, col3=predicates, col4=verification_rdl_time, col5=proof_bytes
  与界面一致的 Verification Time = col2 - col4（脚本列 VerifySec 使用该差值）。
#>
param(
  [string]$RepoRoot = "",
  [string]$NewDataDir = "new_data",
  [string]$CircuitGlob = "*matmul*circuit*.txt",
  [string]$VirgoRelBuildDir = "Virgo\build-linux",
  [string]$ConverterExe = "",
  [string]$ZkProofExe = "",
  [switch]$UseWslZkProof,
  [string]$WorkRelDir = "results\virgo_matmul_work",
  [string]$OutCsv = "",
  [switch]$SkipConversion
)

$ErrorActionPreference = "Stop"

function Get-RepoRoot {
  if ($PSScriptRoot) {
    return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
  }
  return (Get-Location).Path
}

function ConvertTo-WslPath([string]$WindowsPath) {
  if (Test-Path -LiteralPath $WindowsPath) {
    $full = (Resolve-Path -LiteralPath $WindowsPath).Path
  }
  else {
    $full = [System.IO.Path]::GetFullPath($WindowsPath)
  }
  $full = $full -replace '\\', '/'
  if ($full -match '^([A-Za-z]):(/.*)$') {
    $d = $Matches[1].ToLower()
    return "/mnt/$d$($Matches[2])"
  }
  throw "Cannot convert to WSL path: $WindowsPath"
}

if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
  $RepoRoot = Get-RepoRoot
}
$RepoRoot = (Resolve-Path $RepoRoot).Path

$nd = Join-Path $RepoRoot $NewDataDir
if (!(Test-Path $nd)) {
  Write-Error "NewDataDir not found: $nd"
}

$workDir = Join-Path $RepoRoot $WorkRelDir
New-Item -ItemType Directory -Force -Path $workDir | Out-Null

# expander_to_virgo（-SkipConversion 时不需要）
$conv = $ConverterExe
if ([string]::IsNullOrWhiteSpace($conv)) {
  foreach ($p in @(
      (Join-Path $RepoRoot "target\release\expander_to_virgo.exe"),
      (Join-Path $RepoRoot "target\debug\expander_to_virgo.exe"),
      (Join-Path $RepoRoot "target\release\expander_to_virgo")
    )) {
    if (Test-Path $p) { $conv = (Resolve-Path $p).Path; break }
  }
}
if (-not $SkipConversion -and -not $conv) {
  Write-Error "expander_to_virgo not found. Run: cargo build -p expander_to_virgo --release"
}

$buildMingw = Join-Path $RepoRoot "Virgo\build-mingw"
$zkMingw = Join-Path $buildMingw "zk_proof.exe"
$fftMingw = Join-Path $buildMingw "fft_gkr.exe"

$buildLinux = Join-Path $RepoRoot $VirgoRelBuildDir
$zkLinux = Join-Path $buildLinux "zk_proof"
$fftLinux = Join-Path $buildLinux "fft_gkr"
$useWsl = $false

if ($UseWslZkProof) {
  if ((Test-Path $zkLinux) -and (Test-Path $fftLinux)) {
    $useWsl = $true
  }
  else {
    Write-Error "UseWslZkProof set but Virgo/build-linux missing zk_proof or fft_gkr."
  }
}
elseif (-not [string]::IsNullOrWhiteSpace($ZkProofExe)) {
  $useWsl = $false
}
elseif ((Test-Path $zkMingw) -and (Test-Path $fftMingw)) {
  $useWsl = $false
  $ZkProofExe = (Resolve-Path $zkMingw).Path
}
elseif ((Test-Path $zkLinux) -and (Test-Path $fftLinux)) {
  $useWsl = $true
}
else {
  foreach ($p in @(
      (Join-Path $RepoRoot "Virgo\zk_proof.exe"),
      (Join-Path $RepoRoot "Virgo\zk_proof")
    )) {
    if (Test-Path $p) { $ZkProofExe = (Resolve-Path $p).Path; break }
  }
  if (-not $ZkProofExe) {
    Write-Error "No zk_proof + fft_gkr: run scripts\build_virgo.ps1 (MinGW), or build Virgo/build-linux under WSL, or set -ZkProofExe."
  }
}

Write-Host "[run_virgo_matmul] RepoRoot=$RepoRoot"
Write-Host "[run_virgo_matmul] SkipConversion=$SkipConversion Converter=$conv"
if ($useWsl) {
  Write-Host "[run_virgo_matmul] zk_proof via WSL: $(ConvertTo-WslPath $zkLinux) (cwd=$(ConvertTo-WslPath $buildLinux))"
}
else {
  Write-Host "[run_virgo_matmul] zk_proof=$ZkProofExe (ensure fft_gkr is in the same working directory as upstream Virgo expects)"
}

$circuits = Get-ChildItem -Path $nd -Filter $CircuitGlob -File
if ($circuits.Count -eq 0) {
  Write-Warning "No files matching $CircuitGlob under $nd"
  exit 0
}

$rows = New-Object System.Collections.Generic.List[object]

foreach ($cf in $circuits) {
  $baseName = $cf.BaseName
  # 与 expander_to_virgo 输出名 {prefix}_circuit.txt 对齐，避免 matmul_*_circuit_circuit.txt
  $prefix = if ($baseName -match '_circuit$') { $baseName -replace '_circuit$', '' } else { $baseName }
  $wit = Join-Path $nd ($prefix + "_witness.txt")
  $convDir = Join-Path $workDir $prefix
  New-Item -ItemType Directory -Force -Path $convDir | Out-Null
  $logOut = Join-Path $convDir "${prefix}_virgo.log"

  $circuitOut = $null
  $metaOut = $null

  if ($SkipConversion) {
    $circuitOut = $cf.FullName
    $metaOut = Join-Path $cf.DirectoryName ($prefix + "_meta.txt")
    if (!(Test-Path $metaOut)) {
      Write-Host "SkipConversion: missing meta: $metaOut"
      $rows.Add([pscustomobject]@{
          Circuit = $cf.Name; Status = "MissingMeta"; ProverSec = $null; VerifySec = $null; ProofBytes = $null; Pass = $null; LogPath = $null
        })
      continue
    }
  }
  else {
    $metaOut = Join-Path $convDir "${prefix}_meta.txt"
    $circuitOut = Join-Path $convDir "${prefix}_circuit.txt"

    $argList = @(
      "--input", $cf.FullName,
      "--field", "m31",
      "--out-dir", $convDir,
      "--prefix", $prefix
    )
    if (Test-Path $wit) {
      $argList += @("--witness", $wit)
    }

    $convErr = Join-Path $convDir "expander_stderr.txt"
    $p = Start-Process -FilePath $conv -ArgumentList $argList -NoNewWindow -Wait -PassThru -RedirectStandardError $convErr -RedirectStandardOutput (Join-Path $convDir "expander_stdout.txt")
    if ($p.ExitCode -ne 0) {
      $errText = ""
      if (Test-Path $convErr) { $errText = Get-Content -Raw -Encoding UTF8 $convErr }
      Write-Host "ConvertFailed exit=$($p.ExitCode)"
      if ($errText) { Write-Host $errText }
      $rows.Add([pscustomobject]@{
          Circuit     = $cf.Name
          Status      = "ConvertFailed"
          ProverSec   = $null
          VerifySec   = $null
          ProofBytes  = $null
          Pass        = $null
          LogPath     = $null
        })
      continue
    }

    if (!(Test-Path $circuitOut) -or !(Test-Path $metaOut)) {
      Write-Host "ConvertFailed: missing output files"
      $rows.Add([pscustomobject]@{
          Circuit = $cf.Name; Status = "ConvertFailed"; ProverSec = $null; VerifySec = $null; ProofBytes = $null; Pass = $null; LogPath = $null
        })
      continue
    }
  }

  Write-Host ""
  Write-Host "=== $($cf.Name) ==="

  if ($useWsl) {
    $wC = ConvertTo-WslPath $circuitOut
    $wM = ConvertTo-WslPath $metaOut
    $wL = ConvertTo-WslPath $logOut
    $wBuild = ConvertTo-WslPath $buildLinux
    $cmd = "cd `"$wBuild`" && ./zk_proof `"$wC`" `"$wM`" `"$wL`" 2>&1"
    $out = wsl -e bash -lc $cmd
    $exit = $LASTEXITCODE
    $out | Write-Host
  }
  else {
    $buildDir = Split-Path -Parent $ZkProofExe
    if (-not (Test-Path (Join-Path $buildDir "fft_gkr.exe")) -and -not (Test-Path (Join-Path $buildDir "fft_gkr"))) {
      Write-Warning "fft_gkr not next to zk_proof; Virgo may crash."
    }
    # 使用 cmd 合并 stderr，避免 PowerShell 将 zk_proof 的 stderr 当作 NativeCommandError 中断（$ErrorActionPreference=Stop 时）。
    $out = cmd /c "cd /d `"$buildDir`" && `"$ZkProofExe`" `"$circuitOut`" `"$metaOut`" `"$logOut`" 2>&1"
    Write-Host $out
    $exit = $LASTEXITCODE
  }

  $pass = $false
  if ($exit -eq 0 -and (Test-Path $logOut)) {
    $line = Get-Content -LiteralPath $logOut -TotalCount 1 -ErrorAction SilentlyContinue
    if ($line -match '^\s*([\d.eE+-]+)\s+([\d.eE+-]+)\s+([\d.eE+-]+)\s+([\d.eE+-]+)\s+(\d+)\s*$') {
      $pSec = [double]$Matches[1]
      $vRaw = [double]$Matches[2]
      $vRdl = [double]$Matches[4]
      $verifySec = $vRaw - $vRdl
      $proofB = [int64]$Matches[5]
      $pass = $true
      $rows.Add([pscustomobject]@{
          Circuit    = $cf.Name
          Status     = "OK"
          ProverSec  = $pSec
          VerifySec  = $verifySec
          ProofBytes = $proofB
          Pass       = "Pass"
          LogPath    = $logOut
        })
    }
    else {
      $rows.Add([pscustomobject]@{
          Circuit = $cf.Name; Status = "LogParseFailed"; ProverSec = $null; VerifySec = $null; ProofBytes = $null; Pass = "Fail"; LogPath = $logOut
        })
    }
  }
  else {
    $rows.Add([pscustomobject]@{
        Circuit = $cf.Name; Status = "ZkProofFailed"; ProverSec = $null; VerifySec = $null; ProofBytes = $null; Pass = "Fail"; LogPath = $logOut
      })
  }
}

Write-Host ""
Write-Host "======== Summary ========"
$rows | Format-Table -AutoSize

if ($OutCsv) {
  $csvPath = if ([System.IO.Path]::IsPathRooted($OutCsv)) { $OutCsv } else { Join-Path $RepoRoot $OutCsv }
  $rows | Export-Csv -LiteralPath $csvPath -Encoding UTF8 -NoTypeInformation
  Write-Host "[run_virgo_matmul] Wrote $csvPath"
}
