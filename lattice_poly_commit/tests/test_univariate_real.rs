use goldilocks::Goldilocks;
use rand::{rngs::StdRng, RngCore, SeedableRng};
use arith::Field;

use lattice_poly_commit::{commit_real, eval_real, open_real, setup_real, verify_real};

#[test]
fn test_univariate_real_commit_open_roundtrip() {
    let mut rng = StdRng::seed_from_u64(42);
    let ck = setup_real(128, 256, &mut rng);

    let mut coeffs = vec![Goldilocks::ZERO; 256];
    for c in coeffs.iter_mut() {
        *c = Goldilocks::from(rng.next_u64());
    }

    let (com, delta) = commit_real(&ck, &coeffs, &mut rng);
    assert!(open_real(&ck, &com, &coeffs, &delta));
}

#[test]
fn test_univariate_real_eval_verify() {
    let mut rng = StdRng::seed_from_u64(7);
    let ck = setup_real(128, 256, &mut rng);

    let mut coeffs = vec![Goldilocks::ZERO; 256];
    for c in coeffs.iter_mut() {
        *c = Goldilocks::from(rng.next_u64());
    }

    let (com, delta) = commit_real(&ck, &coeffs, &mut rng);
    let x = Goldilocks::from(rng.next_u64());
    let (y, rho) = eval_real(&x, &delta);

    assert!(verify_real(&ck, &com, &x, y, &rho));
}

