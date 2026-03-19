# Lattice Poly Commit 性能瓶颈与并行化分析

基于 [mle_pc.md](../mle_pc.md) 协议与 `lattice_poly_commit` 实现，分析主要性能瓶颈及便于并行加速的矩阵/向量运算。

---

## 1. 协议结构回顾

- **单变元承诺**：$\mathsf{PC.Com}$ 计算 $\vec{\mathfrak{h}}_i = \mathbf{A}_0 \vec{\bm{h}}_i + \mathbf{A}_1 \vec{\bm{\eta}}_i \pmod{q}$（矩阵×向量）；$\mathsf{PC.Verify}$ 侧有 $\mathbf{A}_0 \vec{\bm{e}} + \mathbf{A}_1 \vec{\bm{\varepsilon}} = \sum \mathsf{R.Ecd}(x^{ni})\,\vec{\mathfrak{h}}_i$（线性组合 + 一次矩阵×向量校验）。
- **MLE 层**：将 $f$ 排成矩阵 $T \in \mathbb{Z}_p^{2^{l/\iota} \times 2^{l - l/\iota}}$，对每行做单变元承诺得 $C = (c_1,\ldots,c_{2^{l/\iota}})$；Eval 中算 $\vec{u} = \vec{A} \cdot T$（向量×矩阵），再 $y = \langle \vec{u}, \vec{B} \rangle$，以及 $c_u = \sum_i A[i] \cdot c_i$、$c_z = c_a + e \cdot c_u$ 等。

当前仓库中单变元部分为 **stub**（无真实格运算），瓶颈与可并行处主要来自 MLE 层的矩阵/向量运算与“每行一次”的批量单变元调用；若接真实格实现，则单变元内的矩阵乘法会成为另一大瓶颈。

---

## 2. 主要性能瓶颈（按协议与实现）

### 2.1 MLE 层

| 步骤 | 协议/实现位置 | 运算量级 | 说明 |
|------|----------------|----------|------|
| **构建矩阵 T** | `build_matrix_t` | $O(2^l)$ 索引与访存 | 将超立方体 $f$ 按 Lemma 1 重排为 $T_{ij}$，无算术，主要是内存布局。 |
| **行承诺** | PC.Com：对每行 $T^{(i)}$ 调用单变元 Com | $2^{l/\iota}$ 次单变元 Com，每次 $O(2^{l - l/\iota})$ 系数 | 当前 stub 下为拷贝；真实格下为每行两次矩阵×向量 $\mathbf{A}_0 \vec{\bm{h}}_i + \mathbf{A}_1 \vec{\bm{\eta}}_i$，**行与行之间无依赖，天然可并行**。 |
| **$\vec{u} = \vec{A} \cdot T$** | PC.Eval / `compute_u` | $O(2^l)$ 域乘加 | $u[j] = \sum_i A[i]\,T[i][j]$，**最显式的“矩阵乘法”形态**，是 Eval 中证明方的主要计算量。 |
| **$y = \langle \vec{u}, \vec{B} \rangle$** | `dot_product(&u, &b)` | $O(2^{l - l/\iota})$ | 内积，量级小于 $u = A\cdot T$。 |
| **$c_u = \sum_i A[i]\,c_i$** | `linear_combine_commitments` | $O(2^l)$（每系数一次线性组合） | 对 $2^{l/\iota}$ 个承诺做系数维度的线性组合，与“向量×矩阵”同阶，**按输出系数可并行**。 |
| **Verify 侧** | 重算 $\vec{A},\vec{B}$、$c_u$、$\langle \vec{z},\vec{B} \rangle$、单变元 Open | $O(2^l)$ + 单变元 Open | 验证侧同样以 $c_u$ 的线性组合和点积为主；单变元 Open 在真实格下为解码/范数检查等。 |

小结：在 MLE 层，**证明与验证的主要瓶颈**是：

1. **$\vec{u} = \vec{A} \cdot T$**（向量×矩阵，$O(2^l)$）；
2. **$c_u = \sum_i A[i]\,c_i$**（多承诺的系数线性组合，$O(2^l)$）；
3. 若启用真实格单变元：**每行的 $\mathbf{A}_0 \vec{\bm{h}}_i + \mathbf{A}_1 \vec{\bm{\eta}}_i$**（$2^{l/\iota}$ 次矩阵×向量）。

### 2.2 单变元层（真实格实例化时）

根据 mle_pc.md：

- **Com**：对 $0 \leq i \leq m+1$ 计算 $\vec{\mathfrak{h}}_i = \mathbf{A}_0 \vec{\bm{h}}_i + \mathbf{A}_1 \vec{\bm{\eta}}_i$，即 **$R_q$ 上的矩阵×向量**（$\mathbf{A}_0 \in R_q^{\mu \times \ell}$，$\mathbf{A}_1 \in R_q^{\mu \times (\mu+\nu)}$）。
- **Verify**：校验 $\mathbf{A}_0 \vec{\bm{e}} + \mathbf{A}_1 \vec{\bm{\varepsilon}} = \text{RHS}$，同样为 **矩阵×向量**。

因此，一旦接上真实格实现，**单变元内的矩阵×向量**（环 $R_q$ 上）会成为 Com/Verify 的主要成本；行与行、或不同 $i$ 之间无数据依赖，可并行。

---

## 3. 便于并行化的矩阵/向量运算

以下均与 mle_pc.md 及当前代码一一对应，且易于做多线程/ SIMD / GPU。

### 3.1 $\vec{u} = \vec{A} \cdot T$（向量×矩阵）

