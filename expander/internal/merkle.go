package internal

import (
	"fmt"
	"math/big"

	"github.com/consensys/gnark/frontend"
)

// MerkleInclusionCircuit verifies a leaf opens to Root using toy compress H(a,b)=a²+b mod p.
// PathBits[i]==0 means cur is left child at level i; 1 means cur is right child.
type MerkleInclusionCircuit struct {
	Leaf     frontend.Variable
	Siblings []frontend.Variable
	PathBits []frontend.Variable
	Root     frontend.Variable `gnark:",public"`
}

// NewMerkleInclusionCircuit builds a template with path length `depth`.
func NewMerkleInclusionCircuit(depth int) *MerkleInclusionCircuit {
	if depth < 1 {
		panic("merkle: depth >= 1")
	}
	return &MerkleInclusionCircuit{
		Siblings: make([]frontend.Variable, depth),
		PathBits: make([]frontend.Variable, depth),
	}
}

func toyCompress(api frontend.API, a, b frontend.Variable) frontend.Variable {
	aa := api.Mul(a, a)
	return api.Add(aa, b)
}

// Define walks the Merkle path.
func (c *MerkleInclusionCircuit) Define(api frontend.API) error {
	if len(c.Siblings) != len(c.PathBits) {
		return fmt.Errorf("merkle: siblings/path bits length mismatch")
	}
	cur := c.Leaf
	for i := range c.Siblings {
		api.AssertIsBoolean(c.PathBits[i])
		left := api.Select(c.PathBits[i], c.Siblings[i], cur)
		right := api.Select(c.PathBits[i], cur, c.Siblings[i])
		cur = toyCompress(api, left, right)
	}
	api.AssertIsEqual(cur, c.Root)
	return nil
}

// MerkleAssignment builds leaves, tree, and a satisfying assignment for leafIndex.
func MerkleAssignment(depth, leafIndex int, mod *big.Int) (*MerkleInclusionCircuit, error) {
	if depth < 1 {
		return nil, fmt.Errorf("depth must be >= 1")
	}
	n := 1 << depth
	if leafIndex < 0 || leafIndex >= n {
		return nil, fmt.Errorf("leafIndex must be in [0, 2^depth)")
	}
	if mod == nil || mod.Sign() <= 0 {
		return nil, fmt.Errorf("invalid modulus")
	}

	leaves := make([]*big.Int, n)
	for i := 0; i < n; i++ {
		v := big.NewInt(int64(1000 + i))
		v.Mod(v, mod)
		leaves[i] = v
	}

	levels := make([][]*big.Int, depth+1)
	levels[0] = leaves
	for l := 0; l < depth; l++ {
		prev := levels[l]
		if len(prev)%2 != 0 {
			return nil, fmt.Errorf("internal merkle level size")
		}
		next := make([]*big.Int, len(prev)/2)
		for j := 0; j < len(prev); j += 2 {
			next[j/2] = toyHashBig(prev[j], prev[j+1], mod)
		}
		levels[l+1] = next
	}
	root := new(big.Int).Set(levels[depth][0])

	siblings := make([]*big.Int, depth)
	pathBits := make([]*big.Int, depth)
	idx := leafIndex
	for l := 0; l < depth; l++ {
		sibIdx := idx ^ 1
		siblings[l] = new(big.Int).Set(levels[l][sibIdx])
		bit := idx % 2
		pathBits[l] = big.NewInt(int64(bit))
		idx /= 2
	}

	leaf := new(big.Int).Set(levels[0][leafIndex])
	c := NewMerkleInclusionCircuit(depth)
	c.Leaf = leaf
	c.Root = root
	for i := 0; i < depth; i++ {
		c.Siblings[i] = siblings[i]
		c.PathBits[i] = pathBits[i]
	}
	return c, nil
}

func toyHashBig(a, b, mod *big.Int) *big.Int {
	t := new(big.Int).Mul(a, a)
	t.Mod(t, mod)
	t.Add(t, b)
	t.Mod(t, mod)
	return t
}
