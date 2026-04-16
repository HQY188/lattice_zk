//! RNS 多项式 `PolyRns`：在 **剩余数系（RNS）** 下表示环
//! \( R_q = \mathbb{Z}_q[X]/(X^N+1) \)，其中 \( q \) 由若干两两互素的奇素数通过 **CRT** 合成。
//!
//! # 数据布局
//! - 对每个 RNS **腿**（每个素模 \( q_k \)）存长度 `n` 的系数向量，系数恒在 `[0, q_k)`。
//! - `is_ntt` 标记当前各层是在 **系数域** 还是在 **否定循环 NTT 域**（与 `ntt.rs` 中的变换一致）。
//!
//! # 否定循环乘法
//! `mul_negacyclic` 实现 \( a \cdot b \bmod (X^N+1) \)：路径为 **NTT → 逐点乘 → 逆 NTT**，与朴素 \( O(N^2) \) 卷积在测试
//! `test_negacyclic_mul_matches_naive_goldilocks` 中对照。
//!
//! # 范数检查
//! **`norm_sq_first_modulus`**：仅把 **第一条 RNS 腿** 的系数做 **平衡提升**（选绝对值最小的整数代表），再算平方 \( \ell_2 \) 范数；
//! 用于打开/验证时对消息与随机性的原型范数估计（非完整 CRT 提升，属原型级检查）。

use serdes::{ExpSerde, SerdeResult};

use super::ntt::NttPlan;

/// CRT 使用的 **素模列表**。
///
/// - `moduli[k]` 是第 `k` 条 RNS 腿的素数 \( q_k \)；同一次运算中所有 `PolyRns` 必须共享相同的 `RnsModuli`。
/// - 构造时要求：非空、每个元素为 **大于 2 的奇数**（与 NTT/否定循环设置一致；具体 NTT 可解性由 `NttPlan` 侧保证）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RnsModuli {
    /// 各腿素模 \( q_0, q_1, \ldots \)，顺序固定，影响 CRT 与序列化语义。
    pub moduli: Vec<u64>,
}

impl RnsModuli {
    /// 构造 RNS 模数表；会 `assert` 非空且每个模为奇数 \( > 2 \)。
    pub fn new(moduli: Vec<u64>) -> Self {
        assert!(!moduli.is_empty(), "RNS moduli must be non-empty");
        for &q in &moduli {
            assert!(q > 2 && q % 2 == 1, "modulus must be an odd integer");
        }
        Self { moduli }
    }

    /// RNS **腿数**（CRT 层数），等于 `moduli.len()`。
    #[inline]
    pub fn level(&self) -> usize {
        self.moduli.len()
    }

    /// 第 `i` 条腿的素模 \( q_i \)。
    #[inline]
    pub fn modulus(&self, i: usize) -> u64 {
        self.moduli[i]
    }
}

impl ExpSerde for RnsModuli {
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> SerdeResult<()> {
        self.moduli.serialize_into(&mut writer)
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> SerdeResult<Self> {
        let moduli = Vec::<u64>::deserialize_from(&mut reader)?;
        Ok(Self::new(moduli))
    }
}

/// **RNS 多项式**：环 \( \prod_k \mathbb{Z}_{q_k}[X]/(X^N+1) \) 上的元素。
///
/// # 不变量（调用方与实现共同维护）
/// - `coeffs.len() == moduli.level()`，且对每个 `k`，`coeffs[k].len() == n`。
/// - 所有系数已 **模 \( q_k \) 归约** 到 `[0, q_k)`。
/// - `is_ntt == true` 时，每层 `coeffs[k]` 解释为该模数下 **NTT 域** 数据；为 `false` 时为 **标准系数**。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolyRns {
    /// 否定循环次数 \( N \)（即多项式项数为 \( N \)，模 \( X^N+1 \)）。
    pub n: usize,
    /// 与系数层一一对应的 CRT 素模表。
    pub moduli: RnsModuli,
    /// `coeffs[k][i]`：第 `k` 条腿、第 `i` 个系数（域由 `is_ntt` 解释）。
    pub coeffs: Vec<Vec<u64>>,
    /// 当前缓冲区是否处于 NTT 域（各层一致切换，见 `ntt_in_place` / `intt_in_place`）。
    pub is_ntt: bool,
}

