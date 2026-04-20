package internal

import (
	"fmt"
	"math"
	"math/big"

	"github.com/consensys/gnark/frontend"
)

// PixelsInnerProductCircuit verifies Z = sum_i X[i]*Y[i] (mod field), one “pixel” product per term.
type PixelsInnerProductCircuit struct {
	X []frontend.Variable
	Y []frontend.Variable
	Z frontend.Variable `gnark:",public"`
}

// NewPixelsInnerProductCircuit allocates N pixel pairs.
func NewPixelsInnerProductCircuit(n int) *PixelsInnerProductCircuit {
	if n < 1 {
		panic("pixels: n >= 1")
	}
	return &PixelsInnerProductCircuit{
		X: make([]frontend.Variable, n),
		Y: make([]frontend.Variable, n),
	}
}

// Define accumulates the inner product.
func (c *PixelsInnerProductCircuit) Define(api frontend.API) error {
	if len(c.X) != len(c.Y) {
		return fmt.Errorf("pixels: X/Y length mismatch")
	}
	n := len(c.X)
	var acc frontend.Variable
	for i := 0; i < n; i++ {
		term := api.Mul(c.X[i], c.Y[i])
		if i == 0 {
			acc = term
		} else {
			acc = api.Add(acc, term)
		}
	}
	api.AssertIsEqual(acc, c.Z)
	return nil
}

// PixelsInnerProductAssignment fills X,Y with small values and sets Z to their dot product mod `mod`.
func PixelsInnerProductAssignment(n int, mod *big.Int, seed int64) (*PixelsInnerProductCircuit, error) {
	if n < 1 {
		return nil, fmt.Errorf("pixels: n >= 1")
	}
	if mod == nil || mod.Sign() <= 0 {
		return nil, fmt.Errorf("invalid modulus")
	}
	x := seed
	if x == 0 {
		x = 1
	}
	circ := NewPixelsInnerProductCircuit(n)
	sum := big.NewInt(0)
	for i := 0; i < n; i++ {
		vx := big.NewInt((x + int64(i)*5) % 199)
		vx.Mod(vx, mod)
		vy := big.NewInt((x + int64(i)*7) % 211)
		vy.Mod(vy, mod)
		circ.X[i] = vx
		circ.Y[i] = vy
		t := new(big.Int).Mul(vx, vy)
		t.Mod(t, mod)
		sum.Add(sum, t)
		sum.Mod(sum, mod)
	}
	circ.Z = sum
	return circ, nil
}

// DefaultPixelBenchmarkSizes returns N for 10^4, 10^4.5, 10^5, 10^5.5, 10^6 (half-integer exponents rounded to nearest int).
func DefaultPixelBenchmarkSizes() []int {
	return []int{
		10_000,
		int(math.Round(math.Pow(10, 4.5))),
		100_000,
		int(math.Round(math.Pow(10, 5.5))),
		1_000_000,
	}
}
