//! 可复现随机性工具：PRG 与（原型）离散高斯采样。
//!
//! - **`ShakePrg`**：种子经 SHA-256 后喂给 ChaCha20，用于需要确定性扩展比特的场景。
//! - **`GaussianSampler`**：Box–Muller 生成正态再四舍五入为整数，再约化到各模数；**非**常数时间、**非**精确离散高斯，仅用于代数原型。

use rand::RngCore;
use rand_chacha::ChaCha20Rng;
use sha2::{Digest, Sha256};
use rand::SeedableRng;

/// 以 SHA-256(seed) 为密钥的 ChaCha20 PRG（简单可复现扩展）。
#[allow(dead_code)]
#[derive(Clone)]
pub struct ShakePrg {
    rng: ChaCha20Rng,
}

#[allow(dead_code)]
impl ShakePrg {
    pub fn new(seed: &[u8]) -> Self {
        let digest = Sha256::digest(seed);
        let mut key = [0u8; 32];
        key.copy_from_slice(&digest[..32]);
        let rng = ChaCha20Rng::from_seed(key);
        Self { rng }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.rng.next_u64()
    }
}

/// Prototype discrete Gaussian sampler using Box–Muller over f64 and rounding.
///
/// This is **not** hardened; it exists to avoid external big-int/FFI deps while we build the full
/// commitment pipeline on MSVC.
pub struct GaussianSampler<'a, R: RngCore> {
    rng: &'a mut R,
    has_spare: bool,
    spare: f64,
}

impl<'a, R: RngCore> GaussianSampler<'a, R> {
    pub fn new(rng: &'a mut R) -> Self {
        Self {
            rng,
            has_spare: false,
            spare: 0.0,
        }
    }

    fn uniform_f64(&mut self) -> f64 {
        // 53-bit mantissa uniform in (0,1)
        let x = (self.rng.next_u64() >> 11) as u64;
        (x as f64 + 1.0) / ((1u64 << 53) as f64 + 2.0)
    }

    fn normal_std(&mut self) -> f64 {
        if self.has_spare {
            self.has_spare = false;
            return self.spare;
        }
        let u1 = self.uniform_f64();
        let u2 = self.uniform_f64();
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * std::f64::consts::PI * u2;
        self.spare = r * theta.sin();
        self.has_spare = true;
        r * theta.cos()
    }

    pub fn sample_i64(&mut self, sigma: f64) -> i64 {
        (self.normal_std() * sigma).round() as i64
    }
}

