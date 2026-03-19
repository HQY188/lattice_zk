# GKR + Lattice PCS 性能报告

本文档基于对照实验：在同一电路与环境下对比 **Raw PCS**（透明/无密码学承诺）与 **Lattice PCS**（lattice_poly_commit）下 GKR 的证明时间、验证时间与 Proof 大小，用于评估 **lattice_poly_commit 对 GKR 整体效率的影响**。

---

## 1. 测试配置

| 项目 | 说明 |
|------|------|
| **电路** | Keccak，M31x16 域，32 个 Keccak 实例/证明 |
| **GKR 方案** | Vanilla GKR，Fiat–Shamir 哈希 SHA256 |
| **对照 PCS** | Raw（仅传递求值） vs Lattice（MLE 承诺 + 打开/验证） |
| **运行方式** | 单进程（无 MPI 多机），release/bench 优化 |
| **数据来源** | `scripts/bench_gkr_pcs.ps1` 单次 test 解析 + Criterion 证明 bench（100 样本，约 30s 测量） |

---

## 2. 单次运行结果（test 输出解析）

下表为一次完整运行中，由 `gkr_correctness_raw` 与 `gkr_correctness_lattice` 输出解析得到的指标。

| PCS | Proving (μs) | Proof (bytes) | Verify (μs) | ParVerify (μs) |
|-----|--------------|---------------|-------------|----------------|
| **Raw** | 277 207 | 224 844 | 38 567 | 38 567 |
| **Lattice** | 265 349 | 363 116 | 48 974 | 48 974 |

- **Proving**：单次证明耗时（微秒）。
- **Proof**：证明序列化后的字节数。
- **Verify / ParVerify**：单线程与多核验证耗时（微秒）。

---

## 3. Criterion 证明耗时（多轮统计）

证明阶段各跑 100 次采样得到的耗时统计（中位数与置信区间）：

| PCS | time (ms) | 说明 |
|-----|-----------|------|
| **Raw** | 265.56 — **268.50** — 272.18 | 中位数 268.50 ms |
| **Lattice** | 270.50 — **272.11** — 273.78 | 中位数 272.11 ms |

Lattice 相对 Raw 证明耗时约 **+1.3%**（272.11 / 268.50），处于正常波动范围内。

---

## 4. 效率影响分析

### 4.1 证明时间

- 单次运行：Raw 277 ms vs Lattice 265 ms（存在运行波动）。
- Criterion 中位数：Raw 268.5 ms vs Lattice 272.1 ms，**Lattice 约慢 1–2%**。
- **结论**：lattice_poly_commit 在证明端带来的额外开销很小，对 GKR 证明时间影响可忽略。

### 4.2 验证时间

- Raw 验证：38.6 ms（单线程/多核在此配置下相近）。
- Lattice 验证：49.0 ms，约 **+27%**。
- **原因**：Lattice 验证需执行 MLE 的 verify（含单变元承诺的打开验证），计算量大于 Raw 的直接求值比对。
- **结论**：Lattice 主要增加验证端开销，幅度约三成。

### 4.3 Proof 体积

- Raw：224 844 bytes（约 219 KB）。
- Lattice：363 116 bytes（约 355 KB），约为 Raw 的 **1.61×**。
- **原因**：Lattice 证明中除 GKR 本身外，还包含 MLE 的承诺与 opening（如 π₁、π₂ 等），体积增大符合预期。
- **结论**：Lattice 使 Proof 体积约增加 60%。

---

## 5. 汇总表（相对 Raw 的比值）

| 指标 | Raw | Lattice | Lattice / Raw |
|------|-----|---------|----------------|
| 证明 (Criterion 中位数) | 268.5 ms | 272.1 ms | **≈1.01** |
| 验证 (单次) | 38.6 ms | 49.0 ms | **≈1.27** |
| Proof 大小 | 225 KB | 363 KB | **≈1.61** |

---

## 6. 结论

1. **证明**：在 M31x16、32 Keccak/证明的配置下，采用 Lattice PCS 的 GKR 证明时间与 Raw 基本一致，**lattice_poly_commit 对证明效率影响可忽略**。
2. **验证**：Lattice 验证时间约为 Raw 的 1.27 倍，**验证端有约 27% 的额外开销**，主要来自 MLE 的验证逻辑。
3. **体积**：Lattice Proof 约为 Raw 的 1.6 倍，**体积增加约 60%**，用于承载基于格的 MLE 承诺与打开信息。

整体上，**lattice_poly_commit 在保持证明时间几乎不变的前提下，带来了可接受的验证与体积开销**，适合需要多项式承诺安全性或可组合性的场景。

---

## 7. 复现方式

在仓库根目录执行：

```powershell
.\scripts\bench_gkr_pcs.ps1 -OutputCsv bench_gkr_pcs.csv
```

详见 [TESTING.md §4.7 完整性能测试](TESTING.md#47-完整性能测试lattice-pcs-对-gkr-效率的影响)。
