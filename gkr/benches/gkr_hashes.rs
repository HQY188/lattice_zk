use std::path::Path;

use circuit::Circuit;
use config_macros::declare_gkr_config;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use gkr::Prover;
use gkr_engine::{
    BN254Config, ExpanderPCS, FieldEngine, GKREngine, GKRScheme, M31x16Config, MPIConfig,
    StructuredReferenceString,
};
use gkr_hashers::SHA256hasher;
use poly_commit::{expander_pcs_init_testing_only, raw::RawExpanderGKR};
use std::hint::black_box;
use transcript::BytesHashTranscript;

fn prover_run<Cfg: GKREngine>(
    mpi_config: &MPIConfig,
    circuit: &mut Circuit<Cfg::FieldConfig>,
    pcs_params: &<Cfg::PCSConfig as ExpanderPCS<Cfg::FieldConfig>>::Params,
    pcs_proving_key: &<<Cfg::PCSConfig as ExpanderPCS<Cfg::FieldConfig>>::SRS as StructuredReferenceString>::PKey,
    pcs_scratch: &mut <Cfg::PCSConfig as ExpanderPCS<Cfg::FieldConfig>>::ScratchPad,
) where
    Cfg::FieldConfig: FieldEngine,
{
    let mut prover = Prover::<Cfg>::new(mpi_config.clone());
    prover.prepare_mem(circuit);
    prover.prove(circuit, pcs_params, pcs_proving_key, pcs_scratch);
}

fn benchmark_setup<Cfg: GKREngine>(
    circuit_file: &str,
    witness_file: Option<&str>,
) -> (
    MPIConfig<'static>,
    Circuit<Cfg::FieldConfig>,
    <Cfg::PCSConfig as ExpanderPCS<Cfg::FieldConfig>>::Params,
    <<Cfg::PCSConfig as ExpanderPCS<Cfg::FieldConfig>>::SRS as StructuredReferenceString>::PKey,
    <Cfg::PCSConfig as ExpanderPCS<Cfg::FieldConfig>>::ScratchPad,
) {
    let mpi_config = MPIConfig::prover_new(None, None);
    let mut circuit =
        Circuit::<Cfg::FieldConfig>::single_thread_prover_load_circuit::<Cfg>(circuit_file);

    if let Some(witness_file) = witness_file {
        circuit.prover_load_witness_file(witness_file, &mpi_config);
    } else {
        circuit.set_random_input_for_test();
    }

    let (pcs_params, pcs_proving_key, _pcs_verification_key, pcs_scratch) =
        expander_pcs_init_testing_only::<Cfg::FieldConfig, Cfg::PCSConfig>(
            circuit.log_input_size(),
            &mpi_config,
        );

    (
        mpi_config,
        circuit,
        pcs_params,
        pcs_proving_key,
        pcs_scratch,
    )
}

/// Resolve path to workspace data dir (works from any cwd when running `cargo bench -p gkr`).
fn workspace_data_path(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn criterion_gkr_keccak(c: &mut Criterion) {
    declare_gkr_config!(
        M31x16RawConfig,
        FieldType::M31x16,
        FiatShamirHashType::SHA256,
        PCSCommitmentType::Raw,
        GKRScheme::Vanilla
    );
    declare_gkr_config!(
        M31x16LatticeConfig,
        FieldType::M31x16,
        FiatShamirHashType::SHA256,
        PCSCommitmentType::Lattice,
        GKRScheme::Vanilla
    );
    declare_gkr_config!(
        BN254ConfigSha2,
        FieldType::BN254,
        FiatShamirHashType::SHA256,
        PCSCommitmentType::Raw,
        GKRScheme::Vanilla
    );

    let circuit_m31 = workspace_data_path("circuit_m31.txt");
    let witness_m31 = workspace_data_path("witness_m31.txt");
    let circuit_bn254 = workspace_data_path("circuit_bn254.txt");
    let witness_bn254 = workspace_data_path("witness_bn254.txt");

    let (m31_raw_config, mut m31_raw_circuit, m31_raw_pcs_params, m31_raw_pcs_proving_key, mut m31_raw_pcs_scratch) =
        benchmark_setup::<M31x16RawConfig>(&circuit_m31, Some(&witness_m31));
    let (m31_lattice_config, mut m31_lattice_circuit, m31_lattice_pcs_params, m31_lattice_pcs_proving_key, mut m31_lattice_pcs_scratch) =
        benchmark_setup::<M31x16LatticeConfig>(&circuit_m31, Some(&witness_m31));
    let (
        bn254_config,
        mut bn254_circuit,
        bn254_pcs_params,
        bn254_pcs_proving_key,
        mut bn254_pcs_scratch,
    ) = benchmark_setup::<BN254ConfigSha2>(&circuit_bn254, Some(&witness_bn254));

    let num_keccak_m31 = 2 * <M31x16RawConfig as GKREngine>::FieldConfig::get_field_pack_size();
    let num_keccak_bn254 = 2 * <BN254ConfigSha2 as GKREngine>::FieldConfig::get_field_pack_size();

    let mut group_pcs = c.benchmark_group("GKR proving M31x16 by PCS (32 keccak/proof)");
    group_pcs.measurement_time(std::time::Duration::from_secs(30));
    group_pcs.sample_size(100);
    group_pcs.bench_function(BenchmarkId::new("Raw", 0), |b| {
        b.iter(|| {
            prover_run::<M31x16RawConfig>(
                &m31_raw_config,
                &mut m31_raw_circuit,
                &m31_raw_pcs_params,
                &m31_raw_pcs_proving_key,
                &mut m31_raw_pcs_scratch,
            );
            black_box(());
        })
    });
    group_pcs.bench_function(BenchmarkId::new("Lattice", 0), |b| {
        b.iter(|| {
            prover_run::<M31x16LatticeConfig>(
                &m31_lattice_config,
                &mut m31_lattice_circuit,
                &m31_lattice_pcs_params,
                &m31_lattice_pcs_proving_key,
                &mut m31_lattice_pcs_scratch,
            );
            black_box(());
        })
    });
    group_pcs.finish();

    let mut group = c.benchmark_group("single thread proving keccak by GKR vanilla");
    group.bench_function(
        BenchmarkId::new(
            format!(
                "Over M31 (Raw), with {} keccak instances per proof",
                num_keccak_m31
            ),
            0,
        ),
        |b| {
            b.iter(|| {
                {
                    prover_run::<M31x16RawConfig>(
                        &m31_raw_config,
                        &mut m31_raw_circuit,
                        &m31_raw_pcs_params,
                        &m31_raw_pcs_proving_key,
                        &mut m31_raw_pcs_scratch,
                    );
                    black_box(())
                };
            })
        },
    );

    group.bench_function(
        BenchmarkId::new(
            format!(
                "Over BN254, with {} keccak instances per proof",
                num_keccak_bn254
            ),
            0,
        ),
        |b| {
            b.iter(|| {
                {
                    prover_run::<BN254ConfigSha2>(
                        &bn254_config,
                        &mut bn254_circuit,
                        &bn254_pcs_params,
                        &bn254_pcs_proving_key,
                        &mut bn254_pcs_scratch,
                    );
                    black_box(())
                };
            })
        },
    );
}

criterion_group!(benches, criterion_gkr_keccak);
criterion_main!(benches);
