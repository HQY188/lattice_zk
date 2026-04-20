# expander-gen：参数化电路 → Expander `.txt`

本目录是**独立 Go 模块**（不属于上层 Rust `cargo workspace`）。通过 **ExpanderCompilerCollection (`ecgo`) + gnark** 编译电路，并写出与仓库 `data/circuit_*.txt` / `data/witness_*.txt` **相同二进制格式**的文件（`RecursiveCircuit` / `Witness` 序列化，扩展名 `.txt` 仅为历史命名）。

## 构建要求

`ecgo` 依赖 **cgo**（`ecgo/rust/wrapper` 使用 `import "C"`）。在 **Windows** 上若未安装 C 编译器，会出现 `build constraints exclude all Go files` 或 `gcc not found`。

- **推荐**：在 **Linux** 或 **WSL2**（安装 `gcc`、`go`）下构建：`CGO_ENABLED=1 go build -o expander-gen ./cmd/expander-gen`
- Windows原生：安装 **MSYS2 MinGW-w64** 等并将 `gcc` 加入 `PATH`，并设置 `CGO_ENABLED=1`

依赖版本与 [`recursion/go.mod`](../recursion/go.mod) 中的 `ExpanderCompilerCollection`、`gnark` 对齐，避免序列化格式不一致。

## 用法

```bash
# n×n 矩阵乘（全部 witness），域 m31，witness 会自动按 SIMD 复制 16 份（与 gkr 测试一致）
./expander-gen matmul --n 3 --field m31 \
  --out-circuit ./out/matmul_m31_circuit.txt \
  --out-witness ./out/matmul_m31_witness.txt

# Merkle inclusion：深度 depth（叶子数 2^depth），玩具压缩 H(a,b)=a²+b mod p
./expander-gen merkle --depth 4 --leaf-index 0 --field bn254 \
  --out-circuit ./out/merkle_bn254_circuit.txt \
  --out-witness ./out/merkle_bn254_witness.txt
```

标志说明：

- `--field`：`m31` | `bn254` | `gf2`（与 `recursion/modules/fields` 枚举一致）。M31 witness 复制 **16** 路，GF2 复制 **8** 路，BN254 **1** 路。
- `--skip-check`：跳过 `CheckCircuitMultiWitness`（大电路可略省时间；默认会检查）。
- `lanczos`、`pixels` 子命令：**未实现**，仅占位。

## 与 Expander 联调

在仓库根目录（Rust 已能编译的前提下）：

```bash
cargo run -p bin --bin expander-exec --release -- \
  -f SHA256 -p Raw prove \
  -c ./out/matmul_m31_circuit.txt \
  -w ./out/matmul_m31_witness.txt \
  -o ./out/proof.bin
```

具体可用的 `-p`（PCS）与 `-f`（FS 哈希）组合以 [`bin/src/exec.rs`](../bin/src/exec.rs) 为准。

## 已知限制

- **BabyBear / Goldilocks**：当前 `ecgo` 流水线未在本模块中暴露；需等编译器支持或自行扩展 `--field`。
- **Merkle 哈希**为代数玩具函数 **H(a,b)=a²+b**，仅用于基准与管线验证，**不具备密码学抗碰撞性**。
- **电路规模**：`n` 或 `depth` 过大时编译内存与时间急剧上升；大参数建议加 `--skip-check` 并在机器上留足内存。

## 子命令一览

| 子命令   | 说明 |
|----------|------|
| `matmul` | 参数 `--n`，矩阵乘法约束 |
| `merkle` | 参数 `--depth`、`--leaf-index`，Merkle 打开 |
| `lanczos` | 占位 |
| `pixels`  | 占位 |
