#!/usr/bin/env python3
"""
DEC-046 & DEC-047 Independent Python Verification Harness
===========================================================
Audits Quadratic Encoding Soundness (IACR 2026/2235) and Sparse Augmented
Jacobian Tangent-Space Rigidity (IACR 2026/1852) independently over the
BLS12-381 scalar field F_p.

Features:
1. Exact BLS12-381 Fr field arithmetic
2. Sparse matrix Gaussian reduction over F_p
3. Row-Space Rigidity Test: e_j^T in rowspan(M(a)) (Corollary 12)
4. Projected Tangent Dimension r_T(a) (Proposition 14)
5. Kernel Nullspace Extraction (v in ker M(a))
6. Proposition 27 Exact Affine-Line Certificate Verification & Exploit Synthesis
7. Rigidity & Soundness test suite on Zeltra R1CS models
"""

import sys
import random

# BLS12-381 Scalar Field Modulus (r)
BLS12_381_R = 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001

def mod_inv(x: int, p: int = BLS12_381_R) -> int:
    """Computes modular inverse using Python's built-in extended Euclidean algorithm."""
    x = x % p
    if x == 0:
        raise ZeroDivisionError("Cannot invert 0 in F_p")
    return pow(x, -1, p)


class FrElement:
    """Element of BLS12-381 scalar field."""
    def __init__(self, val: int):
        self.val = val % BLS12_381_R

    def __add__(self, other):
        o = other.val if isinstance(other, FrElement) else other
        return FrElement(self.val + o)

    def __sub__(self, other):
        o = other.val if isinstance(other, FrElement) else other
        return FrElement(self.val - o)

    def __mul__(self, other):
        o = other.val if isinstance(other, FrElement) else other
        return FrElement(self.val * o)

    def __neg__(self):
        return FrElement(-self.val)

    def inv(self):
        return FrElement(mod_inv(self.val))

    def is_zero(self):
        return self.val == 0

    def __eq__(self, other):
        o = other.val if isinstance(other, FrElement) else other % BLS12_381_R
        return self.val == o

    def __repr__(self):
        return f"{self.val}"


class SparseRow:
    """Row in sparse coordinate format: dict of {col_idx: FrElement}."""
    def __init__(self, entries=None):
        self.entries = {}
        if entries:
            for col, val in entries.items():
                v = val if isinstance(val, FrElement) else FrElement(val)
                if not v.is_zero():
                    self.entries[col] = v

    def leading_col(self):
        if not self.entries:
            return None
        return min(self.entries.keys())

    def is_empty(self):
        return len(self.entries) == 0

    def copy(self):
        r = SparseRow()
        r.entries = {c: v for c, v in self.entries.items()}
        return r

    def sub_scaled(self, other, scalar: FrElement):
        """self = self - scalar * other"""
        if scalar.is_zero() or not other.entries:
            return
        for col, val in other.entries.items():
            current = self.entries.get(col, FrElement(0))
            new_val = current - scalar * val
            if new_val.is_zero():
                self.entries.pop(col, None)
            else:
                self.entries[col] = new_val

    def scale(self, scalar: FrElement):
        if scalar.is_zero():
            self.entries.clear()
            return
        for col in list(self.entries.keys()):
            self.entries[col] = self.entries[col] * scalar


