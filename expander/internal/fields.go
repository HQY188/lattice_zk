package internal

import (
	"fmt"
	"math/big"

	eccFields "github.com/PolyhedraZK/ExpanderCompilerCollection/ecgo/field"
)

// FieldKind matches recursion/modules/fields ECCFieldEnum values (ecgo alignment).
type FieldKind uint64

const (
	FieldM31 FieldKind = 1
	FieldBN254 FieldKind = 2
	FieldGF2 FieldKind = 3
)

func ParseField(s string) (FieldKind, error) {
	switch s {
	case "m31", "M31":
		return FieldM31, nil
	case "bn254", "BN254":
		return FieldBN254, nil
	case "gf2", "GF2", "gf2ext128":
		return FieldGF2, nil
	default:
		return 0, fmt.Errorf("unknown field %q (use m31, bn254, gf2)", s)
	}
}

func (f FieldKind) FieldModulus() *big.Int {
	return f.GetFieldEngine().Field()
}

func (f FieldKind) GetFieldEngine() eccFields.Field {
	return eccFields.GetFieldById(uint64(f))
}

// SIMDPackSize is the witness replication count for Expander (see recursion/modules/fields).
func (f FieldKind) SIMDPackSize() int {
	switch f {
	case FieldBN254:
		return 1
	case FieldM31:
		return 16
	case FieldGF2:
		return 8
	default:
		panic("internal: invalid field")
	}
}
