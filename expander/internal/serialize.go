package internal

import (
	"fmt"
	"math/big"
	"os"

	"github.com/PolyhedraZK/ExpanderCompilerCollection/ecgo"
	"github.com/PolyhedraZK/ExpanderCompilerCollection/ecgo/irwg"
	"github.com/PolyhedraZK/ExpanderCompilerCollection/ecgo/test"
	"github.com/consensys/gnark/frontend"
)

// CompileWrite compiles a gnark circuit with ecgo, solves witness, optionally checks, writes .txt files.
func CompileWrite(field FieldKind, circuit, assignment frontend.Circuit, outCircuit, outWitness string, skipCheck bool) error {
	mod := field.FieldModulus()
	compiled, err := ecgo.Compile(mod, circuit)
	if err != nil {
		return fmt.Errorf("ecgo.Compile: %w", err)
	}

	solver := compiled.GetInputSolver()
	unit, err := solver.SolveInput(assignment, 0)
	if err != nil {
		return fmt.Errorf("SolveInput: %w", err)
	}

	packed, err := expandWitnessSIMD(field, unit)
	if err != nil {
		return err
	}

	layered := compiled.GetLayeredCircuit()
	if !skipCheck {
		checks := test.CheckCircuitMultiWitness(layered, packed)
		for i, ok := range checks {
			if !ok {
				return fmt.Errorf("CheckCircuitMultiWitness failed at witness index %d", i)
			}
		}
	}

	if err := os.WriteFile(outCircuit, layered.Serialize(), 0o644); err != nil {
		return fmt.Errorf("write circuit: %w", err)
	}
	if err := os.WriteFile(outWitness, packed.Serialize(), 0o644); err != nil {
		return fmt.Errorf("write witness: %w", err)
	}
	return nil
}

func expandWitnessSIMD(field FieldKind, unit *irwg.Witness) (*irwg.Witness, error) {
	if unit == nil {
		return nil, fmt.Errorf("nil witness")
	}
	k := field.SIMDPackSize()
	if unit.NumWitnesses != 1 {
		return nil, fmt.Errorf("expected NumWitnesses==1, got %d", unit.NumWitnesses)
	}
	values := make([]*big.Int, k*len(unit.Values))
	for rep := 0; rep < k; rep++ {
		copy(values[rep*len(unit.Values):(rep+1)*len(unit.Values)], unit.Values)
	}
	return &irwg.Witness{
		NumWitnesses:              k,
		NumInputsPerWitness:       unit.NumInputsPerWitness,
		NumPublicInputsPerWitness: unit.NumPublicInputsPerWitness,
		Field:                     unit.Field,
		Values:                    values,
	}, nil
}
