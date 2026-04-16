//! 否定循环 NTT（长度 `n` 为 2 的幂）：实现 \( \mathbb{Z}_q[X]/(X^n+1) \) 上的快速乘法。
//!
//! `psi` 为模 `q` 下的本原 `2n` 次单位根（故 `psi^n = -1`）。`fwd`/`inv` 含 twist/untwist 与 bit-reverse 蝶形运算。
//! `NttPlan` 对 RNS 的每一腿各持有一张 `NttTable`。

use crate::ring::RnsModuli;

/// 单个模数 `q`、环维度 `n` 上的否定循环 NTT 预计算表。
#[derive(Clone, Debug)]
pub struct NttTable {
    pub n: usize,
    pub q: u64,
    /// psi is a primitive 2n-th root of unity mod q (so psi^n = -1).
    pub psi: u64,
    pub psi_inv: u64,
    /// omega = psi^2 is a primitive n-th root of unity.
    pub omega: u64,
    pub omega_inv: u64,
    pub n_inv: u64,
    pub psi_pows: Vec<u64>,
    pub psi_inv_pows: Vec<u64>,
    pub twiddles: Vec<u64>,
    pub inv_twiddles: Vec<u64>,
}

impl NttTable {
    /// Build a negacyclic NTT table given modulus q and a primitive 2n-th root `psi`.
    pub fn new(n: usize, q: u64, psi: u64) -> Self {
        assert!(n.is_power_of_two());
        // psi must satisfy psi^(2n)=1 and psi^n = -1.
        let psi_inv = mod_inv(psi, q).expect("psi must be invertible");
        let omega = mul_mod(psi, psi, q);
        let omega_inv = mod_inv(omega, q).expect("omega must be invertible");
        let n_inv = mod_inv(n as u64 % q, q).expect("n must be invertible");

        let log_n = n.ilog2() as usize;
        let _ = log_n;

        let mut psi_pows = Vec::with_capacity(n);
        let mut p = 1u64;
        for _ in 0..n {
            psi_pows.push(p);
            p = mul_mod(p, psi, q);
        }
        let mut psi_inv_pows = Vec::with_capacity(n);
        let mut pi = 1u64;
        for _ in 0..n {
            psi_inv_pows.push(pi);
            pi = mul_mod(pi, psi_inv, q);
        }

        let mut twiddles = Vec::with_capacity(n);
        let mut w = 1u64;
        // For radix-2 iterative NTT we precompute powers of omega for each stage.
        // We store all powers for maximal stage and index into them.
        for _ in 0..n {
            twiddles.push(w);
            w = mul_mod(w, omega, q);
        }

        let mut inv_twiddles = Vec::with_capacity(n);
        let mut wi = 1u64;
        for _ in 0..n {
            inv_twiddles.push(wi);
            wi = mul_mod(wi, omega_inv, q);
        }

        Self {
            n,
            q,
            psi,
            psi_inv,
            omega,
            omega_inv,
            n_inv,
            psi_pows,
            psi_inv_pows,
            twiddles,
            inv_twiddles,
        }
    }

    pub fn fwd(&self, a: &mut [u64]) {
        assert_eq!(a.len(), self.n);
        // Negacyclic twist: a[i] *= psi^i
        for (i, x) in a.iter_mut().enumerate() {
            *x = mul_mod(*x, self.psi_pows[i], self.q);
        }
        bit_reverse_in_place(a);
        let mut len = 2;
        while len <= self.n {
            let half = len / 2;
            let step = self.n / len;
            for start in (0..self.n).step_by(len) {
                for j in 0..half {
                    let w = self.twiddles[j * step];
                    let u = a[start + j];
                    let v = mul_mod(a[start + j + half], w, self.q);
                    let (x, y) = add_sub_mod(u, v, self.q);
                    a[start + j] = x;
                    a[start + j + half] = y;
                }
            }
            len *= 2;
        }
    }

    pub fn inv(&self, a: &mut [u64]) {
        assert_eq!(a.len(), self.n);
        bit_reverse_in_place(a);
        let mut len = 2;
        while len <= self.n {
            let half = len / 2;
            let step = self.n / len;
            for start in (0..self.n).step_by(len) {
                for j in 0..half {
                    let w = self.inv_twiddles[j * step];
                    let u = a[start + j];
                    let v = mul_mod(a[start + j + half], w, self.q);
                    let (x, y) = add_sub_mod(u, v, self.q);
                    a[start + j] = x;
                    a[start + j + half] = y;
                }
            }
            len *= 2;
        }
        // Scale by n^{-1}
        for x in a.iter_mut() {
            *x = mul_mod(*x, self.n_inv, self.q);
        }
        // Untwist: a[i] *= psi^{-i}
        for (i, x) in a.iter_mut().enumerate() {
            *x = mul_mod(*x, self.psi_inv_pows[i], self.q);
        }
    }
}

/// NTT plan for an RNS polynomial (one table per modulus).
#[derive(Clone, Debug)]
pub struct NttPlan {
    pub n: usize,
    pub moduli: RnsModuli,
    pub table: Vec<NttTable>,
}

impl NttPlan {
    /// Build a plan for the given moduli. `omegas[i]` must be a primitive 2n-th root mod moduli[i].
    pub fn new(n: usize, moduli: RnsModuli, omegas: Vec<u64>) -> Self {
        assert_eq!(moduli.level(), omegas.len());
        let table = moduli
            .moduli
            .iter()
            .zip(omegas.iter())
            .map(|(&q, &psi)| NttTable::new(n, q, psi))
            .collect();
        Self { n, moduli, table }
    }
}

#[inline]
fn add_sub_mod(u: u64, v: u64, q: u64) -> (u64, u64) {
    // x = (u + v) mod q, but u+v may overflow u64 when q is close to 2^64 (e.g. Goldilocks).
    let (sum, carry) = u.overflowing_add(v);
    // In that case sum ≡ u+v - 2^64 (mod 2^64). Since q < 2^64, we need to add (2^64 mod q)
    // which equals (2^64 - q) to recover the true sum modulo q.
    let mut x = if carry {
        // sum + (2^64 - q)
        sum.wrapping_add(u64::MAX.wrapping_sub(q).wrapping_add(1))
    } else {
        sum
    };
    if x >= q {
        x -= q;
    }

    // y = (u - v) mod q
    let y = if u >= v {
        u - v
    } else {
        u.wrapping_add(q).wrapping_sub(v)
    };
    (x, y)
}

#[inline]
fn mul_mod(a: u64, b: u64, q: u64) -> u64 {
    ((a as u128 * b as u128) % (q as u128)) as u64
}

fn mod_pow(mut a: u64, mut e: u64, q: u64) -> u64 {
    let mut r = 1u64;
    while e > 0 {
        if e & 1 == 1 {
            r = mul_mod(r, a, q);
        }
        a = mul_mod(a, a, q);
        e >>= 1;
    }
    r
}

fn mod_inv(a: u64, q: u64) -> Option<u64> {
    if a == 0 {
        return None;
    }
    // Fermat for prime q: a^(q-2)
    Some(mod_pow(a, q - 2, q))
}

fn bit_reverse(mut x: usize, bits: usize) -> usize {
    let mut r = 0usize;
    for _ in 0..bits {
        r = (r << 1) | (x & 1);
        x >>= 1;
    }
    r
}

fn bit_reverse_in_place(a: &mut [u64]) {
    let n = a.len();
    let bits = n.ilog2() as usize;
    for i in 0..n {
        let j = bit_reverse(i, bits);
        if i < j {
            a.swap(i, j);
        }
    }
}