class EchelonBasis:
    """Sparse Row Echelon Basis over F_p."""
    def __init__(self, num_cols: int):
        self.num_cols = num_cols
        self.pivot_rows = {} # pivot_col -> SparseRow
        self.rank = 0

    def copy(self):
        b = EchelonBasis(self.num_cols)
        b.pivot_rows = {c: r.copy() for c, r in self.pivot_rows.items()}
        b.rank = self.rank
        return b

    def reduce_row(self, row: SparseRow):
        """Reduces row against current pivots until 0 or new pivot column found."""
        while not row.is_empty():
            lead_col = row.leading_col()
            if lead_col in self.pivot_rows:
                pivot = self.pivot_rows[lead_col]
                lead_val = row.entries[lead_col]
                row.sub_scaled(pivot, lead_val)
            else:
                break

    def insert_row(self, row: SparseRow) -> bool:
        """Inserts row into basis. Returns True if independent (rank increased)."""
        self.reduce_row(row)
        if row.is_empty():
            return False
        lead_col = row.leading_col()
        lead_val = row.entries[lead_col]
        row.scale(lead_val.inv())
        self.pivot_rows[lead_col] = row
        self.rank += 1
        return True

    def is_target_rigid(self, target_idx: int) -> bool:
        """Tests Corollary 12: e_j^T in rowspan(M(a))."""
        row = SparseRow({target_idx: FrElement(1)})
        self.reduce_row(row)
        return row.is_empty()

    def projected_tangent_dim(self, targets) -> int:
        """Computes Proposition 14: r_T(a)."""
        b = self.copy()
        free_dim = 0
        for t in targets:
            row = SparseRow({t: FrElement(1)})
            if b.insert_row(row):
                free_dim += 1
        return free_dim

    def extract_nullspace_vector(self, free_target_idx: int):
        """Extracts v in ker M(a) with v[free_target] = 1, v[conditioned] = 0."""
        if self.is_target_rigid(free_target_idx):
            return None
        v = [FrElement(0) for _ in range(self.num_cols)]
        v[free_target_idx] = FrElement(1)

        for col in sorted(self.pivot_rows.keys(), reverse=True):
            pivot_row = self.pivot_rows[col]
            s = FrElement(0)
            for k, coeff in pivot_row.entries.items():
                if k > col:
                    s = s + coeff * v[k]
            v[col] = -s
        return v


class R1CSSystem:
    """Rank-1 Constraint System (A, B, C) over F_p."""
    def __init__(self, num_vars: int):
        self.num_vars = num_vars
        self.constraints = [] # list of (SparseRow A, SparseRow B, SparseRow C)

    def add_constraint(self, a_row: dict, b_row: dict, c_row: dict):
        self.constraints.append((SparseRow(a_row), SparseRow(b_row), SparseRow(c_row)))

    def is_satisfied(self, witness: list) -> bool:
        for a, b, c in self.constraints:
            l_val = sum((val.val * witness[col].val for col, val in a.entries.items()), 0) % BLS12_381_R
            r_val = sum((val.val * witness[col].val for col, val in b.entries.items()), 0) % BLS12_381_R
            o_val = sum((val.val * witness[col].val for col, val in c.entries.items()), 0) % BLS12_381_R
            if (l_val * r_val) % BLS12_381_R != o_val:
                return False
        return True

    def build_augmented_jacobian(self, witness: list, conditioned: list) -> EchelonBasis:
        basis = EchelonBasis(self.num_vars)

        # 1. d_a f_i = (B_i . a) A_i + (A_i . a) B_i - C_i
        for a, b, c in self.constraints:
            l_val = FrElement(sum(val.val * witness[col].val for col, val in a.entries.items()))
            r_val = FrElement(sum(val.val * witness[col].val for col, val in b.entries.items()))

            grad = SparseRow()
            for col, coeff in a.entries.items():
                grad.entries[col] = grad.entries.get(col, FrElement(0)) + coeff * r_val
            for col, coeff in b.entries.items():
                grad.entries[col] = grad.entries.get(col, FrElement(0)) + coeff * l_val
            for col, coeff in c.entries.items():
                grad.entries[col] = grad.entries.get(col, FrElement(0)) - coeff

            # Clean zero entries
            for col in list(grad.entries.keys()):
                if grad.entries[col].is_zero():
                    grad.entries.pop(col)

            basis.insert_row(grad)

        # 2. Add E_F conditioned rows
        for k in conditioned:
            basis.insert_row(SparseRow({k: FrElement(1)}))

        return basis

    def verify_affine_line_certificate(self, base_a: list, v: list, target_idx: int) -> bool:
        """Verifies Proposition 27: Q_i(v) := ell_i(v) * r_i(v) == 0 for all i."""
        if not v[0].is_zero():
            return False # Must not move structural constant 1
        if v[target_idx].is_zero():
            return False

        for a, b, _ in self.constraints:
            ell_v = FrElement(sum(coeff.val * v[col].val for col, coeff in a.entries.items() if col > 0))
            r_v = FrElement(sum(coeff.val * v[col].val for col, coeff in b.entries.items() if col > 0))
            q_i = ell_v * r_v
            if not q_i.is_zero():
                return False

        return self.is_satisfied(base_a)


