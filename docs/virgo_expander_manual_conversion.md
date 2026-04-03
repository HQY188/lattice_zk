# Expander `data/` 电路 ↔ Virgo `zk_proof`：手动转换说明

目标：把 **Expander** 侧用于测试的电路（如 `data/circuit_m31.txt`、`circuit_babybear.txt`）变成 Virgo 能读的 **`circuit` 文本 + `meta` 文本**，以便你本地用 `zk_proof` 直接跑（环境变量见 `scripts/run_virgo.ps1`，此处不展开）。

---

## 1. Expander 侧：你手里的文件是什么？

- **电路文件**（如 `circuit_m31.txt`）在仓库里是 **`RecursiveCircuit` 的二进制序列化**，通过 `serdes::ExpSerde` 读写，**不是** Virgo 那种可读的 ASCII。
- 加载入口：`circuit::RecursiveCircuit::load(path)` → `fs::read` + `deserialize_from`（见 `circuit/src/ecc_circuit.rs`）。
- 内部结构：若干 **`Segment`**（`gate_muls` / `gate_adds` / `gate_consts` / `gate_uni` + 子段连接），再按 `layers` 做 `flatten()` 得到 GKR 用的 `Circuit`（见同文件 `flatten`）。
- **见证文件**（`witness_*.txt`）同样是 **`Witness` 的二进制序列化**（`Circuit::load_witness_bytes` → `Witness::deserialize_from`）。

结论：**不能**用文本编辑器「照着改」成 Virgo 格式；至少需要 **用 Expander 代码把二进制反序列化出来**，再谈映射。

---

## 2. Virgo 侧：`zk_proof` 要读什么？

解析代码在 **`Virgo/src/linear_gkr/zk_verifier.cpp`** 的 `zk_verifier::read_circuit(path, meta_path)`。

### 2.1 电路文本（`circuit_in`）

1. 第一个整数：`d`（与层编号相关；内部有 `layer[0..d]`，共 `d+1` 层）。
2. 对 `i = 1 .. d` 每一层：
   - 读一个整数 `n`：该层 gate 数（随后读 `n` 组 gate）。
   - 对 `j = 0 .. n-1`：读 `ty g u v`（`ty`、`g` 为 `int`，`u`、`v` 为 `long long`）。
   - Gate 类型含义见 **`Virgo/src/linear_gkr/README.md`**（加/乘/输入/中继等，与 Expander 的 `GateMul`/`GateAdd` 等 **不是同一套枚举**）。

第一层有特殊处理（输入层与下一层 `relay` 等），并有 **padding** 逻辑（`pad_requirement` 等），与 Expander 的 flatten 结果 **不会天然一致**。

### 2.2 Meta 文本（`meta_in`）

对 `i = 1 .. d` 每一层一行，共 **d 行**，每行 5 个整数：

```text
is_para  block_size  repeat_num  log_block_size  log_repeat_num
```

（见 `fscanf(meta_in, "%d", &is_para)` 与 `fscanf(meta_in, "%d%d%d%d", ...)`。）

并行层要求：`is_para` 为真时，`1 << log_repeat_num == repeat_num`。

### 2.3 第三个参数

`main_zk.cpp` 中 `verify(argv[3])` 是 **输出/日志路径**（见 `zk_verifier::verify` 使用），不是 Expander 的 witness 文件路径。

---

## 3. 为什么不能「手工逐字节改」成 Virgo？

1. **IR 不同**：Expander 是 **递归分段 + flatten 后的 mul/add/const/uni**；Virgo 是 **按层扁平 gate + 固定 wire 索引** + 自己的 gate 类型编号。
2. **域不同**：Virgo 证明系统使用 **`2^61-1` 上的 Mersenne 域实现**（`Virgo/src/linear_gkr/prime_field.cpp`）；Expander 的 `m31` / `babybear` 是 **不同域与 SIMD 打包**。  
   把「同一 Keccak 实例」迁过去，**不是简单换文件格式**，而是 **重新在一个不同的算术系统里表达电路**（或重新编译/生成电路）。
