# Windows 下 Expander 构建/测试环境配置脚本
# 用法: .\scripts\setup_env_win.ps1 [cargo 命令...]
# 示例: .\scripts\setup_env_win.ps1 run -p bin --bin dev-setup --release
# 若不加参数则只设置环境变量并进入当前 shell，然后可手动执行 cargo

param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $CargoArgs
)

$ErrorActionPreference = "Stop"

# ---------- 1. MS-MPI (编译 mpi-sys 需要) ----------
$MSMPI_INC = "C:\Program Files (x86)\Microsoft SDKs\MPI\Include"
$MSMPI_LIB64 = "C:\Program Files (x86)\Microsoft SDKs\MPI\Lib\x64"
if (Test-Path $MSMPI_INC) {
    $env:MSMPI_INC = $MSMPI_INC
    $env:MSMPI_LIB64 = $MSMPI_LIB64
    Write-Host '[OK] MS-MPI: MSMPI_INC, MSMPI_LIB64 set' -ForegroundColor Green
} else {
    Write-Host ('[WARN] MS-MPI SDK not found at: ' + $MSMPI_INC) -ForegroundColor Yellow
}

# ---------- 2. LLVM/Clang (bindgen 需要 libclang.dll，必须 64 位) ----------
$llvmPaths = @(
    "C:\Program Files\LLVM\bin",
    "D:\LLVM\bin",
    "$env:USERPROFILE\scoop\apps\llvm\current\bin"
)
$env:LIBCLANG_PATH = $null
foreach ($p in $llvmPaths) {
    if (Test-Path $p) {
        $dll = Get-ChildItem -Path $p -Filter "libclang.dll" -ErrorAction SilentlyContinue
        if ($dll) {
            $env:LIBCLANG_PATH = $p
            Write-Host ('[OK] LLVM: LIBCLANG_PATH = ' + $p) -ForegroundColor Green
            break
        }
    }
}
if (-not $env:LIBCLANG_PATH) {
    Write-Host '[WARN] libclang.dll not found. Install 64-bit LLVM and set LIBCLANG_PATH to its bin dir' -ForegroundColor Yellow
    Write-Host '       e.g. $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"' -ForegroundColor Gray
}

# ---------- 3. Visual Studio 环境 (bindgen 解析头文件需要 INCLUDE/LIB) ----------
$vsPath = "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
if (-not (Test-Path $vsPath)) {
    $vsPath = "C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat"
}
if (-not (Test-Path $vsPath)) {
    $vsPath = "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat"
}

if (Test-Path $vsPath) {
    Write-Host '[...] Loading VS 2022 x64 env...' -ForegroundColor Cyan
    $tempFile = [System.IO.Path]::GetTempFileName()
    cmd /c "`"$vsPath`" && set" | Out-File -FilePath $tempFile -Encoding ascii
    Get-Content $tempFile | ForEach-Object {
        if ($_ -match "^([^=]+)=(.*)$") {
            $key = $matches[1]
            $val = $matches[2]
            [System.Environment]::SetEnvironmentVariable($key, $val, "Process")
        }
    }
    Remove-Item $tempFile -Force -ErrorAction SilentlyContinue
    Write-Host '[OK] VS 2022 env loaded (INCLUDE, LIB)' -ForegroundColor Green
} else {
    Write-Host "[WARN] vcvars64.bat not found. Use Start Menu: x64 Native Tools Command Prompt for VS 2022" -ForegroundColor Yellow
}

# ---------- 4. 可选：bindgen 与 MSVC 兼容（若仍报 libclang error 可取消下行注释） ----------
# $env:BINDGEN_EXTRA_CLANG_ARGS = "-fms-compatibility -fms-compatibility-version=19.00"

# ---------- 5. 若传入了 cargo 参数则执行 ----------
if ($CargoArgs.Count -gt 0) {
    Write-Host ('[...] Running: cargo ' + ($CargoArgs -join ' ')) -ForegroundColor Cyan
    & cargo @CargoArgs
    exit $LASTEXITCODE
} else {
    Write-Host ""
    Write-Host "Env set. Run cargo in this session, e.g.:" -ForegroundColor Cyan
    Write-Host '  cargo run -p bin --bin dev-setup --release' -ForegroundColor White
    Write-Host '  cargo test -p gkr --release gkr_correctness' -ForegroundColor White
    Write-Host ""
}
