# Expander 测试指令说明

本文档汇总仓库内各类测试的运行方式与前置条件。

---

## 1. 前置要求

| 项目 | 说明 |
|------|------|
| **Rust** | 需安装 Cargo，建议使用 stable 或 nightly |
| **MPI** | 多数测试与二进制依赖 MPI。**Windows** 需安装 [Microsoft MPI (MS-MPI)](https://learn.microsoft.com/en-us/message-passing-interface/microsoft-mpi)，并设置 `MSMPI_INC`、`MSMPI_LIB64` |
| **测试数据** | GKR 正确性测试需要 `data/` 下的电路与 witness 文件（见下文「数据准备」） |

可选加速（推荐在 x86 上使用）：

- **AVX2**：`RUSTFLAGS="-C target-cpu=native"`
- **AVX-512**：`RUSTFLAGS="-C target-cpu=native -C target-feature=+avx512f"`（部分 CI 使用）

---

## 2. 数据准备

GKR 测试会从 `data/` 读取电路与 witness（路径以**当前工作目录**为基准，测试在 workspace 根目录运行时对应根目录下的 `data/`）。

### 方式 A：使用 dev-setup 下载（推荐）

```powershell
# 在仓库根目录执行（需已安装并配置好 MPI）
cargo run -p bin --bin dev-setup --release
```

会创建 `data/` 并从 R2 下载 circuit/witness；若文件已存在会跳过下载。使用 `--no-download` 可跳过下载仅用本地已有文件。

### 方式 B：使用 E2E 脚本生成

参考 `scripts/e2e.sh`：克隆 ExpanderCompilerCollection，生成 Keccak/Poseidon 的 circuit 与 witness，再拷贝到本仓库的 `data/`。

### 数据文件说明

- 电路：`data/circuit_m31.txt`、`data/circuit_gf2.txt`、`data/circuit_bn254.txt` 等  
- Witness：`data/witness_m31.txt`、`data/witness_gf2.txt` 等  
- 下载基址：`https://pub-3be2514d8cd1470691b87e515984f903.r2.dev`（见 `gkr/src/utils.rs`）

---

## 3. 测试命令速查

### 3.1 全工作区测试

```powershell
# 基础（AVX2）
RUSTFLAGS="-C target-cpu=native" cargo test --release --workspace

# 带 AVX-512（与部分 CI 一致）
RUSTFLAGS="-C target-cpu=native -C target-feature=+avx512f" cargo test --release --workspace

# 显示测试输出
RUSTFLAGS="-C target-cpu=native" cargo test --release --workspace -- --nocapture
```

### 3.2 GKR 正确性测试（核心端到端）

测试入口：`gkr/src/tests/gkr_correctness.rs` 中的 `test_gkr_correctness`，会跑 17 种配置（不同域 + Fiat–Shamir + 多项式承诺），其中 **C16 为 Lattice PCS（lattice_poly_commit）**。

**单进程（无 MPI 多进程）：**

```powershell
cargo test -p gkr --release gkr_correctness
```

**仅测 Lattice PCS 的 GKR 正确性（只跑 M31x16 + SHA256 + Lattice，便于快速迭代）：**

```powershell
cargo test -p gkr --release gkr_correctness_lattice
```

**带 AVX-512（推荐在支持的机器上使用）：**

```powershell
RUSTFLAGS="-C target-cpu=native -C target-feature=+avx512f" cargo +nightly test -p gkr --release gkr_correctness
```

**MPI 多进程（2 进程）：**

```powershell
RUSTFLAGS="-C target-cpu=native -C target-feature=+avx512f" mpiexec -n 2 cargo +nightly test -p gkr --release gkr_correctness
```

注意：测试内电路/witness 路径为 `../data/...`，若在**仓库根目录**运行且数据在根目录的 `data/` 下，应能正确找到；若在 `gkr` 子目录运行，则 `../data` 指向根目录的 `data/`。

### 3.3 其他包内测试

| 包 | 说明 | 命令示例 |
|----|------|----------|
| **gkr** | 系统/架构互斥检查 | `cargo test -p gkr --release test_mutually_exclusive_flags` |
| **poly_commit** | Raw PCS、Orion、Hyrax、KZG、GKR 用 PCS | `cargo test -p poly_commit --release` |
| **circuit** | 电路序列化、共享内存 | `cargo test -p circuit --release` |
| **serdes** | 序列化 | `cargo test -p serdes --release` |
| **config_macros** | 宏展开 | `cargo test -p config_macros --release` |
| **crosslayer_prototype** | 跨层 sumcheck | `cargo test -p crosslayer_prototype --release` |
| **arith** | 域运算等 | `cargo test -p arith --release` |
| **utils** | 计时等 | `cargo test -p utils --release` |

上述测试中，依赖 MPI 的会使用 `MPIConfig::init()`，需在已配置好 MPI 的环境下运行。

### 3.4 按测试名称过滤

```powershell
# 只跑名称包含 gkr_correctness 的测试
cargo test -p gkr --release gkr_correctness

# 只跑名称包含 raw 的测试（poly_commit）
cargo test -p poly_commit --release raw

# 显示 stdout
cargo test -p gkr --release gkr_correctness -- --nocapture
```

---

## 4. 二进制与 Benchmark

### 4.1 dev-setup（下载数据 / 生成并比对证明）

```powershell
cargo run -p bin --bin dev-setup --release
cargo run -p bin --bin dev-setup --release -- --no-download   # 不下载，仅用本地 data
cargo run -p bin --bin dev-setup --release -- --compare       # 生成证明并与下载的 proof 比对
```

### 4.2 GKR 证明生成（单机）

```powershell
RUSTFLAGS="-C target-cpu=native" cargo run -p bin --bin gkr --release -- -f fr -t 16 -c keccak
```

`-f`：域（如 `fr`、`m31ext3`、`gf2ext128`、`goldilocks`）；`-t`：线程数；`-c`：电路（如 `keccak`）。

### 4.3 GKR MPI 二进制（多进程）

```powershell
mpiexec -n 2 cargo run -p bin --bin gkr-mpi --release -- -c keccak -f gf2ext128
mpiexec -n 2 cargo run -p bin --bin gkr-mpi --release -- -c keccak -f m31ext3
mpiexec -n 2 cargo run -p bin --bin gkr-mpi --release -- -c keccak -f fr
```

### 4.4 expander-exec（证明 / 验证 / 服务）

```powershell
# 生成证明
RUSTFLAGS="-C target-cpu=native" cargo run -p bin --bin expander-exec --release -- prove -c ./data/circuit_m31.txt -w ./data/witness_m31.txt -o ./data/out_m31.bin

# 验证
RUSTFLAGS="-C target-cpu=native" cargo run -p bin --bin expander-exec --release -- verify -c ./data/circuit_m31.txt -w ./data/witness_m31.txt -i ./data/out_m31.bin

# 指定哈希与 PCS（需与 prove 一致）
RUSTFLAGS="-C target-cpu=native" cargo run -p bin --bin expander-exec --release -- -f SHA256 -p Raw prove -c <circuit> -w <witness> -o <proof>
```

使用 MPI 时可在命令前加 `mpiexec -n 1`。

### 4.5 Benchmark 脚本

```bash
# Linux/macOS：运行脚本（需在仓库根目录，且已编译出 target/release/gkr）
chmod +x scripts/run_benchmarks.sh
./scripts/run_benchmarks.sh
```

脚本会按不同 field/PCS/circuit 组合调用 `gkr` 二进制。

### 4.6 GKR + PCS 性能对照（Raw vs Lattice）

同一电路（M31x16，32 keccak/proof）下对比 **Raw PCS** 与 **Lattice PCS** 的证明时间、验证时间与 Proof 大小，并运行 criterion 证明耗时 bench。

**PowerShell（仓库根目录）：**

```powershell
.\scripts\bench_gkr_pcs.ps1
.\scripts\bench_gkr_pcs.ps1 -OutputCsv bench_gkr_pcs.csv
.\scripts\bench_gkr_pcs.ps1 -SkipBench   # 只跑 test 计时，不跑 criterion
```

脚本会依次运行 `gkr_correctness_raw` 与 `gkr_correctness_lattice`，解析输出中的 Proving time、Proof size、Verification time、Multi-core Verification time，并打印对照表；若未加 `-SkipBench`，还会执行 `cargo bench -p gkr "GKR proving M31x16"` 输出 Criterion 的证明耗时统计。

**单独跑 Criterion 证明 bench：**

```powershell
cargo bench -p gkr "GKR proving M31x16"
```

（`cargo bench` 默认使用优化的 bench profile，无需加 `--release`；过滤串为 cargo 参数，不要写在 `--` 后面。）

**单独跑 Raw / Lattice 正确性（带计时输出）：**

```powershell
cargo test -p gkr --release gkr_correctness_raw -- --nocapture
cargo test -p gkr --release gkr_correctness_lattice -- --nocapture
```

### 4.7 完整性能测试：Lattice PCS 对 GKR 效率的影响

目标：在同一电路、同一环境下对比 **Raw PCS**（仅做对照）与 **Lattice PCS** 下 GKR 的**证明时间、验证时间、Proof 大小**，量化 lattice_poly_commit 对整体 GKR 效率的影响。

**前置条件**

- 已安装 MPI、配置好 `data/`（见 2 与 3.2）。
- 建议在 x86 上使用本地 CPU 优化：  
  `$env:RUSTFLAGS="-C target-cpu=native"`（PowerShell）后再执行下列命令。
- Windows 下若需 MS-MPI/VS 环境，先执行 `.\scripts\setup_env_win.ps1`。

**步骤一：一键对照（推荐）**

在仓库根目录执行：

```powershell
.\scripts\bench_gkr_pcs.ps1 -OutputCsv bench_gkr_pcs.csv
```

脚本会：

1. 跑一次 **Raw** 正确性测试，记录 Proving(μs)、Proof(bytes)、Verify(μs)、ParVerify(μs)。
2. 跑一次 **Lattice** 正确性测试，记录同样四项。
3. 跑 **Criterion** 证明 bench（约 1 分钟，Raw 与 Lattice 各约 30s 测量、100 样本），得到证明耗时的均值/置信区间。

终端会打印一张对照表；同时结果会写入 `bench_gkr_pcs.csv`，便于多次运行后对比或画图。

**步骤二：只看单次 test 计时（不跑 Criterion）**

若只想快速看一次证明/验证/Proof 大小、不关心 Criterion 统计：

```powershell
.\scripts\bench_gkr_pcs.ps1 -SkipBench -OutputCsv bench_gkr_pcs.csv
```

**步骤三：只跑 Criterion 证明耗时**

若已跑过步骤一，只想重新测证明耗时（多轮统计）：

```powershell
cargo bench -p gkr "GKR proving M31x16"
```

**如何解读「Lattice 对 GKR 效率的影响」**

| 指标 | 含义 | 对照方式 |
|------|------|----------|
| **Proving(μs)** | 单次证明耗时 | 表中 Raw vs Lattice；Criterion 输出为多轮均值 [low, mid, high] ms |
| **Proof(bytes)** | 证明体积 | Lattice 会大于 Raw（含 PCS opening），看绝对值与倍数 |
| **Verify(μs)** | 单线程验证耗时 | Lattice 验证需做 MLE verify，一般高于 Raw |
| **ParVerify(μs)** | 多核验证耗时 | 同上，看多核下的验证开销 |

- **证明开销**：`(Lattice_proving − Raw_proving) / Raw_proving` 或直接用 Criterion 的 Raw vs Lattice 的 time 比值。
- **验证开销**：同上，用 Verify 或 ParVerify 的差值/比值。
- **体积开销**：`Lattice_proof_bytes / Raw_proof_bytes`。

多次运行或多次执行脚本后，可对 CSV 做平均或取中位数，再算上述比值，得到更稳定的「lattice_poly_commit 对 GKR 效率的影响」结论。

**结果是否完整**：只要终端里出现了「单次运行结果」表、Criterion 的 Raw/Lattice 两行 `time: [low mid high] ms`，以及「Wrote CSV」，则数据是完整的。中间若出现一次 `RemoteException`（因 cargo 把 "Compiling/Finished" 写到 stderr），在 `$ErrorActionPreference = Continue` 下脚本会继续跑完，不影响结果。

**示例解读**（某次典型跑法）：

| PCS     | Proving(μs) | Proof(bytes) | Verify(μs) | ParVerify(μs) |
|---------|-------------|--------------|------------|---------------|
| Raw     | ~277k       | 224844       | ~38.5k     | ~38.5k        |
| Lattice | ~265k       | 363116       | ~49k       | ~49k          |

Criterion 证明中位数约：Raw 268 ms，Lattice 272 ms。

- **证明**：Lattice 与 Raw 证明时间同量级，Criterion 下 Lattice 略慢约 1–2%，说明 lattice_poly_commit 在证明端带来的额外开销很小。
- **验证**：Lattice 验证约比 Raw 慢 27%（49k vs 38.5k μs），因验证方需做 MLE 的 verify。
- **体积**：Lattice proof 约 363 KB，Raw 约 225 KB，约 1.6×，来自 Lattice opening（承诺 + 证明）的额外数据。

---

## 5. E2E 脚本（Linux/macOS）

`scripts/e2e.sh` 会：克隆 ExpanderCompilerCollection、生成 circuit/witness，再克隆 Expander、拷贝数据并执行本地测试与 MPI 测试。适合在类 Unix 环境下做完整流水线，Windows 下需按步骤手动执行或改写为 PowerShell。

---

## 6. Windows 构建环境（MPI + bindgen/libclang）

编译依赖 **MS-MPI SDK**、**64 位 LLVM/Clang**（libclang.dll）和 **Visual Studio 2022**（含 C++ 工作负载）。**每次在新开的普通 PowerShell 中编译前**，都需要先加载这些环境，否则会报 `libclang error`（因 INCLUDE/LIB 未设置）。推荐用仓库脚本：

```powershell
# 在仓库根目录执行（PowerShell）
.\scripts\setup_env_win.ps1 run -p bin --bin dev-setup --release
```

或先设环境、再在本会话内执行任意 cargo 命令：

```powershell
.\scripts\setup_env_win.ps1
cargo run -p bin --bin dev-setup --release
```

脚本会设置 `MSMPI_INC`、`MSMPI_LIB64`、`LIBCLANG_PATH` 并加载 VS 的 `vcvars64`（INCLUDE/LIB）。**新开一个终端后需重新执行脚本**；也可改用开始菜单中的「x64 Native Tools Command Prompt for VS 2022」再运行 cargo。

**手动设置时请确保：**

1. **MS-MPI SDK**：`MSMPI_INC` = `C:\Program Files (x86)\Microsoft SDKs\MPI\Include`，`MSMPI_LIB64` = `C:\Program Files (x86)\Microsoft SDKs\MPI\Lib\x64`
2. **64 位 LLVM**：安装后设置 `LIBCLANG_PATH` 为包含 `libclang.dll` 的 bin 目录（如 `C:\Program Files\LLVM\bin`）
3. **VS 环境**：在「x64 Native Tools Command Prompt for VS 2022」中运行，或先运行脚本再执行 cargo，这样 bindgen 才能正确解析 MPI/Windows 头文件

若仍报 `libclang error`（Invalid flag / Host vs. target mismatch），请确认 LLVM 为 64 位、与 VS 同机安装，并在该 VS 的 x64 环境下编译。

---

## 7. 常见问题

- **找不到 `cargo`**：检查 PATH 是否包含 Cargo（如 `$env:USERPROFILE\.cargo\bin`），或在新终端/重启 Cursor 后重试。
- **MPI 相关编译错误（如 `MSMPI_INC`/`MSMPI_LIB64` 未找到）**：安装 MS-MPI SDK 并设置上述环境变量，或确认安装路径与文档一致。
- **bindgen/libclang 报错**：安装 64 位 LLVM、设置 `LIBCLANG_PATH`，并在已加载 VS 2022 x64 环境（如运行 `setup_env_win.ps1` 或使用 x64 Native Tools 终端）下编译。
- **测试报错找不到 circuit/witness**：确保已运行 `dev-setup` 或手动准备好 `data/`，且在**仓库根目录**执行测试（或保证 `../data` 指向包含数据的目录）。
- **只想跑部分 GKR 配置**：当前需在 `gkr_correctness.rs` 中注释掉不需要的 `test_gkr_correctness_helper::<Cx>(...)` 再运行测试。

---

## 8. 参考

- 使用示例与流程说明：根目录 [readme.md](../readme.md)
- GKR 正确性测试实现：[gkr/src/tests/gkr_correctness.rs](../gkr/src/tests/gkr_correctness.rs)
- 数据下载与路径常量：[gkr/src/utils.rs](../gkr/src/utils.rs)
