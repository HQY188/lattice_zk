use serdes::{ExpSerde, SerdeResult};

use super::ntt::NttPlan;

/// RNS moduli (CRT primes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RnsModuli {
    pub moduli: Vec<u64>,
}

impl RnsModuli {
    pub fn new(moduli: Vec<u64>) -> Self {
        assert!(!moduli.is_empty(), "RNS moduli must be non-empty");
        for &q in &moduli {
            assert!(q > 2 && q % 2 == 1, "modulus must be an odd integer");
        }
        Self { moduli }
    }

    #[inline]
    pub fn level(&self) -> usize {
        self.moduli.len()
    }

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

/// RNS polynomial in Z_q[X]/(X^N + 1), stored as `level` slices of length `n`.
///
/// Representation:
/// - coefficients are always reduced to [0, q)
/// - `is_ntt` indicates whether each layer is in NTT domain (negacyclic NTT)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolyRns {
    pub n: usize,
    pub moduli: RnsModuli,
    pub coeffs: Vec<Vec<u64>>,
    pub is_ntt: bool,
}

impl PolyRns {
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

    pub fn assert_same_shape(&self, other: &Self) {
        assert_eq!(self.n, other.n);
        assert_eq!(self.moduli, other.moduli);
        assert_eq!(self.is_ntt, other.is_ntt);
    }

    #[inline]
    pub fn level(&self) -> usize {
        self.moduli.level()
    }

    pub fn clear(&mut self) {
        for layer in &mut self.coeffs {
            layer.fill(0);
        }
    }

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

    /// Multiply pointwise in NTT domain: self *= rhs.
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

    /// Convert to NTT domain in-place using the given plan.
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

    /// Convert from NTT domain back to coefficient domain in-place.
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

    /// Negacyclic multiplication in Z_q[X]/(X^N + 1).
    ///
    /// If operands are not in NTT domain, this will transform them, multiply, and inverse transform.
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

    /// Prototype squared 2-norm using balanced lift of the first modulus only.
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

    fn goldilocks_psi(n: usize) -> u64 {
        // Goldilocks modulus: 2^64 - 2^32 + 1, two-adicity 32.
        let q = goldilocks::GOLDILOCKS_MOD;
        let root_2_32 = goldilocks::Goldilocks::root_of_unity().v;
        let k_plus_1 = (2 * n).ilog2() as u64;
        assert!(k_plus_1 <= 32);
        let shift = 32 - k_plus_1;
        pow_mod(root_2_32, 1u64 << shift, q)
    }

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

