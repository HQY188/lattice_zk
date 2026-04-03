"""
柱状图：Proof Size (Lattiswift vs Libra)
数据为内嵌列表（与 FIELDS 顺序一致：M31x16, BabyBearx16）；更新时直接改下方常量。
"""
from __future__ import annotations

import os

import matplotlib.pyplot as plt
import numpy as np

RESULTS_DIR = os.path.dirname(os.path.abspath(__file__))

FIELDS = ["M31x16", "BabyBearx16"]

# Proof size (bytes), order: M31x16, BabyBearx16
PROOF_SIZE_LATTISWIFT = [440_793, 440_793]
PROOF_SIZE_LIBRA = [224_844, 224_844]

COLOR_LATTISWIFT = "#2ca02c"
COLOR_LIBRA = "#1f77b4"


def plot(out_path: str | None = None) -> None:
    plt.rcParams["font.size"] = 12
    lat_vals = PROOF_SIZE_LATTISWIFT
    lib_vals = PROOF_SIZE_LIBRA

    x = np.arange(len(FIELDS))
    bar_w = 0.32
    gap = 0.04

    fig, ax = plt.subplots(figsize=(7.5, 4.8))
    ax.bar(
        x - bar_w / 2 - gap / 2,
        lat_vals,
        bar_w,
        label="Lattiswift",
        color=COLOR_LATTISWIFT,
        edgecolor="0.2",
        linewidth=0.6,
    )
    ax.bar(
        x + bar_w / 2 + gap / 2,
        lib_vals,
        bar_w,
        label="Libra",
        color=COLOR_LIBRA,
        edgecolor="0.2",
        linewidth=0.6,
    )

    ax.set_ylabel("Proof Size (byte)")
    ax.set_title("Proof Size(Lattiswift vs Libra)")
    ax.set_xticks(x)
    ax.set_xticklabels([f"Field Type: {f}" for f in FIELDS])
    ax.set_xlim(x[0] - 0.55, x[-1] + 0.55)
    ymax = max(max(lat_vals), max(lib_vals))
    ax.set_ylim(0, ymax * 1.08)

    ax.legend(
        loc="upper center",
        bbox_to_anchor=(0.5, -0.14),
        ncol=2,
        frameon=True,
    )
    fig.tight_layout()
    fig.subplots_adjust(bottom=0.22)

    if out_path is None:
        out_path = os.path.join(RESULTS_DIR, "figure2.pdf")
    os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
    fig.savefig(out_path, bbox_inches="tight")
    png_path = os.path.splitext(out_path)[0] + ".png"
    fig.savefig(png_path, bbox_inches="tight", dpi=150)
    plt.close(fig)


def main() -> None:
    plot()


if __name__ == "__main__":
    main()
