package internal

import (
	"fmt"
	"math/big"

	"github.com/consensys/gnark/frontend"
)

// MatMulCircuit verifies C = A*B over n×n matrices (flat row-major A, B, C of length n²).
type MatMulCircuit struct {
	A []frontend.Variable
	B []frontend.Variable
	C []frontend.Variable
}

// NewMatMulCircuit allocates variables for n×n matrices.
func NewMatMulCircuit(n int) *MatMulCircuit {
	if n < 1 {
		panic("matmul: n >= 1")
	}
	nn := n * n
	return &MatMulCircuit{
		A: make([]frontend.Variable, nn),
		B: make([]frontend.Variable, nn),
		C: make([]frontend.Variable, nn),
	}
}

func matMulIndex(n, i, j int) int {
	return i*n + j
}

// Define checks C[i,j] = sum_k A[i,k]*B[k,j].
func (c *MatMulCircuit) Define(api frontend.API) error {
	n2 := len(c.A)
	if len(c.B) != n2 || len(c.C) != n2 {
		return fmt.Errorf("matmul: A,B,C length mismatch")
	}
	n, err := intSqrtDim(n2)
	if err != nil {
		return err
	}
	for i := 0; i < n; i++ {
		for j := 0; j < n; j++ {
			var sum frontend.Variable
			for k := 0; k < n; k++ {
				term := api.Mul(c.A[matMulIndex(n, i, k)], c.B[matMulIndex(n, k, j)])
				if k == 0 {
					sum = term
				} else {
					sum = api.Add(sum, term)
				}
			}
			api.AssertIsEqual(sum, c.C[matMulIndex(n, i, j)])
		}
	}
	return nil
}

func intSqrtDim(n2 int) (int, error) {
	if n2 < 1 {
		return 0, fmt.Errorf("invalid flat len %d", n2)
	}
	n := 0
	for n*n < n2 {
		n++
	}
	if n*n != n2 {
		return 0, fmt.Errorf("flat len %d is not n²", n2)
	}
	return n, nil
}

// MatMulAssignment builds a satisfying witness: random-ish small entries mod `mod`.
func MatMulAssignment(n int, mod *big.Int, seed int64) (*MatMulCircuit, error) {
	if mod == nil || mod.Sign() <= 0 {
		return nil, fmt.Errorf("invalid modulus")
	}
	a := randMatrixBig(n, mod, seed)
	b := randMatrixBig(n, mod, seed+1)
	c := mulMatrixMod(n, mod, a, b)
	circ := NewMatMulCircuit(n)
	for i := 0; i < n; i++ {
		for j := 0; j < n; j++ {
			circ.A[matMulIndex(n, i, j)] = a[i][j]
			circ.B[matMulIndex(n, i, j)] = b[i][j]
			circ.C[matMulIndex(n, i, j)] = c[i][j]
		}
	}
	return circ, nil
}

func randMatrixBig(n int, mod *big.Int, seed int64) [][]*big.Int {
	x := seed
	if x == 0 {
		x = 1
	}
	out := make([][]*big.Int, n)
	for i := 0; i < n; i++ {
		out[i] = make([]*big.Int, n)
		for j := 0; j < n; j++ {
			v := (x + int64(i)*7 + int64(j)*11) % 97
			if v < 0 {
				v = -v
			}
			out[i][j] = big.NewInt(v)
			out[i][j].Mod(out[i][j], mod)
		}
	}
	return out
}

func mulMatrixMod(n int, mod *big.Int, a, b [][]*big.Int) [][]*big.Int {
	c := make([][]*big.Int, n)
	for i := 0; i < n; i++ {
		c[i] = make([]*big.Int, n)
		for j := 0; j < n; j++ {
			sum := big.NewInt(0)
			for k := 0; k < n; k++ {
				t := new(big.Int).Mul(a[i][k], b[k][j])
				sum.Add(sum, t)
				sum.Mod(sum, mod)
			}
			c[i][j] = sum
		}
	}
	return c
}
