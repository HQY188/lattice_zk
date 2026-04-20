package internal

import (
	"fmt"
	"math"
	"math/big"
	"sort"

	"github.com/consensys/gnark/frontend"
)

// LanczosResample1DCircuit encodes classic 1D Lanczos resampling (windowed sinc, cf. "Filters for common
// resampling tasks"): each low-res sample Out[j] is a fixed integer-weight sum of high-res samples In[i].
//
// Constraints (mod field, for each output j):  sum_t w_{j,t} * In[i_{j,t}] = Out[j] * Scale
// where public Scale is a fixed-point denominator and public Weight holds all w_{j,t} (normalized per j to sum Scale).
//
// This is 1D (one scanline). Separable 2D image scaling is row pass + column pass with the same kernel.
type LanczosResample1DCircuit struct {
	In     []frontend.Variable
	Out    []frontend.Variable
	Weight []frontend.Variable `gnark:",public"`
	Scale  frontend.Variable   `gnark:",public"`

	TapIn     []int `gnark:"-"` // len == len(Weight)
	OutTapBeg []int `gnark:"-"` // len == len(Out)+1, prefix into TapIn/Weight
}

// NewLanczosResample1D builds tap layout and public weights for Lanczos-a resampling from length nin to nout.
// scaleInt is the integer denominator (e.g. 1_000_000): for each j, integer weights sum to scaleInt exactly.
func NewLanczosResample1D(nin, nout, a int, scaleInt int64) (*LanczosResample1DCircuit, []int64, error) {
	if nin < 1 || nout < 1 {
		return nil, nil, fmt.Errorf("lanczos resample: nin, nout must be >= 1")
	}
	if a < 1 {
		return nil, nil, fmt.Errorf("lanczos resample: order a must be >= 1 (use 2 or 3 for classic Lanczos-2 / Lanczos-3)")
	}
	if scaleInt < 1 {
		return nil, nil, fmt.Errorf("lanczos resample: scale must be >= 1")
	}
	tapIn, outTapBeg, wInt, err := computeLanczos1DTaps(nin, nout, a, scaleInt)
	if err != nil {
		return nil, nil, err
	}
	nTaps := len(tapIn)
	if nTaps != len(wInt) || len(outTapBeg) != nout+1 || outTapBeg[nout] != nTaps {
		return nil, nil, fmt.Errorf("lanczos resample: internal tap layout error")
	}
	return &LanczosResample1DCircuit{
		In:        make([]frontend.Variable, nin),
		Out:       make([]frontend.Variable, nout),
		Weight:    make([]frontend.Variable, nTaps),
		TapIn:     tapIn,
		OutTapBeg: outTapBeg,
	}, wInt, nil
}

// Define checks Out[j] * Scale = sum of Weight[t] * In[TapIn[t]] over taps belonging to j.
func (c *LanczosResample1DCircuit) Define(api frontend.API) error {
	nout := len(c.Out)
	if len(c.OutTapBeg) != nout+1 {
		return fmt.Errorf("lanczos resample: OutTapBeg length mismatch")
	}
	if len(c.Weight) != len(c.TapIn) {
		return fmt.Errorf("lanczos resample: Weight/TapIn length mismatch")
	}
	last := c.OutTapBeg[nout]
	if last != len(c.Weight) {
		return fmt.Errorf("lanczos resample: OutTapBeg[last] != num taps")
	}
	for j := 0; j < nout; j++ {
		beg, end := c.OutTapBeg[j], c.OutTapBeg[j+1]
		if beg > end || end > len(c.Weight) {
			return fmt.Errorf("lanczos resample: bad tap range for out %d", j)
		}
		var acc frontend.Variable
		for ti := beg; ti < end; ti++ {
			idx := c.TapIn[ti]
			if idx < 0 || idx >= len(c.In) {
				return fmt.Errorf("lanczos resample: tap index out of range")
			}
			term := api.Mul(c.In[idx], c.Weight[ti])
			if ti == beg {
				acc = term
			} else {
				acc = api.Add(acc, term)
			}
		}
		api.AssertIsEqual(acc, api.Mul(c.Out[j], c.Scale))
	}
	return nil
}