impl PolyRns {
    /// 零多项式：所有系数为 0，**系数域**（`is_ntt == false`）。
    pub fn zero(n: usize, moduli: RnsModuli) -> Self {
        let level = moduli.level();
        let coeffs = vec![vec![0u64; n]; level];
        Self {
            n,
            moduli,
            coeffs,
            is_ntt: false,
        }
    }

    /// 调试断言：`n`、`moduli`、**NTT 标志**与 `other` 一致（混合域运算会逻辑错误）。
    pub fn assert_same_shape(&self, other: &Self) {
        assert_eq!(self.n, other.n);
        assert_eq!(self.moduli, other.moduli);
        assert_eq!(self.is_ntt, other.is_ntt);
    }

    /// RNS 腿数。
    #[inline]
    pub fn level(&self) -> usize {
        self.moduli.level()
    }

    /// 所有系数置 0；**不改变** `is_ntt`（仍为当前域下的“零向量”）。
    pub fn clear(&mut self) {
        for layer in &mut self.coeffs {
            layer.fill(0);
        }
    }

    /// **原地**加法：`self += rhs`，逐腿逐系数在 \( \mathbb{Z}_{q_k} \) 上相加。
    ///
    /// # 实现说明
    /// Goldilocks 类素数接近 \( 2^{64} \)，`a + b` 在 `u64` 上可能溢出；这里用 `overflowing_add` 与
    /// `wrapping_add` 把“和”正确归约到 `[0, q)`，避免 debug 下加法 panic。
    pub fn add_assign(&mut self, rhs: &Self) {
        self.assert_same_shape(rhs);
        for (k, layer) in self.coeffs.iter_mut().enumerate() {
            let q = self.moduli.modulus(k);
            for (a, &b) in layer.iter_mut().zip(rhs.coeffs[k].iter()) {
                // Goldilocks-style moduli can be close to 2^64, so a+b may overflow u64 in debug.
                let (sum, carry) = a.overflowing_add(b);
                let mut s = if carry {
                    // sum + (2^64 - q)
                    sum.wrapping_add(u64::MAX.wrapping_sub(q).wrapping_add(1))
                } else {
                    sum
                };
                if s >= q {
                    s -= q;
                }
                *a = s;
            }
        }
    }

    /// **原地**减法：`self -= rhs`，等价于 \( a \leftarrow a - b \pmod{q_k} \)。
    ///
    /// 使用 `wrapping_sub` 后在 `[0, q)` 上修正一次，得到标准非负代表。
    pub fn sub_assign(&mut self, rhs: &Self) {
        self.assert_same_shape(rhs);
        for (k, layer) in self.coeffs.iter_mut().enumerate() {
            let q = self.moduli.modulus(k);
            for (a, &b) in layer.iter_mut().zip(rhs.coeffs[k].iter()) {
                let s = a.wrapping_sub(b);
                *a = if s >= q { s.wrapping_add(q) } else { s };
            }
        }
    }

    /// **NTT 域**下的 **逐点乘法**：`self *= rhs`（Schönhage/NTT 卷积中的“频域相乘”步）。
    ///
    /// # 前置条件
    /// `self.is_ntt && rhs.is_ntt`，且形状一致；否则应先变换或会 `assert` 失败。
    pub fn mul_pointwise_assign(&mut self, rhs: &Self) {
        self.assert_same_shape(rhs);
        assert!(self.is_ntt, "pointwise multiply requires NTT domain");
        for (k, layer) in self.coeffs.iter_mut().enumerate() {
            let q = self.moduli.modulus(k);
            for (a, &b) in layer.iter_mut().zip(rhs.coeffs[k].iter()) {
                *a = mul_mod_u64(*a, b, q);
            }
        }
    }