- **协议**：PC.Eval 中 $u[j] = \sum_{i} A[i]\,T[i][j]$，$j \in [2^{l - l/\iota}]$。
- **实现**：`multilinear::compute_u(a, t)`，双重循环累加。
- **并行策略**：
  - **按列 $j$ 并行**：每个 $j$ 对应一个独立的 $u[j]$，计算时只需读 $\vec{A}$ 和 $T[\cdot][j]$，无写冲突。  
    可对 $j$ 做区间划分，每线程/每核算一段 $u[j]$，**无锁、负载均衡简单**。
  - 也可按行 $i$ 做“部分和”，再对 $j$ 做归约，但不如按列并行直接。
- **数据规模**：$l=14,\,\iota=2$ 时，$T$ 为 $2^7 \times 2^7$，$2^l = 2^{14}$ 次乘加；$l$ 增大时该步会主导 Eval 时间，**优先并行此步收益最大**。

### 3.2 $c_u = \sum_i A[i] \cdot c_i$（承诺的线性组合）

- **协议**：PC.Eval / Verify 中 $c_u = \sum_{i=1}^{2^{l/\iota}} \mathsf{R.Ecd}(A[i])\cdot c_i$；当前 stub 下为系数域上的线性组合。
- **实现**：`linear_combine_commitments(&c.row_commitments, &a)`，对每个系数下标做标量乘加。
- **并行策略**：
  - **按“系数下标”并行**：输出 $c_u$ 的每一维只依赖各 $c_i$ 的同一维和 $A[i]$，不同维之间完全独立。  
    可把输出向量按下标分片，每线程负责一段系数，**无写冲突、易并行**。
  - 若真实格中 $c_i$ 为环元素/向量，同样按输出向量的分量或环系数的下标并行即可。

### 3.3 行承诺：对每行 $T^{(i)}$ 的单变元 Com

- **协议**：PC.Com 中对 $i \in [2^{l/\iota}]$ 依次算 $(c_i,\delta_i) \gets \mathsf{PC.Com}(\mathsf{ck}_{\mathrm{uni}}, T^{(i)}(X))$。
- **实现**：`commit` 中 `for row in &t { uni_commit(&ck.ck_uni, row); }`。
- **并行策略**：
  - **按行 $i$ 并行**：不同行之间无依赖，可对 $i$ 做并行 for，每线程处理若干行，**天然适合多核/多机**。
  - 真实格下，每行内部是 $\mathbf{A}_0 \vec{\bm{h}}_i + \mathbf{A}_1 \vec{\bm{\eta}}_i$；若进一步在单变元内部对矩阵×向量的**输出分量**并行，可形成“行间 + 行内”两层并行。

### 3.4 单变元内矩阵×向量（真实格）

- **协议**：$\vec{\mathfrak{h}} = \mathbf{A}_0 \vec{\bm{h}} + \mathbf{A}_1 \vec{\bm{\eta}}$，均在 $R_q$ 上。
- **并行策略**：
  - **按结果向量分量并行**：$\vec{\mathfrak{h}}$ 的每个分量由 $\mathbf{A}_0$ 的一行、$\mathbf{A}_1$ 的一行与对应向量点积得到，分量之间无依赖，可对输出分量分片并行。
  - 若 $R_q$ 为多项式环，每个分量可能是多项式乘法/卷积，可再在分量内用 NTT/FFT 或已有多项式乘法的并行实现。

### 3.5 其他

- **$\langle \vec{z}, \vec{B} \rangle$、$\langle \vec{u}, \vec{B} \rangle$**：点积可先按块做局部点积再归约（map-reduce），便于多线程/SIMD。
- **build_matrix_t**：按输出下标 $(i,j)$ 并行填 $T[i][j] = f(\vec{b}^{(i,j)})$，仅依赖只读的 $f$ 与索引，无写冲突。

---

## 4. 小结表（瓶颈 vs 并行）

| 运算 | 协议/代码位置 | 量级 | 主要瓶颈？ | 推荐并行方式 |
|------|----------------|------|------------|--------------|
| $\vec{u} = \vec{A} \cdot T$ | PC.Eval, `compute_u` | $O(2^l)$ | ✅ 是（Eval 证明方） | **按列 $j$ 并行**算 $u[j]$ |
| $c_u = \sum_i A[i]\,c_i$ | Eval/Verify, `linear_combine_commitments` | $O(2^l)$ | ✅ 是 | **按输出系数下标**分片并行 |
| 行承诺（每行单变元 Com） | PC.Com, `commit` 中循环 | $2^{l/\iota}$ 次 Com | ✅ 是（Com 阶段） | **按行 $i$ 并行** |
| 单变元内 $\mathbf{A}\vec{x}$ | mle_pc.md 单变元 Com/Verify | 每行一次 | 真实格下是 | **按结果分量**并行 |
| build_matrix_t | 构建 $T$ | $O(2^l)$ 访存 | 次要 | 按 $(i,j)$ 并行填表 |
| $\langle \cdot,\cdot \rangle$ | 多处点积 | $O(2^{l-l/\iota})$ | 次要 | 分块局部点积 + 归约 |

**结论**：与“矩阵乘法”最直接对应、且最便于并行的是 **$\vec{u} = \vec{A} \cdot T$**（按列并行）；其次为 **$c_u$ 的线性组合**（按系数下标并行）和 **行承诺**（按行并行）。真实格单变元接入后，**单变元内的矩阵×向量**按输出分量并行即可与 MLE 层并行策略组合使用。
