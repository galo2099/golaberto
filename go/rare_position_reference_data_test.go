package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

func TestGoldenReferenceDataAndPortableInputs(t *testing.T) {
	root := filepath.Join("..", "experiments", "rare_positions", "reference", "2026-09-30-hundredfold")
	data, err := os.ReadFile(filepath.Join(root, "reference.json"))
	if err != nil {
		t.Fatal(err)
	}
	var reference struct {
		Cases []struct {
			Group     int    `json:"group"`
			Request   string `json:"request"`
			SourceSHA string `json:"source_input_sha256"`
			Cells     []struct {
				Team          int       `json:"team"`
				Position      int       `json:"position"`
				Status        string    `json:"status"`
				Scorable      bool      `json:"scorable"`
				Probability   *float64  `json:"probability"`
				Confirmations int       `json:"independent_is_confirmations"`
				Range         []float64 `json:"is_empirical_range"`
				MC            struct {
					Samples int `json:"samples"`
					Hits    int `json:"hits"`
				} `json:"plain_mc"`
				Proof *struct {
					Impossible bool `json:"impossible"`
				} `json:"proof"`
			} `json:"cells"`
		} `json:"cases"`
	}
	if err := json.Unmarshal(data, &reference); err != nil {
		t.Fatal(err)
	}
	if len(reference.Cases) != 5 {
		t.Fatal("five reference snapshots required")
	}
	for _, c := range reference.Cases {
		inputData, err := os.ReadFile(filepath.Join(root, c.Request))
		if err != nil {
			t.Fatal(err)
		}
		var group GroupType
		if err := json.Unmarshal(inputData, &group); err != nil {
			t.Fatal(err)
		}
		if group.Id != c.Group || len(c.Cells) != len(group.Team_groups)*len(group.Team_groups) {
			t.Fatal("reference dimensions mismatch")
		}
		seen := make(map[[2]int]bool, len(c.Cells))
		for _, cell := range c.Cells {
			key := [2]int{cell.Team, cell.Position}
			if seen[key] {
				t.Fatal("duplicate cell")
			}
			seen[key] = true
			if cell.MC.Samples != 5000000 {
				t.Fatal("reference requires independent 5M MC")
			}
			if cell.Probability != nil && *cell.Probability == 0 && cell.Status != "impossible" {
				t.Fatal("sampling zero reported as known zero")
			}
			switch cell.Status {
			case "impossible":
				if !cell.Scorable || cell.Probability == nil || *cell.Probability != 0 || cell.MC.Hits != 0 || cell.Proof == nil || !cell.Proof.Impossible {
					t.Fatal("invalid impossibility record")
				}
			case "reference_mc":
				if !cell.Scorable || cell.MC.Hits < 25 || cell.Probability == nil || *cell.Probability != float64(cell.MC.Hits)/float64(cell.MC.Samples) {
					t.Fatal("invalid MC reference")
				}
			case "reference_is":
				if !cell.Scorable || cell.Confirmations < 2 || cell.Probability == nil || *cell.Probability <= 0 || len(cell.Range) != 2 || cell.Range[1]/cell.Range[0] > 3 {
					t.Fatal("invalid importance reference")
				}
			case "provisional_is", "reachable_no_reference", "estimated_unverified", "undecided":
				if cell.Scorable {
					t.Fatal("uncertified cell is scorable")
				}
			default:
				t.Fatalf("unknown status %s", cell.Status)
			}
		}
	}
}

// Optional equivalence check against the private raw exports. The committed
// portable fixtures contain every field GroupType consumes, in the same order.
func TestGoldenReferenceRawInputEquivalence(t *testing.T) {
	paths := os.Getenv("RARE_POSITION_BUDGET_EXPERIMENT_REQUESTS")
	if paths == "" {
		t.Skip("set original saved request paths")
	}
	for _, path := range strings.Split(paths, ",") {
		raw, err := os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
		var original GroupType
		if err := json.Unmarshal(raw, &original); err != nil {
			t.Fatal(err)
		}
		hash := fmt.Sprintf("%x", sha256.Sum256(raw))
		portablePath := filepath.Join("..", "experiments", "rare_positions", "reference", "2026-09-30-hundredfold", "inputs", fmt.Sprintf("group-%d-%s.json", original.Id, hash[:8]))
		data, err := os.ReadFile(portablePath)
		if err != nil {
			t.Fatal(err)
		}
		var portable GroupType
		if err := json.Unmarshal(data, &portable); err != nil {
			t.Fatal(err)
		}
		if !reflect.DeepEqual(original, portable) {
			t.Fatalf("portable input changes consumed fields: %s", path)
		}
	}
}