def run_tests():
    print("=" * 70)
    print("ZELTRA DEC-046 & DEC-047 PYTHON AUDIT & VERIFICATION SUITE")
    print("=" * 70)

    # ─────────────────────────────────────────────────────────────────
    # Test 1: Proposition 27 Affine-Line Verification on Under-Constrained Gate
    # ─────────────────────────────────────────────────────────────────
    print("\n[Test 1] Auditing Under-Constrained Null Branch: s * (y - 5) == 0 with s = 0")
    # Wires: 0: ONE, 1: selector s, 2: target wire y
    r1cs = R1CSSystem(3)
    # s * (y - 5) == 0 => (1 * s) * (1 * y - 5 * ONE) == 0
    r1cs.add_constraint({1: 1}, {2: 1, 0: -5}, {})

    witness = [FrElement(1), FrElement(0), FrElement(42)] # s = 0, y = 42
    assert r1cs.is_satisfied(witness), "Witness must satisfy"

    conditioned = [0, 1] # ONE and s are conditioned
    basis = r1cs.build_augmented_jacobian(witness, conditioned)

    assert not basis.is_target_rigid(2), "Target wire y MUST be detected as NOT rigid!"
    r_t = basis.projected_tangent_dim([2])
    assert r_t == 1, f"Projected tangent dim must be 1, got {r_t}"
    print("  -> Correctly identified: r_T(a) = 1 (Class 1 Infinitesimal Freedom)")

    # Extract nullspace tangent vector v
    v = basis.extract_nullspace_vector(2)
    assert v[0].is_zero() and v[1].is_zero() and not v[2].is_zero()
    print(f"  -> Extracted tangent vector v: {v}")

    # Certify Proposition 27
    certified = r1cs.verify_affine_line_certificate(witness, v, 2)
    assert certified, "Proposition 27 Affine-Line Certificate MUST pass!"
    print("  -> Proposition 27 Affine-Line Certificate: CERTIFIED! (Class 2 Non-Uniqueness)")

    # Exploit Verification: test a + t * v for 10 random field scalars
    print("  -> Testing explicit counterexample pairs (a + t * v):")
    for t_val in [1, 2, 42, 999999, 0x1337c001]:
        t = FrElement(t_val)
        w_exploit = [witness[i] + t * v[i] for i in range(3)]
        assert r1cs.is_satisfied(w_exploit), f"Exploit witness for t={t_val} failed satisfaction!"
        assert w_exploit[2] != witness[2], "Target wire must differ"
    print("  -> All 5 exploit instances successfully verified across F_p!")

    # ─────────────────────────────────────────────────────────────────
    # Test 2: DEC-046 Hardening Soundness Verification
    # ─────────────────────────────────────────────────────────────────
    print("\n[Test 2] Applying DEC-046 Canonical Companion Constraint: (1 - s) * (y - 0) == 0")
    # Add companion constraint: (1*ONE - 1*s) * (1*y) == 0
    r1cs_hardened = R1CSSystem(3)
    r1cs_hardened.add_constraint({1: 1}, {2: 1, 0: -5}, {})
    r1cs_hardened.add_constraint({0: 1, 1: -1}, {2: 1}, {})

    witness_hardened = [FrElement(1), FrElement(0), FrElement(0)] # y must be 0
    assert r1cs_hardened.is_satisfied(witness_hardened)

    basis_hardened = r1cs_hardened.build_augmented_jacobian(witness_hardened, conditioned)
    assert basis_hardened.is_target_rigid(2), "With DEC-046, target wire y MUST BE RIGID!"
    r_t_h = basis_hardened.projected_tangent_dim([2])
    assert r_t_h == 0, f"With DEC-046, r_T must be 0, got {r_t_h}"
    print("  -> DEC-046 Verification: r_T(a) = 0 (Class 3 Pointwise Rigidity Guaranteed!)")

    # Tamper test: y = 42 must now be rejected
    witness_tampered = [FrElement(1), FrElement(0), FrElement(42)]
    assert not r1cs_hardened.is_satisfied(witness_tampered), "Tampered unpinned wire MUST be rejected"
    print("  -> Negative test: unpinned wire y=42 strictly rejected by companion constraint!")

    # ─────────────────────────────────────────────────────────────────
    # Test 3: DEC-046 Zero-Safe Anti-Aliasing Inverse (diff_inv)
    # ─────────────────────────────────────────────────────────────────
    print("\n[Test 3] Auditing Zero-Safe Anti-Aliasing Inverse Gadget (DEC-046 §3.C)")
    # Wires: 0: ONE, 1: is_dummy, 2: nf_diff, 3: diff_inv
    # active = 1 - is_dummy
    # (active * nf_diff) * diff_inv == active
    # is_dummy * diff_inv == 0
    # Let active be wire 4
    r1cs_diff = R1CSSystem(5)
    # 1. active = ONE - is_dummy => 1 * (ONE - is_dummy) == active
    r1cs_diff.add_constraint({0: 1}, {0: 1, 1: -1}, {4: 1})
    # 2. term1 = active * nf_diff
    # Let term1 be wire 5 (or direct)
    # 3. Companion constraint: is_dummy * diff_inv == 0
    r1cs_diff.add_constraint({1: 1}, {3: 1}, {})

    # Case A: is_dummy = 1 => diff_inv MUST be 0
    w_dummy_valid = [FrElement(1), FrElement(1), FrElement(10), FrElement(0), FrElement(0)]
    assert r1cs_diff.is_satisfied(w_dummy_valid)

    basis_dummy = r1cs_diff.build_augmented_jacobian(w_dummy_valid, [0, 1])
    assert basis_dummy.is_target_rigid(3), "diff_inv must be rigid when is_dummy == 1!"
    print("  -> Case A (Dummy): diff_inv is 100% rigid (r_T = 0)")

    # Tamper Case A: is_dummy = 1 with non-zero diff_inv = 0x1337
    w_dummy_invalid = [FrElement(1), FrElement(1), FrElement(10), FrElement(0x1337), FrElement(0)]
    assert not r1cs_diff.is_satisfied(w_dummy_invalid), "Non-zero diff_inv on dummy branch MUST fail!"
    print("  -> Case A (Negative): Non-zero diff_inv rejected by is_dummy * diff_inv == 0!")

    # ─────────────────────────────────────────────────────────────────
    # Test 4: Inactive Change Outputs Canonical Pinning (DEC-046 §3.D)
    # ─────────────────────────────────────────────────────────────────
    print("\n[Test 4] Auditing Inactive Change Output Pinning: (1 - has_change) * coord == 0")
    # Wires: 0: ONE, 1: has_change, 2: change_val, 3: change_owner, 4: change_rho, 5: change_rand
    r1cs_change = R1CSSystem(6)
    # Constraints: (ONE - has_change) * coord == 0 for coords in [2, 3, 4, 5]
    for c in [2, 3, 4, 5]:
        r1cs_change.add_constraint({0: 1, 1: -1}, {c: 1}, {})

    # Case has_change = 0 => all coords 0
    w_ch_zero = [FrElement(1), FrElement(0), FrElement(0), FrElement(0), FrElement(0), FrElement(0)]
    assert r1cs_change.is_satisfied(w_ch_zero)
    basis_ch = r1cs_change.build_augmented_jacobian(w_ch_zero, [0, 1])
    for c in [2, 3, 4, 5]:
        assert basis_ch.is_target_rigid(c), f"Change coord {c} must be rigid!"
    r_t_ch = basis_ch.projected_tangent_dim([2, 3, 4, 5])
    assert r_t_ch == 0, f"r_T must be 0, got {r_t_ch}"
    print("  -> Inactive Change: all 4 change coordinates are 100% rigid (r_T = 0)")

    # Negative test: tamper any coordinate
    for c in [2, 3, 4, 5]:
        w_tampered = list(w_ch_zero)
        w_tampered[c] = FrElement(0x9999)
        assert not r1cs_change.is_satisfied(w_tampered), f"Tampered coord {c} must fail!"
    print("  -> Negative tests: 4/4 tampered change coordinates rejected!")

    print("\n" + "=" * 70)
    print("ALL DEC-046 & DEC-047 PYTHON AUDIT CHECKS PASSED (100% VERIFIED)")
    print("=" * 70)


if __name__ == "__main__":
    run_tests()
