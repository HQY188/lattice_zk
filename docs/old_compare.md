# GKR Raw vs Lattice 性能对照报告（基于 `perf_gkr_compare.csv`）

## 测试设置
- 入口：`cargo test -p gkr --release gkr_correctness_raw|gkr_correctness_lattice`
- 电路：M31x16, 32 keccak instances per proof（与测试用例一致）
- 重复次数：`5`（RunIndex = 0..4）
- 解析字段：`Proving time` / `Verification time` / `Multi-core Verification time` / `Proof size`

## 结果概览
`ProofBytes` 在两种 PCS 下为常数（与 RunIndex 无关）：
- Lattice PCS：`440793 bytes`
- Raw PCS：`224844 bytes`

## 关键指标（us，越小越好）
| 指标 | Lattice PCS（avg / median） | Raw PCS（avg / median） | 相对变化（Lattice 相对 Raw） |
|---|---:|---:|---:|
| 证明时间 `ProvingUs` | `326132 / 327465` | `293727 / 272426` | avg：`+11.0%`；median：`+20.2%` |
| 验证时间 `VerifyUs` | `68000 / 66861` | `60494 / 59113` | avg：`+12.5%`；median：`+13.1%` |
| 多核验证 `ParVerifyUs` | `48482 / 49357` | `41054 / 36971` | avg：`+18.1%`；median：`+33.4%` |

## 观察与解读（简要）
1. **Lattice PCS 在本次基准下整体更慢**：不论是证明、单核验证还是多核验证，Lattice 均相对 Raw 有明显开销（尤其多核验证的中位数差距最大）。
2. **Raw 的证明时间存在一次明显抖动**：RunIndex=3 的 Raw `ProvingUs` 达到 `380284`，显著高于其他轮次；因此建议以 median 作为更稳健的对比依据。
3. **证明大小几乎翻倍**：Lattice PCS 的 proof bytes 约为 Raw 的 `1.96x`，这也可能间接影响后续的序列化/反序列化与验证侧内存访问开销。

## 建议后续（可选）
- 若目标是“绝对性能优先”，建议继续定位 Lattice PCS 的主要耗时模块（通常在承诺/打开/ML E 相关线性代数与 ring 运算上）。
- 若目标是“端到端吞吐”，建议把 `perf_gkr_compare.csv` 扩展到更少干扰的环境（固定 CPU 亲和/进程优先级）并增加重复次数，降低偶发抖动影响。