    /// **原地**正向 NTT：系数域 → NTT 域。若已在 NTT 域则 **幂等**（直接返回）。
    ///
    /// `plan` 必须对 `(n, moduli)` 构建，且每层 `plan.table[k].fwd` 实现该模数下的否定循环 NTT。
    pub fn ntt_in_place(&mut self, plan: &NttPlan) {
        assert_eq!(plan.n, self.n);
        assert_eq!(plan.moduli, self.moduli);
        if self.is_ntt {
            return;
        }
        for (k, layer) in self.coeffs.iter_mut().enumerate() {
            plan.table[k].fwd(layer);
        }
        self.is_ntt = true;
    }

    /// **原地**逆 NTT：NTT 域 → 系数域。若已在系数域则 **幂等**。
    pub fn intt_in_place(&mut self, plan: &NttPlan) {
        assert_eq!(plan.n, self.n);
        assert_eq!(plan.moduli, self.moduli);
        if !self.is_ntt {
            return;
        }
        for (k, layer) in self.coeffs.iter_mut().enumerate() {
            plan.table[k].inv(layer);
        }
        self.is_ntt = false;
    }

    /// **否定循环乘法**：计算 \( a \cdot b \) 在 \( \mathbb{Z}_q[X]/(X^N+1) \) 中。
    ///
    /// 若操作数在系数域，会先对副本做 NTT，再 `mul_pointwise_assign`，再逆变换；**返回值在系数域**
    ///（`is_ntt == false`）。输入可为任意域，但须与 `plan` 匹配。
    pub fn mul_negacyclic(&self, rhs: &Self, plan: &NttPlan) -> Self {
        self.assert_same_shape(rhs);
        assert_eq!(plan.n, self.n);
        assert_eq!(plan.moduli, self.moduli);

        let mut a = self.clone();
        let mut b = rhs.clone();
        a.ntt_in_place(plan);
        b.ntt_in_place(plan);
        a.mul_pointwise_assign(&b);
        a.intt_in_place(plan);
        a
    }

    /// **仅第一条 RNS 腿**上，系数的 **平衡提升** 后平方 \( \ell_2 \) 范数 \( \sum_i v_i^2 \)（`f64`）。
    ///
    /// 对每个系数 \( c \in [0, q) \)，取代表 \( v \in \mathbb{Z} \) 满足 \( v \equiv c \pmod q \) 且 \( |v| \le q/2 \)，
    /// 再累加 \( v^2 \)。**不**做完整 CRT 提升到单一大整数环；用于快速原型界估计。
    pub fn norm_sq_first_modulus(&self) -> f64 {
        let q = self.moduli.modulus(0);
        let half = q / 2;
        let mut acc: f64 = 0.0;
        for &c in &self.coeffs[0] {
            let v: i64 = if c >= half {
                (c as i128 - q as i128) as i64
            } else {
                c as i64
            };
            acc += (v as f64) * (v as f64);
        }
        acc
    }
}

impl ExpSerde for PolyRns {
    /// 线格式顺序：`n` → `moduli` → `is_ntt` → `coeffs`（与 `deserialize_from` 对称）。
    ///
    /// 反序列化后 **`is_ntt` 按字节还原**：若持久化时处于 NTT 域，读回后仍为 NTT 域，调用方需自知。
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> SerdeResult<()> {
        self.n.serialize_into(&mut writer)?;
        self.moduli.serialize_into(&mut writer)?;
        self.is_ntt.serialize_into(&mut writer)?;
        self.coeffs.serialize_into(&mut writer)?;
        Ok(())
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> SerdeResult<Self> {
        let n = usize::deserialize_from(&mut reader)?;
        let moduli = RnsModuli::deserialize_from(&mut reader)?;
        let is_ntt = bool::deserialize_from(&mut reader)?;
        let coeffs = Vec::<Vec<u64>>::deserialize_from(&mut reader)?;
        Ok(Self {
            n,
            moduli,
            coeffs,
            is_ntt,
        })
    }
}

/// \( (a \cdot b) \bmod q \)，用 `u128` 乘积避免 `u64` 溢出；对本库选用的素数与 \( N \) 足够。
#[inline]
fn mul_mod_u64(a: u64, b: u64, q: u64) -> u64 {
    // For our chosen primes and N, u128 multiplication is fine.
    ((a as u128 * b as u128) % (q as u128)) as u64
}

#[cfg(test)]
mod tests {
    use arith::FFTField;
    use super::*;
    use crate::ring::NttPlan;
    use rand::{rngs::StdRng, RngCore, SeedableRng};

