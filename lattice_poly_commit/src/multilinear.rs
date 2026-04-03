//! Lattice-based multilinear polynomial commitment (mle_pc.md).
//! Matrix T from f, row-wise univariate commitments, Eval/Verify with Fiat-Shamir.
//!
//! Parallel matrix `T` construction, row commit, [`compute_u`], and [`dot_product`] (above a length threshold)
//! share the same environment variables (read once per process on first use):
//! - [`ENV_COMMIT_PARALLEL`]: set to `0`, `false`, or `no` to force sequential versions.
//! - [`ENV_COMMIT_THREADS`]: if set to a positive integer, use a dedicated thread pool with that many threads;
//!   if unset, Rayon's **global** pool is used, which respects **`RAYON_NUM_THREADS`**.
//! - Per-row [`crate::ajtai::AjtaiCrs::commit`] can parallelize over μ when not on a Rayon worker; see
//!   [`crate::ajtai::ENV_AJTAI_COMMIT_PARALLEL`] / [`crate::ajtai::PAR_AJTAI_COMMIT_MIN_MU`].

use std::sync::OnceLock;

use arith::Field;
use gkr_engine::StructuredReferenceString;
use polynomials::EqPolynomial;
use rand::RngCore;
use rand::SeedableRng;
use rayon::prelude::*;
use rayon::{ThreadPool, ThreadPoolBuilder};
use serdes::{ExpSerde, SerdeResult};

use crate::ring::PolyRns;
use crate::univariate::{random_poly};
use crate::univariate_real::{
    commit_real as uni_commit,
    commit_real_with_r_vec as uni_commit_with_r_vec,
    open_real as uni_open,
    open_real_commitment_only as uni_open_commit_only,
    setup_real as uni_setup,
    UniCkReal, UniCommitmentReal, UniOpeningReal,
};

/// ι ≥ 2, l/ι integer. We use ι = 2.
pub const IOTA: usize = 2;

/// Set to `0`, `false`, or `no` (case-insensitive) to disable parallel matrix build, row commit, `compute_u`, and `dot_product`.
/// Default: parallel when each routine's granularity threshold is met.
pub const ENV_COMMIT_PARALLEL: &str = "LATTICE_MLE_COMMIT_PARALLEL";

/// If set to a positive integer, lattice MLE parallel routines use a **dedicated** Rayon pool with that many worker threads.
/// If unset, the **global** Rayon pool is used; configure it with **`RAYON_NUM_THREADS`**.
///
/// Parsed once per process on first parallel use; set before first use if you rely on it.
pub const ENV_COMMIT_THREADS: &str = "LATTICE_MLE_COMMIT_THREADS";

/// Minimum slice length for parallel [`dot_product`] when [`ENV_COMMIT_PARALLEL`] is on (avoids Rayon overhead on short vectors).
pub const PAR_DOT_PRODUCT_MIN_LEN: usize = 2048;

static COMMIT_THREAD_POOL: OnceLock<Option<ThreadPool>> = OnceLock::new();

fn commit_row_parallel_enabled() -> bool {
    match std::env::var(ENV_COMMIT_PARALLEL) {
        Ok(s) => {
            let s = s.trim().to_ascii_lowercase();
            !(s.is_empty() || s == "0" || s == "false" || s == "no")
        }
        Err(_) => true,
    }
}

fn commit_thread_pool() -> &'static Option<ThreadPool> {
    COMMIT_THREAD_POOL.get_or_init(|| {
        std::env::var(ENV_COMMIT_THREADS)
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .filter(|&n| n > 0)
            .map(|n| {
                ThreadPoolBuilder::new()
                    .num_threads(n)
                    .thread_name(|i| format!("lattice-mle-commit-{i}"))
                    .build()
                    .expect("lattice commit thread pool")
            })
    })
}

