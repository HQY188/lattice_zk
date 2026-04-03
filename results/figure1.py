"""
柱状图：Prover Time (Lattiswift vs Libra)
数据为内嵌列表（与 FIELDS 顺序一致：M31x16, BabyBearx16）；更新时直接改下方常量。
"""
from __future__ import annotations

import os

import matplotlib.pyplot as plt
import numpy as np

RESULTS_DIR = os.path.dirname(os.path.abspath(__file__))

FIELDS = ["M31x16", "BabyBearx16"]

# Prover time (μs), order: M31x16, BabyBearx16
PROVER_TIME_LATTISWIFT = [329_290, 366_132]
PROVER_TIME_LIBRA = [272_295, 354_011]
PROVER_TIME_LATTISWIFT_PARA = [136_479, 154_940]


def load_all_series() -> dict[str, list[int]]:
    return {
        "Lattiswift": PROVER_TIME_LATTISWIFT,
        "Libra": PROVER_TIME_LIBRA,
        "Lattiswift_para": PROVER_TIME_LATTISWIFT_PARA,
    }


def plot(out_path: str | None = None) -> None:
    plt.rcParams["font.size"] = 12
    series = load_all_series()
    x = np.arange(len(FIELDS))
    width = 0.24
    colors = ["#2ca02c", "#1f77b4", "#d62728"]

    fig, ax = plt.subplots(figsize=(7.5, 4.8))
    offsets = [-width, 0.0, width]
    labels = ["Lattiswift", "Libra", "Lattiswift(8 threads)"]

    for off, label, color, values in zip(offsets, labels, colors, series.values()):
        ax.bar(x + off, values, width, label=label, color=color, edgecolor="0.2", linewidth=0.6)

    ax.set_ylabel(r"Prover Time ($\mu\mathrm{s}$)")
    ax.set_title("Prover Time (Lattiswift vs Libra)")
    ax.set_xticks(x)
    ax.set_xticklabels([f"Field Type: {f}" for f in FIELDS])
    ax.set_xlim(x[0] - 0.55, x[-1] + 0.55)
    ymax = max(v for vals in series.values() for v in vals)
    ax.set_ylim(0, ymax * 1.08)

    ax.legend(
        loc="upper center",
        bbox_to_anchor=(0.5, -0.14),
        ncol=3,
        frameon=True,
    )
    fig.tight_layout()
    fig.subplots_adjust(bottom=0.22)

    if out_path is None:
        out_path = os.path.join(RESULTS_DIR, "figure1.pdf")
    os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
    fig.savefig(out_path, bbox_inches="tight")
    png_path = os.path.splitext(out_path)[0] + ".png"
    fig.savefig(png_path, bbox_inches="tight", dpi=150)
    plt.close(fig)


def main() -> None:
    plot()


if __name__ == "__main__":
    main()
