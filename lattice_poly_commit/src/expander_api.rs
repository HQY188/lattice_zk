//! ExpanderPCS implementation for the lattice-based multilinear PC (mle_pc.md).
//! Uses Fiat-Shamir to make Eval non-interactive.

use arith::{Field, SimdField};
use gkr_engine::{
    ExpanderPCS, ExpanderSingleVarChallenge, FieldEngine, MPIEngine, PolynomialCommitmentType,
    StructuredReferenceString, Transcript,
};
use polynomials::MultilinearExtension;
use rand::{rngs::StdRng, RngCore, SeedableRng};
use serdes::{ExpSerde, SerdeResult};

use crate::multilinear::{
    self, commit_with_rng as mle_commit_with_rng, compute_a_b, compute_u, dot_product,
    eval_with_fs_given_a as mle_eval_with_fs_given_a, setup as mle_setup, verify as mle_verify,
    MleCk, MleCommitment, MleOpening, IOTA,
};
use crate::univariate::{random_poly};
use crate::univariate_real::{commit_real as uni_commit};

/// Build 2^l ChallengeField hypercube from 10-var Simd poly so that the lattice MLE evaluation
/// (head = first l-l/ι, tail = last l/ι per mle_pc.md Lemma 1) equals single_core_eval at
/// (r_simd, rz). We map lattice index idx = tail_7 + head_7*2^7 to expander (i_rz, i_simd):
/// r_head = [r_simd, rz[0..3]], r_tail = rz[3..10] => head_7 = (i_simd, i_rz_low3), tail_7 = i_rz_high7.
fn expand_simd_poly_to_challenge_hypercube<C: FieldEngine>(
    poly_evals: &[C::SimdCircuitField],
    l: usize,
    iota: usize,
) -> Vec<C::ChallengeField>
where
    C::SimdCircuitField: SimdField,
{
    let tail_len = l / iota;
    let _head_len = l - tail_len;
    let pack_size = C::SimdCircuitField::PACK_SIZE;
    let simd_bits = pack_size.ilog2() as usize;
    debug_assert_eq!(1usize << simd_bits, pack_size);

    // One SIMD multilinear eval has `num_vars` variables on the hypercube; each point is a SIMD
    // vector of `pack_size` lanes. Total indexed space is 2^l = 2^num_vars * pack_size.
    let hc_len = poly_evals.len();
    assert!(
        hc_len.is_power_of_two(),
        "SIMD hypercube basis length must be a power of two, got {}",
        hc_len
    );
    let num_poly_vars = hc_len.trailing_zeros() as usize;
    debug_assert_eq!(hc_len, 1usize << num_poly_vars);
    assert_eq!(
        poly_evals.len() * pack_size,
        1usize << l,
        "SIMD hypercube size mismatch: len={} pack={} l={}",
        poly_evals.len(),
        pack_size,
        l
    );

    // Same bit layout as the M31x16 special case: split `idx` into (head, tail) of lengths
    // (head_len, tail_len), take SIMD lane from low bits of `head`, and interleave the rest
    // with `tail` so that `i_rz` spans `num_poly_vars` bits.
    let shift_tail = tail_len.saturating_sub(simd_bits);

    let size = 1 << l;
    let mut out = vec![C::ChallengeField::zero(); size];
    for idx in 0..size {
        let tail = idx & ((1 << tail_len) - 1);
        let head = idx >> tail_len;
        let i_simd = head & (pack_size - 1);
        let i_rz = (head >> simd_bits) | (tail << shift_tail);
        debug_assert!(i_rz < poly_evals.len());
        out[idx] = C::ChallengeField::from(poly_evals[i_rz].unpack()[i_simd]);
    }
    out
}

