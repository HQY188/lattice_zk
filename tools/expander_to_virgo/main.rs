//! Expander 二进制电路 → Virgo `read_circuit` 可读文本（circuit + meta）的**尽力而为**转换器。
//!
//! 限制（见 `Virgo/data/README_CONVERTER.txt`）：
//! - 每层每个输出下标 `o_id` 至多一条门；否则无法压成 Virgo「每位置单门」格式。
//! - `GateMul` / `GateAdd` 的 `coef` 须为 1（与 Virgo 无标量系数乘/加一致）。
//! - 不支持 `GateUni`、不支持 `CoefType::Random`。
//! - 域与 Virgo 内部 `2^61-1` 不同；本工具只导出**拓扑与常量整数码**，不保证与 Expander 证明语义一致。

use arith::Field;
use circuit::{Circuit, CoefType, GateAdd, GateConst, GateMul, GateUni, RecursiveCircuit, Witness};
use gkr_engine::{BabyBearx16Config, FieldEngine, M31x16Config};
use serdes::ExpSerde;
use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

struct Args {
    input: PathBuf,
    out_dir: PathBuf,
    field: String,
    prefix: String,
    witness: Option<PathBuf>,
}

fn print_usage() {
    eprintln!(
        "用法: expander_to_virgo --input <circuit.bin> [--out-dir Virgo/data] [--field m31|babybear] [--prefix expander] [--witness witness.bin]"
    );
}

fn parse_args() -> Result<Args, String> {
    let mut input = None::<PathBuf>;
    let mut out_dir = PathBuf::from("Virgo/data");
    let mut field = String::new();
    let mut prefix = "expander".to_string();
    let mut witness = None::<PathBuf>;

    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--input" => {
                input = Some(PathBuf::from(it.next().ok_or("--input 需要路径")?));
            }
            "--out-dir" => {
                out_dir = PathBuf::from(it.next().ok_or("--out-dir 需要路径")?);
            }
            "--field" => {
                field = it.next().ok_or("--field 需要 m31 或 babybear")?;
            }
            "--prefix" => {
                prefix = it.next().ok_or("--prefix 需要字符串")?;
            }
            "--witness" => {
                witness = Some(PathBuf::from(it.next().ok_or("--witness 需要路径")?));
            }
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            _ => return Err(format!("未知参数: {a}")),
        }
    }

    let input = input.ok_or("缺少 --input")?;
    if field.is_empty() {
        return Err("缺少 --field（m31 或 babybear）".to_string());
    }

    Ok(Args {
        input,
        out_dir,
        field,
        prefix,
        witness,
    })
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            print_usage();
            std::process::exit(1);
        }
    };
    let field = args.field.to_lowercase();
    let r = match field.as_str() {
        "m31" => run::<M31x16Config>(&args),
        "babybear" => run::<BabyBearx16Config>(&args),
        _ => {
            eprintln!("未知 --field，请使用 m31 或 babybear");
            std::process::exit(1);
        }
    };
    if let Err(e) = r {
        eprintln!("转换失败: {e}");
        std::process::exit(1);
    }
}

fn run<C: FieldEngine>(args: &Args) -> Result<(), String> {
    let rc = RecursiveCircuit::<C>::load(args.input.to_str().ok_or("路径无效")?)
        .map_err(|e| format!("加载电路失败: {e}"))?;
    let circuit = rc.flatten();

    fs::create_dir_all(&args.out_dir)
        .map_err(|e| format!("创建输出目录失败: {e}"))?;

    let witness_parsed: Option<Witness<C>> = if let Some(w) = &args.witness {
        let bytes = fs::read(w).map_err(|e| format!("读取 witness 失败: {e}"))?;
        let wit = Witness::<C>::deserialize_from(Cursor::new(&bytes))
            .map_err(|e| format!("反序列化 witness 失败: {e}"))?;
        Some(wit)
    } else {
        None
    };

    let witness_vals: Option<&[C::CircuitField]> =
        witness_parsed.as_ref().map(|w| w.values.as_slice());
    let private_n = 1usize << circuit.log_input_size();
    let num_pub = witness_parsed
        .as_ref()
        .map(|w| w.num_public_inputs_per_witness)
        .unwrap_or(rc.num_public_inputs);

    let d = virgo_d(&circuit);
    let mut circuit_txt = String::new();
    circuit_txt.push_str(&format!("{}\n", d));

    // i = 1：输入层 + relay（read_circuit 特殊分支）
    write_first_block::<C>(
        &circuit,
        witness_vals,
        private_n,
        d,
        &mut circuit_txt,
    )?;

    // i = 2..=d：对应 Expander layers[0..]
    for exp_li in 0..circuit.layers.len() {
        write_compute_block::<C>(
            &circuit,
            exp_li,
            witness_vals,
            private_n,
            num_pub,
            &mut circuit_txt,
        )?;
    }

    let meta_txt = build_meta(d);

    let cpath = args
        .out_dir
        .join(format!("{}_circuit.txt", args.prefix));
    let mpath = args.out_dir.join(format!("{}_meta.txt", args.prefix));

    fs::write(&cpath, &circuit_txt).map_err(|e| format!("写入 {:?}: {e}", cpath))?;
    fs::write(&mpath, &meta_txt).map_err(|e| format!("写入 {:?}: {e}", mpath))?;

    eprintln!(
        "已写入:\n  {}\n  {}",
        cpath.display(),
        mpath.display()
    );
    Ok(())
}

