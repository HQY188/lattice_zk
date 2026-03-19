use goldilocks::Goldilocks;
use rand::{rngs::StdRng, RngCore, SeedableRng};
use arith::Field;

use lattice_poly_commit::multilinear;

#[test]
fn test_mle_commit_open_small_l() {
    let mut rng = StdRng::seed_from_u64(123);
    let l = 8usize;
    let iota = 2usize;
    let ck = multilinear::setup::<Goldilocks>(128, l, iota);

    let mut f = vec![Goldilocks::ZERO; 1 << l];
    for v in f.iter_mut() {
        *v = Goldilocks::from(rng.next_u64());
    }

    let (com, delta) = multilinear::commit(&ck, &f);
    assert!(multilinear::open(&ck, &com, &f, &delta));
}

#[test]
fn test_mle_eval_verify_with_fs() {
    let mut rng = StdRng::seed_from_u64(999);
    let l = 8usize;
    let iota = 2usize;
    let ck = multilinear::setup::<Goldilocks>(128, l, iota);

    let mut f = vec![Goldilocks::ZERO; 1 << l];
    for v in f.iter_mut() {
        *v = Goldilocks::from(rng.next_u64());
    }

    let (com, delta) = multilinear::commit(&ck, &f);
    let t = multilinear::build_matrix_t(&f, l, iota);

    // random r in F^l
    let mut r = vec![Goldilocks::ZERO; l];
    for ri in r.iter_mut() {
        *ri = Goldilocks::from(rng.next_u64());
    }

    let e = Goldilocks::from(rng.next_u64());
    let mut prover_rng = StdRng::seed_from_u64(2026);
    let (y, pi1, pi2) = multilinear::eval_with_fs(&ck, &com, &r, &t, &delta, e, &mut prover_rng);

    assert!(multilinear::verify(&ck, &com, &r, y, e, &pi1, &pi2));
}