fn commit_rows_zip<F: Field + ExpSerde>(
    ck: &MleCk<F>,
    t: &[Vec<F>],
    r_vecs: Vec<Vec<PolyRns>>,
) -> (Vec<UniCommitmentReal>, Vec<UniOpeningReal>) {
    let n_rows = t.len();
    let use_rayon = commit_row_parallel_enabled() && n_rows > 1;
    if !use_rayon {
        return t
            .iter()
            .zip(r_vecs.into_iter())
            .map(|(row, r_vec)| uni_commit_with_r_vec(&ck.ck_uni, row, r_vec))
            .unzip();
    }
    match commit_thread_pool().as_ref() {
        Some(pool) => pool.install(|| {
            t.par_iter()
                .zip(r_vecs.into_par_iter())
                .map(|(row, r_vec)| uni_commit_with_r_vec(&ck.ck_uni, row, r_vec))
                .unzip()
        }),
        None => t
            .par_iter()
            .zip(r_vecs.into_par_iter())
            .map(|(row, r_vec)| uni_commit_with_r_vec(&ck.ck_uni, row, r_vec))
            .unzip(),
    }
}

/// Multilinear ck = (ck_uni, l, ι).
#[derive(Clone, Debug)]
pub struct MleCk<F: Field> {
    pub ck_uni: UniCkReal,
    pub l: usize,
    pub iota: usize,
    _phantom: std::marker::PhantomData<F>,
}