fn virgo_d<C: FieldEngine>(circuit: &Circuit<C>) -> usize {
    // 文件中的层块数：1（输入+relay）+ 每层 Expander 计算层
    1 + circuit.layers.len()
}

fn pad_requirement_for_first_layer(d: usize) -> usize {
    if d > 3 {
        17
    } else {
        15
    }
}

fn write_first_block<C: FieldEngine>(
    circuit: &Circuit<C>,
    witness_vals: Option<&[C::CircuitField]>,
    private_n: usize,
    d: usize,
    out: &mut String,
) -> Result<(), String> {
    let in_bits = circuit.layers[0].input_var_num;
    let n = 1usize << in_bits;
    if n != private_n {
        return Err(format!(
            "内部不一致: 第一层输入规模 {} 与 log_input_size 推出 {} 不一致",
            n, private_n
        ));
    }
    let pad_req = pad_requirement_for_first_layer(d);
    let min_pad_n = 1usize << pad_req;
    if n > min_pad_n {
        return Err(format!(
            "第一层输入线数 {} 超过 Virgo 首层 padding 上界 2^{}（请减小电路或改工具）",
            n, pad_req
        ));
    }

    out.push_str(&format!("{}\n", n));

    for g in 0..n {
        let u = if let Some(vals) = witness_vals {
            if vals.len() < private_n {
                return Err(format!(
                    "witness 过短: 至少需要 {} 个私钥标量（lane0），实际 {}",
                    private_n,
                    vals.len()
                ));
            }
            i64::from(vals[g].as_u32_unchecked())
        } else {
            0i64
        };
        out.push_str(&format!("{} {} {} {}\n", 3i32, g, u, 0i64));
    }
    Ok(())
}

fn write_compute_block<C: FieldEngine>(
    circuit: &Circuit<C>,
    exp_li: usize,
    witness_vals: Option<&[C::CircuitField]>,
    private_n: usize,
    num_pub: usize,
    out: &mut String,
) -> Result<(), String> {
    let layer = &circuit.layers[exp_li];
    let n = 1usize << layer.output_var_num;

    let mut buckets: HashMap<usize, Vec<ComputeGate<'_, C>>> = HashMap::new();
    for g in &layer.mul {
        buckets
            .entry(g.o_id)
            .or_default()
            .push(ComputeGate::Mul { g });
    }
    for g in &layer.add {
        buckets
            .entry(g.o_id)
            .or_default()
            .push(ComputeGate::Add { g });
    }
    for g in &layer.const_ {
        buckets
            .entry(g.o_id)
            .or_default()
            .push(ComputeGate::Const { g });
    }
    for g in &layer.uni {
        buckets
            .entry(g.o_id)
            .or_default()
            .push(ComputeGate::Uni { g });
    }

    out.push_str(&format!("{}\n", n));
    for g in 0..n {
        let (ty, gu, gv) = match buckets.get(&g) {
            None => (2i32, 0i64, 0i64),
            Some(v) => emit_or_merge_gates::<C>(v, witness_vals, private_n, num_pub, exp_li, g)?,
        };
        out.push_str(&format!("{} {} {} {}\n", ty, g, gu, gv));
    }
    Ok(())
}