/// Prover scratch: caches `(C, δ)` from [`LatticeMlePCS::commit`] so [`LatticeMlePCS::open`] skips
/// repeating `build_matrix_t` and full row commits (GKR calls `open` twice per proof for the same input poly).
#[derive(Clone, Debug, Default)]
pub struct LatticeMleScratchPad<F: Field + ExpSerde> {
    cached: Option<(MleCommitment<F>, MleOpening<F>)>,
}

impl<F: Field + ExpSerde> ExpSerde for LatticeMleScratchPad<F> {
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> SerdeResult<()> {
        match &self.cached {
            None => 0u8.serialize_into(&mut writer)?,
            Some((c, d)) => {
                1u8.serialize_into(&mut writer)?;
                c.serialize_into(&mut writer)?;
                d.serialize_into(&mut writer)?;
            }
        }
        Ok(())
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> SerdeResult<Self> {
        let tag = u8::deserialize_from(&mut reader)?;
        let cached = match tag {
            0 => None,
            1 => Some((
                MleCommitment::deserialize_from(&mut reader)?,
                MleOpening::deserialize_from(&mut reader)?,
            )),
            _ => return Err(serdes::SerdeError::DeserializeError),
        };
        Ok(Self { cached })
    }
}

/// Lattice-based MLE PCS for Expander GKR.
pub struct LatticeMlePCS<C: FieldEngine> {
    _phantom: std::marker::PhantomData<C>,
}

impl<C: FieldEngine> ExpanderPCS<C> for LatticeMlePCS<C>
where
    C::SimdCircuitField: Field + ExpSerde + SimdField,
    C::ChallengeField: Field + ExpSerde + From<C::CircuitField>,
{
    const NAME: &'static str = "LatticeMlePCS";

    const PCS_TYPE: PolynomialCommitmentType = PolynomialCommitmentType::Lattice;

    type Params = usize;
    type ScratchPad = LatticeMleScratchPad<C::ChallengeField>;

    type SRS = MleCk<C::ChallengeField>;
    type Commitment = MleCommitment<C::ChallengeField>;
    type Opening = LatticeMleOpening<C>;
    type BatchOpening = ();

    fn gen_srs(
        params: &Self::Params,
        _mpi_engine: &impl MPIEngine,
        _rng: impl RngCore,
    ) -> Self::SRS {
        let l = *params;
        assert!(l >= 1, "l must be >= 1");
        let iota = if l % 2 == 0 { IOTA } else { 1 };
        mle_setup(128, l, iota)
    }

    fn gen_params(n_input_vars: usize, world_size: usize) -> Self::Params {
        let mpi_bits = (world_size as u32).ilog2() as usize;
        let simd_bits = (C::SimdCircuitField::PACK_SIZE as u32).ilog2() as usize;
        n_input_vars + simd_bits + mpi_bits
    }

    fn init_scratch_pad(_params: &Self::Params, _mpi_engine: &impl MPIEngine) -> Self::ScratchPad {
        LatticeMleScratchPad::default()
    }

    fn commit(
        params: &Self::Params,
        mpi_engine: &impl MPIEngine,
        proving_key: &<Self::SRS as StructuredReferenceString>::PKey,
        poly: &impl MultilinearExtension<C::SimdCircuitField>,
        scratch_pad: &mut Self::ScratchPad,
    ) -> Option<Self::Commitment> {
        let l = *params;
        assert!(l >= 1 && poly.num_vars() <= l);
        let iota = proving_key.iota;

        let f_challenge: Vec<C::ChallengeField> = if poly.num_vars() < l {
            expand_simd_poly_to_challenge_hypercube::<C>(&poly.hypercube_basis(), l, iota)
        } else {
            poly.hypercube_basis()
                .iter()
                .map(|x| C::ChallengeField::from(x.unpack()[0]))
                .collect()
        };

        if mpi_engine.is_single_process() {
            let mut rng = StdRng::seed_from_u64(0);
            let (c, delta) = mle_commit_with_rng(proving_key, &f_challenge, &mut rng);
            scratch_pad.cached = Some((c.clone(), delta));
            return Some(c);
        }

        let mut buffer = if mpi_engine.is_root() {
            vec![
                C::SimdCircuitField::ZERO;
                poly.hypercube_size() * mpi_engine.world_size()
            ]
        } else {
            vec![]
        };
        mpi_engine.gather_vec(poly.hypercube_basis_ref(), &mut buffer);

        if !mpi_engine.is_root() {
            return None;
        }

        assert_eq!(buffer.len(), 1 << l);
        let f_challenge: Vec<C::ChallengeField> = buffer
            .iter()
            .map(|x| C::ChallengeField::from(x.unpack()[0]))
            .collect();
        let mut rng = StdRng::seed_from_u64(0);
        let (c, delta) = mle_commit_with_rng(proving_key, &f_challenge, &mut rng);
        scratch_pad.cached = Some((c.clone(), delta));
        Some(c)
    }

    fn open(
        params: &Self::Params,
        mpi_engine: &impl MPIEngine,
        proving_key: &<Self::SRS as StructuredReferenceString>::PKey,
        poly: &impl MultilinearExtension<C::SimdCircuitField>,
        x: &ExpanderSingleVarChallenge<C>,
        transcript: &mut impl Transcript,
        scratch_pad: &Self::ScratchPad,
    ) -> Option<Self::Opening> {
        let r = x.global_xs();
        let l = r.len();
        assert_eq!(l, *params);
        let iota = proving_key.iota;

        let mut f_challenge: Vec<C::ChallengeField> = if poly.num_vars() < l {
            expand_simd_poly_to_challenge_hypercube::<C>(&poly.hypercube_basis(), l, iota)
        } else {
            poly.hypercube_basis()
                .iter()
                .map(|v| C::ChallengeField::from(v.unpack()[0]))
                .collect()
        };

        if !mpi_engine.is_single_process() {
            let mut buffer = if mpi_engine.is_root() {
                vec![
                    C::SimdCircuitField::ZERO;
                    poly.hypercube_size() * mpi_engine.world_size()
                ]
            } else {
                vec![]
            };
            mpi_engine.gather_vec(poly.hypercube_basis_ref(), &mut buffer);
            if !mpi_engine.is_root() {
                return None;
            }
            f_challenge = buffer
                .iter()
                .map(|v| C::ChallengeField::from(v.unpack()[0]))
                .collect();
        }

        // Reuse `(c, δ)` from `commit` when present (GKR: same input poly, same deterministic RNG).
        let fresh_commit = if scratch_pad.cached.is_none() {
            let mut rng_commit = StdRng::seed_from_u64(0);
            Some(mle_commit_with_rng(proving_key, &f_challenge, &mut rng_commit))
        } else {
            None
        };
        let (c, delta) = match &scratch_pad.cached {
            Some((c, delta)) => (c, delta),
            None => {
                let (c, delta) = fresh_commit.as_ref().expect("fresh commit when cache empty");
                (c, delta)
            }
        };

        let t = multilinear::build_matrix_t(&f_challenge, l, iota);
        let (a, b) = compute_a_b(&r, l, iota);
        let u = compute_u(&a, &t);
        let y = dot_product(&u, &b);

        let n_cols = 1 << (l - l / iota);
        let mut rng = rand::thread_rng();
        let a_poly = random_poly::<C::ChallengeField>(n_cols, &mut rng);
        let (c_a, delta_a) = uni_commit(&proving_key.ck_uni, &a_poly, &mut rng);
        let t_val = dot_product(&a_poly, &b);

        transcript.append_field_element(&y);
        let mut pi1_buf = vec![];
        c_a.serialize_into(&mut pi1_buf).unwrap();
        t_val.serialize_into(&mut pi1_buf).unwrap();
        transcript.append_u8_slice(&pi1_buf);

        let e = transcript.generate_field_element::<C::ChallengeField>();

        let (y2, pi1, pi2) = mle_eval_with_fs_given_a(
            proving_key,
            &c,
            &r,
            &t,
            &delta,
            &a_poly,
            c_a,
            delta_a,
            e,
        );
        debug_assert_eq!(y2, y);

        Some(LatticeMleOpening {
            y,
            pi1,
            pi2,
            _phantom: std::marker::PhantomData,
        })
    }

    fn verify(
        params: &Self::Params,
        verifying_key: &<Self::SRS as StructuredReferenceString>::VKey,
        commitment: &Self::Commitment,
        x: &ExpanderSingleVarChallenge<C>,
        v: C::ChallengeField,
        transcript: &mut impl Transcript,
        opening: &Self::Opening,
    ) -> bool {
        let r = x.global_xs();
        assert_eq!(r.len(), *params);
        if opening.y != v {
            return false;
        }
        transcript.append_field_element(&opening.y);
        let mut pi1_buf = vec![];
        opening.pi1.0.serialize_into(&mut pi1_buf).unwrap();
        opening.pi1.1.serialize_into(&mut pi1_buf).unwrap();
        transcript.append_u8_slice(&pi1_buf);
        let e = transcript.generate_field_element::<C::ChallengeField>();
        mle_verify(
            verifying_key,
            commitment,
            &r,
            opening.y,
            e,
            &opening.pi1,
            &opening.pi2,
        )
    }
}

/// Opening for lattice MLE: (y, π1, π2) with Fiat-Shamir challenge e in transcript.
#[derive(Clone, Debug, Default)]
pub struct LatticeMleOpening<C: FieldEngine> {
    pub y: C::ChallengeField,
    pub pi1: (
        crate::univariate_real::UniCommitmentReal,
        C::ChallengeField,
    ),
    pub pi2: (
        Vec<C::ChallengeField>,
        crate::univariate_real::UniOpeningReal,
        crate::univariate_real::UniCommitmentReal,
    ),
    _phantom: std::marker::PhantomData<C>,
}

/// Max length for opening pi2.0 (z coeffs) to avoid huge allocation from corrupt proof bytes.
const MAX_OPENING_Z_LEN: usize = 1 << 24;

impl<C: FieldEngine> ExpSerde for LatticeMleOpening<C>
where
    C::ChallengeField: ExpSerde,
{
    fn serialize_into<W: std::io::Write>(&self, mut writer: W) -> serdes::SerdeResult<()> {
        self.y.serialize_into(&mut writer)?;
        self.pi1.0.serialize_into(&mut writer)?;
        self.pi1.1.serialize_into(&mut writer)?;
        self.pi2.0.serialize_into(&mut writer)?;
        self.pi2.1.serialize_into(&mut writer)?;
        self.pi2.2.serialize_into(&mut writer)?;
        Ok(())
    }

    fn deserialize_from<R: std::io::Read>(mut reader: R) -> serdes::SerdeResult<Self> {
        let y = C::ChallengeField::deserialize_from(&mut reader)?;
        let pi1_0 = crate::univariate_real::UniCommitmentReal::deserialize_from(&mut reader)?;
        let pi1_1 = C::ChallengeField::deserialize_from(&mut reader)?;
        let len_z = usize::deserialize_from(&mut reader)?;
        if len_z > MAX_OPENING_Z_LEN {
            return Err(serdes::SerdeError::DeserializeError);
        }
        let mut pi2_0 = Vec::with_capacity(len_z);
        for _ in 0..len_z {
            pi2_0.push(C::ChallengeField::deserialize_from(&mut reader)?);
        }
        let pi2_1 = crate::univariate_real::UniOpeningReal::deserialize_from(&mut reader)?;
        let pi2_2 = crate::univariate_real::UniCommitmentReal::deserialize_from(&mut reader)?;
        Ok(Self {
            y,
            pi1: (pi1_0, pi1_1),
            pi2: (pi2_0, pi2_1, pi2_2),
            _phantom: std::marker::PhantomData,
        })
    }
}