// LanczosResample1DZeroAssignment sets In[i]=0, Out[j]=0 and fills public Weight/Scale so the sums hold (trivial solution).
func LanczosResample1DZeroAssignment(tpl *LanczosResample1DCircuit, wInt []int64, scaleInt int64, mod *big.Int) (*LanczosResample1DCircuit, error) {
	if tpl == nil || mod == nil || mod.Sign() <= 0 {
		return nil, fmt.Errorf("lanczos resample: invalid args")
	}
	nin, nout := len(tpl.In), len(tpl.Out)
	if len(tpl.Weight) != len(wInt) {
		return nil, fmt.Errorf("lanczos resample: template size mismatch")
	}
	circ := *tpl
	circ.In = make([]frontend.Variable, nin)
	circ.Out = make([]frontend.Variable, nout)
	circ.Weight = make([]frontend.Variable, len(wInt))
	for i := 0; i < nin; i++ {
		circ.In[i] = big.NewInt(0)
	}
	for j := 0; j < nout; j++ {
		circ.Out[j] = big.NewInt(0)
	}
	for t := range wInt {
		v := big.NewInt(wInt[t])
		v.Mod(v, mod)
		circ.Weight[t] = v
	}
	sc := big.NewInt(scaleInt)
	sc.Mod(sc, mod)
	circ.Scale = sc
	return &circ, nil
}

func sincPi(x float64) float64 {
	if math.Abs(x) < 1e-15 {
		return 1
	}
	return math.Sin(math.Pi*x) / (math.Pi * x)
}

// lanczosWindow returns L_a(delta) = sinc(delta) * sinc(delta/a) for |delta| < a, else 0 (continuous Lanczos kernel).
func lanczosWindow(delta float64, a float64) float64 {
	if delta <= -a || delta >= a {
		return 0
	}
	return sincPi(delta) * sincPi(delta/a)
}

func computeLanczos1DTaps(nin, nout, a int, scaleInt int64) (tapIn []int, outTapBeg []int, wInt []int64, err error) {
	af := float64(a)
	outTapBeg = make([]int, nout+1)
	var tapsFlat []struct {
		outJ int
		inI  int
		w    int64
	}

	for j := 0; j < nout; j++ {
		outTapBeg[j] = len(tapsFlat)
		srcX := (float64(j)+0.5)*float64(nin)/float64(nout) - 0.5
		lo := int(math.Ceil(srcX - af))
		hi := int(math.Floor(srcX + af))
		if lo < 0 {
			lo = 0
		}
		if hi >= nin {
			hi = nin - 1
		}
		type cw struct {
			i int
			w float64
		}
		var cand []cw
		sum := 0.0
		for i := lo; i <= hi; i++ {
			delta := srcX - float64(i)
			w := lanczosWindow(delta, af)
			if w == 0 || math.Abs(w) < 1e-18 {
				continue
			}
			cand = append(cand, cw{i: i, w: w})
			sum += w
		}
		if len(cand) == 0 {
			return nil, nil, nil, fmt.Errorf("lanczos resample: empty tap set for output %d (nin=%d nout=%d a=%d)", j, nin, nout, a)
		}
		// Integer weights summing exactly to scaleInt: round then fix drift on the largest tap.
		type part struct {
			i   int
			wt  int64
			rem float64 // fractional mass after rounding, for tie-break
		}
		parts := make([]part, len(cand))
		scaleF := float64(scaleInt)
		var floors int64
		for k := range cand {
			exact := cand[k].w / sum * scaleF
			rnd := int64(math.Round(exact))
			if rnd < 0 {
				rnd = 0
			}
			parts[k].i = cand[k].i
			parts[k].wt = rnd
			parts[k].rem = exact - float64(rnd)
			floors += rnd
		}
		diff := scaleInt - floors
		if len(parts) > 0 {
			sort.Slice(parts, func(i, j int) bool {
				if parts[i].rem != parts[j].rem {
					return parts[i].rem > parts[j].rem
				}
				return parts[i].wt > parts[j].wt
			})
			parts[0].wt += diff
		} else if diff != 0 {
			return nil, nil, nil, fmt.Errorf("lanczos resample: no taps but nonzero diff at output %d", j)
		}
		for k := range parts {
			if parts[k].wt < 0 {
				return nil, nil, nil, fmt.Errorf("lanczos resample: negative weight at output %d", j)
			}
			tapsFlat = append(tapsFlat, struct {
				outJ int
				inI  int
				w    int64
			}{outJ: j, inI: parts[k].i, w: parts[k].wt})
		}
	}
	outTapBeg[nout] = len(tapsFlat)

	tapIn = make([]int, len(tapsFlat))
	wInt = make([]int64, len(tapsFlat))
	for t := range tapsFlat {
		tapIn[t] = tapsFlat[t].inI
		wInt[t] = tapsFlat[t].w
	}
	return tapIn, outTapBeg, wInt, nil
}