3. **Witness**：Virgo 的 `zk_proof` 路径里，证明/验证由 `zk_prover` 内部 `evaluate()` 等驱动；**不是** Expander 的 `Witness` 二进制直接喂给第三个参数。

因此：**「手动转换」**在工程上指两件事之一：

- **A. 手工操作流水线**：用脚本/工具生成 Virgo 文件，而不是手改 `circuit_m31.txt` 的十六进制。  
- **B. 手工指定对照关系**：你接受 **用 Virgo 自带生成器**（如 `Virgo/tests/SHA256`）产出的电路作为 **Virgo 侧基准**，与 Expander 侧 **同一套数据文件字节** 不对齐，只对齐「同一类 benchmark（哈希）」。

---

## 4. 建议的「手动」步骤（按现实可行度排序）

### 步骤 1：确认 Expander 文件是二进制

在 PowerShell 中：

```powershell
Format-Hex -Path .\data\circuit_m31.txt -Count 64
```

若首字节不是可打印 ASCII 数字，即 **不是** Virgo 那种以 `d` 的十进制文本开头。

### 步骤 2（推荐对照）：在 Virgo 内生成**可读**的 circuit + meta

在 `Virgo/tests/SHA256` 按仓库说明执行 `build.py` / `build.sh`（需 Python、C++ 编译器、`parser_sha_data_parallel` 等），生成例如：

- `SHA256_64_merkle_1_circuit.txt`
- `SHA256_64_merkle_1_meta.txt`

用文本编辑器打开，对照 **`Virgo/src/linear_gkr/README.md`** 与 `read_circuit` 的读入顺序，理解 **meta 每行 5 个数**、**每层 `n` 行 gate** 的含义。

这是你**唯一能「完全手工核对」**的 Virgo 格式样本（**不是** Expander 那份 Keccak 电路）。

### 步骤 3（若必须坚持 Expander → Virgo 同一套电路语义）

需要**单独实现**一个转换器（建议单独 crate / 工具，**不要**塞进 `run_virgo.ps1`）：

1. 用 Expander 类型加载：`RecursiveCircuit::<Cfg>::load("data/circuit_m31.txt")` → `flatten()` → 遍历每层 `CircuitLayer` 的 `mul`/`add`/…  
2. 定义 **Expander gate → Virgo `ty` + `(u,v)`** 的映射规则；  
3. 按 Virgo 要求排序 gate 下标 `g`、补齐 padding；  
4. 为每层生成 **meta**（`is_para` / block / repeat 等），需与 Virgo 并行 GKR 假设一致。

这是 **研发级工作**，无法靠本文档「手工完成」而不写代码。

### 步骤 4：转换完成后跑 Virgo

设置环境变量（示例）：

```powershell
$env:VIRGO_M31_CIRCUIT = "D:\path\to\virgo_m31_circuit.txt"
$env:VIRGO_M31_META    = "D:\path\to\virgo_m31_meta.txt"
$env:VIRGO_M31_LOG     = "D:\CS\ZK\work\Expander\results\virgo_m31.log"
# babybear 同理 VIRGO_BABYBEAR_*
powershell -ExecutionPolicy Bypass -File scripts\run_virgo.ps1
```

---

## 5. 小结

| 项目 | Expander `data/circuit_*.txt` | Virgo `circuit` + `meta` |
|------|-------------------------------|---------------------------|
| 编码 | 二进制 `ExpSerde` | 明文 `fscanf` |
| 语义 | 递归电路 + flatten 后 GKR 层 | 线性 GKR 线性门列表 + 并行 meta |
| 域 | M31 / BabyBear 等 | 固定 `2^61-1` 路径（Virgo 实现） |

**手动能做的**：搞清 Virgo 样本格式、在 Virgo 侧生成对照样本；**不能做的**：把 Expander 二进制当文本改写成 Virgo。

**下一步若要自动化**：单独实现「Expander `Circuit` → Virgo 文本」的转换程序，再交给 `run_virgo.ps1` 用环境变量调用。