    /// 二进制快速幂：\( a^e \bmod q \)。
    fn pow_mod(mut a: u64, mut e: u64, q: u64) -> u64 {
        let mut r = 1u64;
        while e > 0 {
            if e & 1 == 1 {
                r = ((r as u128 * a as u128) % (q as u128)) as u64;
            }
            a = ((a as u128 * a as u128) % (q as u128)) as u64;
            e >>= 1;
        }
        r
    }

    /// 为长度 `n` 的否定循环 NTT 生成 **单位根相关参数** \( \psi \)（Goldilocks 域上）。
    ///
    /// Goldilocks 模数 \( 2^{64}-2^{32}+1 \) 的 2-adicity 为 32；由本原 \(2^{32} \) 次根缩放得 \( 2n \) 次本原根方向。
    fn goldilocks_psi(n: usize) -> u64 {
        // Goldilocks modulus: 2^64 - 2^32 + 1, two-adicity 32.
        let q = goldilocks::GOLDILOCKS_MOD;
        let root_2_32 = goldilocks::Goldilocks::root_of_unity().v;
        let k_plus_1 = (2 * n).ilog2() as u64;
        assert!(k_plus_1 <= 32);
        let shift = 32 - k_plus_1;
        pow_mod(root_2_32, 1u64 << shift, q)
    }

    /// 朴素 \( O(N^2) \) 否定循环卷积，用于与 NTT 路径对比正确性。
    ///
    /// 规则：索引 \( i+j \ge N \) 时贡献到 \( X^{i+j-N} \) 且带负号（因 \( X^N = -1 \)）。
    fn naive_negacyclic(a: &[u64], b: &[u64], q: u64) -> Vec<u64> {
        let n = a.len();
        let mut out = vec![0u64; n];
        for i in 0..n {
            for j in 0..n {
                let mut prod = ((a[i] as u128 * b[j] as u128) % (q as u128)) as u64;
                let k = i + j;
                if k >= n {
                    // x^n = -1
                    prod = if prod == 0 { 0 } else { q - prod };
                    let idx = k - n;
                    out[idx] = ((out[idx] as u128 + prod as u128) % (q as u128)) as u64;
                } else {
                    out[k] = ((out[k] as u128 + prod as u128) % (q as u128)) as u64;
                }
            }
        }
        out
    }

    /// 单腿 Goldilocks 上，`mul_negacyclic` 与朴素卷积结果一致，且输出在系数域。
    #[test]
    fn test_negacyclic_mul_matches_naive_goldilocks() {
        let n = 256usize;
        let q = goldilocks::GOLDILOCKS_MOD;
        let moduli = RnsModuli::new(vec![q]);
        let psi = goldilocks_psi(n);
        let plan = NttPlan::new(n, moduli.clone(), vec![psi]);

        let mut rng = StdRng::seed_from_u64(123);
        let mut a = PolyRns::zero(n, moduli.clone());
        let mut b = PolyRns::zero(n, moduli.clone());
        for i in 0..n {
            a.coeffs[0][i] = rng.next_u64() % q;
            b.coeffs[0][i] = rng.next_u64() % q;
        }

        let c = a.mul_negacyclic(&b, &plan);
        let naive = naive_negacyclic(&a.coeffs[0], &b.coeffs[0], q);
        assert_eq!(c.coeffs[0], naive);
        assert!(!c.is_ntt);
    }
}