enum ComputeGate<'a, C: FieldEngine> {
    Mul { g: &'a GateMul<C> },
    Add { g: &'a GateAdd<C> },
    Const { g: &'a GateConst<C> },
    Uni { g: &'a GateUni<C> },
}

/// Expander 允许多条门写到同一 `o_id`（累加）；Virgo 每层每个 `g` 只能有一条门。
/// 仅当恰好两条门且均为 `GateAdd`、系数为 1 时，可合并为一条 `ty=0`（prev[u]+prev[v]）。
fn emit_or_merge_gates<C: FieldEngine>(
    gates: &[ComputeGate<'_, C>],
    witness_vals: Option<&[C::CircuitField]>,
    private_n: usize,
    num_pub: usize,
    exp_li: usize,
    out_wire: usize,
) -> Result<(i32, i64, i64), String> {
    let one = C::CircuitField::ONE;
    match gates.len() {
        0 => Err(format!("内部错误: 输出线 {out_wire} 门列表为空")),
        1 => emit_one_gate::<C>(&gates[0], witness_vals, private_n, num_pub),
        2 => {
            let (a, b) = (&gates[0], &gates[1]);
            match (a, b) {
                (
                    ComputeGate::Add { g: ga },
                    ComputeGate::Add { g: gb },
                ) if ga.coef == one && gb.coef == one => {
                    Ok((0, ga.i_ids[0] as i64, gb.i_ids[0] as i64))
                }
                _ => Err(format!(
                    "层 {exp_li} 输出线 {out_wire} 上有 2 条门，且无法合并为单条 Virgo 门（仅支持「两条 GateAdd 且系数均为 1」→ ty=0）。\
                     Expander 常见为多条 mul/add 累加同一输出；完整 Keccak 电路通常无法单层映射到 Virgo。\
                     若需对比 Virgo zk_proof，请使用 Virgo 自带电路生成器；或实现多门拆层编译（本工具未包含）。"
                )),
            }
        }
        k => Err(format!(
            "层 {exp_li} 输出线 {out_wire} 上有 {k} 条门；Virgo 每层每位置仅一条门，且本工具仅合并「恰好两条 GateAdd(系数1)」。\
             请见 Virgo/data/README_CONVERTER.txt。"
        )),
    }
}

fn emit_one_gate<C: FieldEngine>(
    gate: &ComputeGate<'_, C>,
    witness_vals: Option<&[C::CircuitField]>,
    private_n: usize,
    num_pub: usize,
) -> Result<(i32, i64, i64), String> {
    let one = C::CircuitField::ONE;
    match gate {
        ComputeGate::Uni { g } => Err(format!(
            "不支持 GateUni（gate_type={}）",
            g.gate_type
        )),
        ComputeGate::Mul { g } => {
            if g.coef != one {
                return Err(format!(
                    "GateMul 系数须为 1 才能对应 Virgo 乘法门，当前 o_id={}",
                    g.o_id
                ));
            }
            Ok((1, g.i_ids[0] as i64, g.i_ids[1] as i64))
        }
        ComputeGate::Add { g } => {
            if g.coef != one {
                return Err(format!(
                    "GateAdd 系数须为 1 才能用 Virgo ty=0（prev[u]+prev[v]），假定 prev[0]=0，当前 o_id={}",
                    g.o_id
                ));
            }
            // Virgo add: prev[u] + prev[v]；用 v=0 表示第二项为「零线」
            Ok((0, g.i_ids[0] as i64, 0i64))
        }
        ComputeGate::Const { g } => match g.coef_type {
            CoefType::Random => Err("不支持 Random 系数门".to_string()),
            CoefType::Constant => {
                let u = i64::from(g.coef.as_u32_unchecked());
                Ok((3, u, 0))
            }
            CoefType::PublicInput(pi) => {
                let vals = witness_vals.ok_or("含 PublicInput 常数门但未提供 --witness".to_string())?;
                let idx = pi;
                if idx >= num_pub {
                    return Err(format!("PublicInput 索引 {} 越界（public 个数 {}）", idx, num_pub));
                }
                if vals.len() < private_n + num_pub {
                    return Err("witness 过短，无法取 public input".to_string());
                }
                let u = i64::from(vals[private_n + idx].as_u32_unchecked());
                Ok((3, u, 0))
            }
        },
    }
}

fn build_meta(d: usize) -> String {
    let mut s = String::new();
    for _ in 0..d {
        // 非并行层：is_para=0；占位参数与 Virgo 样本一致即可
        s.push_str("0 1 1 0 0\n");
    }
    s
}