impl<F: Field + ExpSerde> ExpSerde for MleCk<F> {
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> SerdeResult<()> {
        self.ck_uni.serialize_into(&mut writer)?;
        self.l.serialize_into(&mut writer)?;
        self.iota.serialize_into(&mut writer)?;
        Ok(())
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> SerdeResult<Self> {
        let ck_uni = UniCkReal::deserialize_from(&mut reader)?;
        let l = usize::deserialize_from(&mut reader)?;
        let iota = usize::deserialize_from(&mut reader)?;
        Ok(Self {
            ck_uni,
            l,
            iota,
            _phantom: std::marker::PhantomData,
        })
    }
}

impl<F: Field + ExpSerde> Default for MleCk<F> {
    fn default() -> Self {
        let params = crate::params::LatticeParams::default_small();
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let crs = crate::ajtai::AjtaiCrs::setup(params.clone(), &mut rng);
        Self {
            ck_uni: UniCkReal { params, crs },
            l: 0,
            iota: IOTA,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<F: Field + ExpSerde + 'static> StructuredReferenceString for MleCk<F> {
    type PKey = MleCk<F>;
    type VKey = MleCk<F>;

    fn into_keys(self) -> (Self::PKey, Self::VKey) {
        let ck = self;
        (ck.clone(), ck)
    }
}

/// Commitment C = (c_1, ..., c_{2^{l/ι}}).
#[derive(Clone, Debug, Default)]
pub struct MleCommitment<F: Field> {
    pub row_commitments: Vec<UniCommitmentReal>,
    _phantom: std::marker::PhantomData<F>,
}

impl<F: Field + ExpSerde> ExpSerde for MleCommitment<F> {
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> SerdeResult<()> {
        self.row_commitments.serialize_into(&mut writer)
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> SerdeResult<Self> {
        let row_commitments = Vec::deserialize_from(&mut reader)?;
        Ok(Self {
            row_commitments,
            _phantom: std::marker::PhantomData,
        })
    }
}

/// Opening δ = { δ_i }.
#[derive(Clone, Debug, Default)]
pub struct MleOpening<F: Field> {
    pub row_openings: Vec<UniOpeningReal>,
    _phantom: std::marker::PhantomData<F>,
}

impl<F: Field + ExpSerde> ExpSerde for MleOpening<F> {
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> SerdeResult<()> {
        self.row_openings.serialize_into(&mut writer)
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> SerdeResult<Self> {
        let row_openings = Vec::deserialize_from(&mut reader)?;
        Ok(Self {
            row_openings,
            _phantom: std::marker::PhantomData,
        })
    }
}

/// Commitment + its openings, for use in ExpanderPCS (prover side needs δ as well).
#[derive(Clone, Debug, Default)]
pub struct MleCommitmentWithOpening<F: Field> {
    pub commitment: MleCommitment<F>,
    pub opening: MleOpening<F>,
    _phantom: std::marker::PhantomData<F>,
}

impl<F: Field + ExpSerde> ExpSerde for MleCommitmentWithOpening<F> {
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> SerdeResult<()> {
        self.commitment.serialize_into(&mut writer)?;
        self.opening.serialize_into(&mut writer)?;
        Ok(())
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> SerdeResult<Self> {
        let commitment = MleCommitment::<F>::deserialize_from(&mut reader)?;
        let opening = MleOpening::<F>::deserialize_from(&mut reader)?;
        Ok(Self {
            commitment,
            opening,
            _phantom: std::marker::PhantomData,
        })
    }
}

/// PC.Setup(1^λ, l) -> ck. N = 2^{l - l/ι}. ι=1 allowed for odd l (then one column).
pub fn setup<F: Field>(lambda: usize, l: usize, iota: usize) -> MleCk<F> {
    assert!(iota >= 1 && (iota == 1 || l % iota == 0));
    let n_cols = 1 << (l - l / iota);
    let mut rng = rand::thread_rng();
    let ck_uni = uni_setup(lambda, n_cols, &mut rng);
    MleCk {
        ck_uni,
        l,
        iota,
        _phantom: std::marker::PhantomData,
    }
}

/// Lemma 1 indexing: head = first (l - l/ι) bits, tail = last l/ι bits.
/// b^(i,j) = (b_head^(j), b_tail^(i)). Full hypercube index = j * 2^{l/ι} + i.
#[inline]
fn matrix_index(l: usize, iota: usize, row: usize, col: usize) -> usize {
    let tail_bits = l / iota;
    (col << tail_bits) | row
}

/// Build matrix T from f (hypercube values). T[i][j] = f(b^(i,j)).
/// Row i has 2^{l - l/ι} elements.
///
/// When [`ENV_COMMIT_PARALLEL`] allows it and `n_rows > 1`, rows are filled in parallel (same pool as row commit).
pub fn build_matrix_t<F: Field + Send + Sync>(
    f_vals: &[F],
    l: usize,
    iota: usize,
) -> Vec<Vec<F>> {
    assert_eq!(f_vals.len(), 1 << l);
    let n_rows = 1 << (l / iota);
    let n_cols = 1 << (l - l / iota);

    let use_rayon = commit_row_parallel_enabled() && n_rows > 1;
    if !use_rayon {
        let mut t: Vec<Vec<F>> = (0..n_rows).map(|_| vec![F::ZERO; n_cols]).collect();
        for (i, row) in t.iter_mut().enumerate() {
            for j in 0..n_cols {
                let idx = matrix_index(l, iota, i, j);
                row[j] = f_vals[idx];
            }
        }
        return t;
    }

    // Build `t` inside the closure so `ThreadPool::install` receives a `Send` closure (no `&mut` to caller stack).
    let fill_parallel = || {
        let mut t: Vec<Vec<F>> = (0..n_rows).map(|_| vec![F::ZERO; n_cols]).collect();
        t.par_iter_mut().enumerate().for_each(|(i, row)| {
            for j in 0..n_cols {
                let idx = matrix_index(l, iota, i, j);
                row[j] = f_vals[idx];
            }
        });
        t
    };

    match commit_thread_pool().as_ref() {
        Some(pool) => pool.install(fill_parallel),
        None => fill_parallel(),
    }
}

/// PC.Com(ck, f) -> (C, δ).
///
/// Row commitments run in parallel (rayon) after randomness is drawn sequentially from `rng`,
/// so the result matches the previous sequential implementation for the same RNG state.
pub fn commit_with_rng<F: Field + ExpSerde>(
    ck: &MleCk<F>,
    f_vals: &[F],
    rng: &mut impl RngCore,
) -> (MleCommitment<F>, MleOpening<F>) {
    let t = build_matrix_t(f_vals, ck.l, ck.iota);
    let n_rows = t.len();
    let r_vecs: Vec<Vec<PolyRns>> = (0..n_rows)
        .map(|_| ck.ck_uni.crs.sample_rand_vec(rng))
        .collect();
    let (row_commitments, row_openings) = commit_rows_zip(ck, &t, r_vecs);
    (
        MleCommitment {
            row_commitments,
            _phantom: std::marker::PhantomData,
        },
        MleOpening {
            row_openings,
            _phantom: std::marker::PhantomData,
        },
    )
}

pub fn commit<F: Field + ExpSerde>(
    ck: &MleCk<F>,
    f_vals: &[F],
) -> (MleCommitment<F>, MleOpening<F>) {
    let t = build_matrix_t(f_vals, ck.l, ck.iota);
    let n_rows = t.len();
    let mut rng = rand::thread_rng();
    let r_vecs: Vec<Vec<PolyRns>> = (0..n_rows)
        .map(|_| ck.ck_uni.crs.sample_rand_vec(&mut rng))
        .collect();
    let (row_commitments, row_openings) = commit_rows_zip(ck, &t, r_vecs);
    (
        MleCommitment {
            row_commitments,
            _phantom: std::marker::PhantomData,
        },
        MleOpening {
            row_openings,
            _phantom: std::marker::PhantomData,
        },
    )
}

/// PC.Open(ck, C, f, δ): verify each row.
#[allow(dead_code)]
pub fn open<F: Field + ExpSerde>(
    ck: &MleCk<F>,
    c: &MleCommitment<F>,
    f_vals: &[F],
    delta: &MleOpening<F>,
) -> bool {
    let t = build_matrix_t(f_vals, ck.l, ck.iota);
    if c.row_commitments.len() != t.len() || delta.row_openings.len() != t.len() {
        return false;
    }
    for (i, row) in t.iter().enumerate() {
        if !uni_open::<F>(
            &ck.ck_uni,
            &c.row_commitments[i],
            row.as_slice(),
            &delta.row_openings[i],
        ) {
            return false;
        }
    }
    true
}

/// Compute A[i] = χ_{b_tail^(i)}(r_tail), B[j] = χ_{b_head^(j)}(r_head).
/// r is full length l; head = first (l - l/ι), tail = last l/ι.
pub fn compute_a_b<F: Field>(r: &[F], l: usize, iota: usize) -> (Vec<F>, Vec<F>) {
    let head_len = l - l / iota;
    let tail_len = l / iota;
    assert_eq!(r.len(), l);
    let r_head = &r[..head_len];
    let r_tail = &r[head_len..];
    let a = EqPolynomial::<F>::build_eq_x_r(r_tail);
    let b = EqPolynomial::<F>::build_eq_x_r(r_head);
    assert_eq!(a.len(), 1 << tail_len);
    assert_eq!(b.len(), 1 << head_len);
    (a, b)
}

/// u = A · T (vector u of length 2^{l - l/ι}); u[j] = sum_i A[i] * T[i][j].
///
/// When [`ENV_COMMIT_PARALLEL`] allows it and `n_cols > 1`, columns of `u` are computed in parallel (same pool as matrix/commit).
pub fn compute_u<F: Field + Send + Sync>(a: &[F], t: &[Vec<F>]) -> Vec<F> {
    assert!(!t.is_empty());
    let n_rows = t.len();
    let n_cols = t[0].len();
    assert_eq!(a.len(), n_rows);
    debug_assert!(t.iter().all(|row| row.len() == n_cols));

    let use_rayon = commit_row_parallel_enabled() && n_cols > 1;
    if !use_rayon {
        let mut u = vec![F::ZERO; n_cols];
        for (i, row) in t.iter().enumerate() {
            let ai = a[i];
            for (j, t_ij) in row.iter().enumerate() {
                u[j] = u[j] + ai * *t_ij;
            }
        }
        return u;
    }

    let run_parallel = || {
        (0..n_cols)
            .into_par_iter()
            .map(|j| {
                let mut acc = F::ZERO;
                for i in 0..n_rows {
                    acc = acc + a[i] * t[i][j];
                }
                acc
            })
            .collect()
    };

    match commit_thread_pool().as_ref() {
        Some(pool) => pool.install(run_parallel),
        None => run_parallel(),
    }
}

/// y = <u, B>.
///
/// When [`ENV_COMMIT_PARALLEL`] allows it and `u.len() >= PAR_DOT_PRODUCT_MIN_LEN`, uses a parallel sum (same pool as matrix/commit).
pub fn dot_product<F: Field + Send + Sync>(u: &[F], b: &[F]) -> F {
    assert_eq!(u.len(), b.len());
    let seq = || u.iter().zip(b.iter()).map(|(x, y)| *x * *y).sum();
    let n = u.len();
    if !commit_row_parallel_enabled() || n < PAR_DOT_PRODUCT_MIN_LEN {
        return seq();
    }

    let run_parallel = || {
        u.par_iter()
            .zip(b.par_iter())
            .map(|(x, y)| *x * *y)
            .sum()
    };

    match commit_thread_pool().as_ref() {
        Some(pool) => pool.install(run_parallel),
        None => run_parallel(),
    }
}

/// PC.Eval with Fiat-Shamir: prover computes (y, π1, π2) and derives e from transcript.
/// Returns (y, π1, π2) where π1 = (c_a, t), π2 = (z_coeffs, delta_z, c_z).
#[allow(clippy::type_complexity, dead_code)]
pub fn eval_with_fs<F: Field + ExpSerde>(
    ck: &MleCk<F>,
    c: &MleCommitment<F>,
    r: &[F],
    t: &[Vec<F>],
    delta: &MleOpening<F>,
    e: F, // challenge from transcript (Fiat-Shamir)
    rng: &mut impl RngCore,
) -> (
    F,
    (UniCommitmentReal, F),
    (Vec<F>, UniOpeningReal, UniCommitmentReal),
) {
    let (a, b) = compute_a_b(r, ck.l, ck.iota);
    let u = compute_u(&a, t);
    let y = dot_product(&u, &b);

    let n_cols = 1 << (ck.l - ck.l / ck.iota);
    let a_poly = random_poly::<F>(n_cols, rng);
    let mut uni_rng = rand::thread_rng();
    let (c_a, delta_a) = uni_commit(&ck.ck_uni, &a_poly, &mut uni_rng);
    let t_val = dot_product(&a_poly, &b);

    // z = a + e * u (coefficient-wise)
    let z: Vec<F> = a_poly
        .iter()
        .zip(u.iter())
        .map(|(ai, ui)| *ai + e * *ui)
        .collect();

    let c_u = linear_combine_commitments(&ck.ck_uni, &c.row_commitments, &a);
    let c_z = add_scalar_mul(&ck.ck_uni, &c_a, &e, &c_u);
    let pi1 = (c_a, t_val);

    // Build an opening for c_u from the row openings: δ_u corresponds to u.
    // Commitment linearity is over the ring: multiplying a commitment by an encoded scalar
    // corresponds to multiplying both message and randomness polynomials by that scalar.
    if delta.row_openings.len() != a.len() {
        panic!("delta length mismatch: expected {} row openings", a.len());
    }

    let scalar_polys: Vec<_> = a.iter().map(|ai| crate::encoder::encode_block(&[*ai], &ck.ck_uni.params)).collect();

    // One-block prototype: accumulate r_u (length nu) as sum_i scalar_i * r_i.
    let nu = ck.ck_uni.params.nu;
    let mut r_u = (0..nu)
        .map(|_| crate::ring::PolyRns::zero(ck.ck_uni.params.ring_degree, ck.ck_uni.params.moduli.clone()))
        .collect::<Vec<_>>();

    let ntt = ck.ck_uni.params.ntt_plan();
    for (i, opening_i) in delta.row_openings.iter().enumerate() {
        // r_blocks[0] is the nu-vector for the single message block.
        if opening_i.r_blocks.len() != 1 || opening_i.r_blocks[0].len() != nu {
            panic!("unexpected opening shape for row {}", i);
        }
        for j in 0..nu {
            let prod = opening_i.r_blocks[0][j].mul_negacyclic(&scalar_polys[i], &ntt);
            r_u[j].add_assign(&prod);
        }
    }

    // Now build δ_z consistent with c_z = c_a + e*c_u:
    // m_z = encode(z), r_z = r_a + e*r_u.
    let m_z = crate::encoder::encode_block(&z, &ck.ck_uni.params);
    let e_poly = crate::encoder::encode_block(&[e], &ck.ck_uni.params);

    let mut r_z = delta_a.r_blocks[0].clone();
    for j in 0..nu {
        let prod = r_u[j].mul_negacyclic(&e_poly, &ntt);
        r_z[j].add_assign(&prod);
    }

    let delta_z = UniOpeningReal {
        m_blocks: vec![m_z],
        r_blocks: vec![r_z],
    };

    let pi2 = (z, delta_z, c_z);

    (y, pi1, pi2)
}

/// Same as `eval_with_fs`, but uses caller-provided `a_poly`, `c_a` and `delta_a`
/// so that Fiat-Shamir challenge generation can depend on `pi1` computed outside.
pub fn eval_with_fs_given_a<F: Field + ExpSerde>(
    ck: &MleCk<F>,
    c: &MleCommitment<F>,
    r: &[F],
    t: &[Vec<F>],
    delta: &MleOpening<F>,
    a_poly: &[F],
    c_a: UniCommitmentReal,
    delta_a: UniOpeningReal,
    e: F,
) -> (
    F,
    (UniCommitmentReal, F),
    (Vec<F>, UniOpeningReal, UniCommitmentReal),
) {
    let (a, b) = compute_a_b(r, ck.l, ck.iota);
    let u = compute_u(&a, t);
    let y = dot_product(&u, &b);

    let n_cols = 1 << (ck.l - ck.l / ck.iota);
    assert_eq!(a_poly.len(), n_cols);

    let t_val = dot_product(a_poly, &b);

    // z = a + e * u (coefficient-wise)
    let z: Vec<F> = a_poly
        .iter()
        .zip(u.iter())
        .map(|(ai, ui)| *ai + e * *ui)
        .collect();

    let c_u = linear_combine_commitments(&ck.ck_uni, &c.row_commitments, &a);
    let c_z = add_scalar_mul(&ck.ck_uni, &c_a, &e, &c_u);
    let pi1 = (c_a, t_val);

    // Build δ_z consistent with c_z = c_a + e*c_u
    if delta.row_openings.len() != a.len() {
        panic!("delta length mismatch: expected {} row openings", a.len());
    }

    let scalar_polys: Vec<_> = a
        .iter()
        .map(|ai| crate::encoder::encode_block(&[*ai], &ck.ck_uni.params))
        .collect();

    // One-block prototype: r_z = r_a + e * r_u where r_u = sum_i scalar_i * r_i
    let nu = ck.ck_uni.params.nu;
    let mut r_u = (0..nu)
        .map(|_| crate::ring::PolyRns::zero(ck.ck_uni.params.ring_degree, ck.ck_uni.params.moduli.clone()))
        .collect::<Vec<_>>();

    let ntt = ck.ck_uni.params.ntt_plan();
    let ring_degree = ck.ck_uni.params.ring_degree;

    for (i, opening_i) in delta.row_openings.iter().enumerate() {
        // r_blocks[0] is the nu-vector for the single message block.
        if opening_i.r_blocks.len() != 1 || opening_i.r_blocks[0].len() != nu {
            panic!("unexpected opening shape for row {}", i);
        }
        for j in 0..nu {
            let prod = opening_i.r_blocks[0][j].mul_negacyclic(&scalar_polys[i], &ntt);
            r_u[j].add_assign(&prod);
        }
    }

    // m_u = sum_i Ecd(a[i]) * row_openings[i].m_blocks[0] (ring linear combination)
    let mut m_u = crate::ring::PolyRns::zero(ring_degree, ck.ck_uni.params.moduli.clone());
    for (i, opening_i) in delta.row_openings.iter().enumerate() {
        let prod = opening_i.m_blocks[0].mul_negacyclic(&scalar_polys[i], &ntt);
        m_u.add_assign(&prod);
    }

    let e_poly = crate::encoder::encode_block(&[e], &ck.ck_uni.params);
    // m_z = m_a + e * m_u so that c_z = commit(m_z, r_z) matches c_a + e*c_u
    let mut m_z = delta_a.m_blocks[0].clone();
    let e_times_m_u = m_u.mul_negacyclic(&e_poly, &ntt);
    m_z.add_assign(&e_times_m_u);

    let mut r_z = delta_a.r_blocks[0].clone();
    for j in 0..nu {
        let prod = r_u[j].mul_negacyclic(&e_poly, &ntt);
        r_z[j].add_assign(&prod);
    }

    let delta_z = UniOpeningReal {
        m_blocks: vec![m_z],
        r_blocks: vec![r_z],
    };

    let pi2 = (z, delta_z, c_z);
    (y, pi1, pi2)
}

/// c_u = sum_i R.Ecd(A[i]) · c_i.
pub fn linear_combine_commitments<F: Field>(
    ck_uni: &UniCkReal,
    commitments: &[UniCommitmentReal],
    coeffs: &[F],
) -> UniCommitmentReal {
    assert_eq!(commitments.len(), coeffs.len());
    // Prototype: commitment is one-block; linear combine blocks component-wise.
    let mut out_blocks = vec![crate::ajtai::AjtaiCommitment::zero(
        ck_uni.params.mu,
        ck_uni.params.ring_degree,
        ck_uni.params.moduli.clone(),
    ); 1];
    for (c, &a) in commitments.iter().zip(coeffs.iter()) {
        let scalar_poly = crate::encoder::encode_block(&[a], &ck_uni.params);
        for (k, out_block) in out_blocks.iter_mut().enumerate() {
            // out += scalar * c
            let mut tmp = c.blocks[k].clone();
            for v in tmp.value.iter_mut() {
                let prod = v.mul_negacyclic(&scalar_poly, &ck_uni.params.ntt_plan());
                *v = prod;
            }
            out_block.add_assign(&tmp);
        }
    }
    UniCommitmentReal { blocks: out_blocks }
}

pub fn add_scalar_mul<F: Field>(
    ck_uni: &UniCkReal,
    base: &UniCommitmentReal,
    e: &F,
    other: &UniCommitmentReal,
) -> UniCommitmentReal {
    let mut out = base.clone();
    let scalar_poly = crate::encoder::encode_block(&[*e], &ck_uni.params);
    for (k, block) in out.blocks.iter_mut().enumerate() {
        let mut tmp = other.blocks[k].clone();
        for v in tmp.value.iter_mut() {
            let prod = v.mul_negacyclic(&scalar_poly, &ck_uni.params.ntt_plan());
            *v = prod;
        }
        block.add_assign(&tmp);
    }
    out
}

/// PC.Verify(ck, C, r, y, π1, π2) with challenge e (from transcript).
pub fn verify<F: Field + ExpSerde>(
    ck: &MleCk<F>,
    c: &MleCommitment<F>,
    r: &[F],
    y: F,
    e: F,
    pi1: &(UniCommitmentReal, F),
    pi2: &(Vec<F>, UniOpeningReal, UniCommitmentReal),
) -> bool {
    let debug = std::env::var("LATTICE_MLE_DEBUG").is_ok();
    let (c_a, t) = pi1;
    let (z, delta_z, c_z) = pi2;

    let (a, b) = compute_a_b(r, ck.l, ck.iota);
    let c_u = linear_combine_commitments::<F>(&ck.ck_uni, &c.row_commitments, &a);

    // c_z ?= c_a + e * c_u
    let c_z_expected = add_scalar_mul::<F>(&ck.ck_uni, c_a, &e, &c_u);
    if c_z != &c_z_expected {
        if debug {
            eprintln!("[mle_verify] fail: c_z mismatch");
        }
        return false;
    }

    // <z, B> ?= t + e * y
    let z_dot_b = dot_product(z, &b);
    if z_dot_b != *t + e * y {
        if debug {
            eprintln!("[mle_verify] fail: z·B mismatch");
        }
        return false;
    }

    // PC.Open(ck_uni, c_z, δ_z): c_z must equal commit(δ_z.m, δ_z.r); δ_z.m = m_a + e*m_u (ring)
    let ok = uni_open_commit_only(&ck.ck_uni, c_z, delta_z);
    if !ok && debug {
        eprintln!("[mle_verify] fail: uni_open mismatch");
    }
    ok
}

#[cfg(test)]
mod commit_row_parallel_tests {
    use super::*;
    use goldilocks::Goldilocks;
    use rand::rngs::StdRng;
    use rand::{RngCore, SeedableRng};
    use serdes::ExpSerde;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn build_matrix_t_parallel_matches_sequential() {
        let _g = ENV_LOCK.lock().unwrap();
        let l = 12usize;
        let iota = 2usize;
        let f: Vec<_> = (0u64..(1u64 << l)).map(Goldilocks::from).collect();
        std::env::set_var(ENV_COMMIT_PARALLEL, "0");
        let sequential = build_matrix_t(&f, l, iota);
        std::env::remove_var(ENV_COMMIT_PARALLEL);
        let parallel = build_matrix_t(&f, l, iota);
        assert_eq!(parallel, sequential);
    }

    #[test]
    fn compute_u_parallel_matches_sequential() {
        let _g = ENV_LOCK.lock().unwrap();
        let n_rows = 32usize;
        let n_cols = 32usize;
        let a: Vec<_> = (0..n_rows).map(|i| Goldilocks::from(i as u64)).collect();
        let t: Vec<Vec<_>> = (0..n_rows)
            .map(|i| {
                (0..n_cols)
                    .map(|j| Goldilocks::from((i * n_cols + j) as u64))
                    .collect()
            })
            .collect();
        std::env::set_var(ENV_COMMIT_PARALLEL, "0");
        let sequential = compute_u(&a, &t);
        std::env::remove_var(ENV_COMMIT_PARALLEL);
        let parallel = compute_u(&a, &t);
        assert_eq!(parallel, sequential);
    }

    #[test]
    fn dot_product_parallel_matches_sequential() {
        let _g = ENV_LOCK.lock().unwrap();
        let n = PAR_DOT_PRODUCT_MIN_LEN + 64;
        let u: Vec<_> = (0..n).map(|i| Goldilocks::from(i as u64)).collect();
        let b: Vec<_> = (0..n).map(|i| Goldilocks::from((i * 7) as u64)).collect();
        std::env::set_var(ENV_COMMIT_PARALLEL, "0");
        let sequential = dot_product(&u, &b);
        std::env::remove_var(ENV_COMMIT_PARALLEL);
        let parallel = dot_product(&u, &b);
        assert_eq!(parallel, sequential);
    }

    #[test]
    fn commit_with_rng_same_seed_bit_identical() {
        let l = 8usize;
        let ck = setup::<Goldilocks>(128, l, IOTA);
        let mut rng_f = StdRng::seed_from_u64(42);
        let mut f = vec![Goldilocks::ZERO; 1 << l];
        for v in f.iter_mut() {
            *v = Goldilocks::from(rng_f.next_u64());
        }
        let mut rng_a = StdRng::seed_from_u64(0);
        let mut rng_b = StdRng::seed_from_u64(0);
        let (ca, da) = commit_with_rng(&ck, &f, &mut rng_a);
        let (cb, db) = commit_with_rng(&ck, &f, &mut rng_b);
        let mut ba = vec![];
        let mut bb = vec![];
        ca.serialize_into(&mut ba).unwrap();
        cb.serialize_into(&mut bb).unwrap();
        assert_eq!(ba, bb);
        da.serialize_into(&mut ba).unwrap();
        db.serialize_into(&mut bb).unwrap();
        assert_eq!(ba, bb);
    }
}
