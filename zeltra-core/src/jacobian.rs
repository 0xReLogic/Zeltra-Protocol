//! Automated Sparse Jacobian Engine and Tangent-Space Rigidity Auditing (DEC-047, IACR ePrint 2026/1852).
//!
//! Provides mathematically exact differential verification for R1CS circuits over `ark_bls12_381::Fr`:
//! - Sparse Augmented Jacobian construction: $M(a) = \begin{pmatrix} J(a) \\ E_F \end{pmatrix}$ in $O(N)$ operations
//! - Sparse Markowitz / Gaussian Row Echelon reduction over finite fields $\mathbb{F}_p$
//! - Row-Space Rigidity Test: $e_j^T \in \text{rowspan}(M(a))$ (Corollary 12)
//! - Projected Tangent Dimension: $r_T(a) = \dim \pi_T(\ker M(a))$ (Proposition 14)
//! - Kernel nullspace vector extraction ($v \in \ker M(a)$ with $v_F = 0$)
//! - Exact Affine-Line Non-Uniqueness Certificate verification and exploit synthesis (Proposition 27)

use ark_bls12_381::Fr;
use ark_ff::{Field, One, Zero};
use ark_relations::gr1cs::ConstraintSystemRef;

/// Standard R1CS constraint matrices extracted from `ConstraintSystemRef`.
#[derive(Clone, Debug)]
pub struct R1CSMatrices {
    pub num_instance_variables: usize,
    pub num_witness_variables: usize,
    pub a: Vec<Vec<(Fr, usize)>>,
    pub b: Vec<Vec<(Fr, usize)>>,
    pub c: Vec<Vec<(Fr, usize)>>,
}

impl R1CSMatrices {
    pub fn num_constraints(&self) -> usize {
        self.a.len()
    }
}

/// Non-zero coordinate entry in a sparse matrix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SparseEntry {
    pub row: usize,
    pub col: usize,
    pub value: Fr,
}

/// Compressed sparse matrix over `Fr` where each row is a vector of `(col, value)` pairs sorted by `col`.
#[derive(Clone, Debug)]
pub struct SparseMatrixFr {
    pub num_rows: usize,
    pub num_cols: usize,
    pub rows: Vec<Vec<(usize, Fr)>>,
}

impl SparseMatrixFr {
    /// Creates an empty sparse matrix with given dimensions.
    pub fn new(num_rows: usize, num_cols: usize) -> Self {
        Self {
            num_rows,
            num_cols,
            rows: Vec::with_capacity(num_rows),
        }
    }

    /// Adds a row to the matrix, automatically sorting columns, merging duplicates, and stripping zeros.
    pub fn add_row(&mut self, mut entries: Vec<(usize, Fr)>) {
        if entries.is_empty() {
            self.rows.push(Vec::new());
            return;
        }

        // Sort by column index
        entries.sort_by_key(|&(c, _)| c);

        let mut cleaned = Vec::with_capacity(entries.len());
        let mut curr_col = entries[0].0;
        let mut curr_val = Fr::zero();

        for (col, val) in entries {
            if col == curr_col {
                curr_val += val;
            } else {
                if !curr_val.is_zero() {
                    cleaned.push((curr_col, curr_val));
                }
                curr_col = col;
                curr_val = val;
            }
        }
        if !curr_val.is_zero() {
            cleaned.push((curr_col, curr_val));
        }

        self.rows.push(cleaned);
    }
}

/// Sparse row echelon basis over `Fr`.
#[derive(Clone, Debug)]
pub struct EchelonBasis {
    pub num_cols: usize,
    /// `pivot_rows[c]` holds `Some(row)` if column `c` is a pivot column.
    /// The leading entry in `row` has column index `c` and value `Fr::one()`.
    pub pivot_rows: Vec<Option<Vec<(usize, Fr)>>>,
    pub rank: usize,
}

impl EchelonBasis {
    /// Initializes an empty echelon basis for a vector space of dimension `num_cols`.
    pub fn new(num_cols: usize) -> Self {
        Self {
            num_cols,
            pivot_rows: vec![None; num_cols],
            rank: 0,
        }
    }

    /// Inserts a row into the echelon basis.
    /// Returns `true` if the row was linearly independent (rank increased),
    /// or `false` if it was reduced to zero (linearly dependent).
    pub fn insert_row(&mut self, mut row: Vec<(usize, Fr)>) -> bool {
        self.reduce_row(&mut row);

        if row.is_empty() {
            return false;
        }

        let lead_col = row[0].0;
        let lead_val = row[0].1;

        // Normalize row so that leading entry is Fr::one()
        let inv = lead_val
            .inverse()
            .expect("Non-zero field element must have inverse");
        for entry in &mut row {
            entry.1 *= inv;
        }

        self.pivot_rows[lead_col] = Some(row);
        self.rank += 1;
        true
    }

    /// Reduces `row` against the current echelon basis in-place.
    /// When finished, `row` either is empty (in row span) or has leading entry at a column
    /// where `self.pivot_rows[lead_col]` is `None`.
    pub fn reduce_row(&self, row: &mut Vec<(usize, Fr)>) {
        while !row.is_empty() {
            let lead_col = row[0].0;
            let lead_val = row[0].1;

            if let Some(ref pivot_row) = self.pivot_rows[lead_col] {
                // row = row - lead_val * pivot_row
                sub_scaled_row(row, pivot_row, lead_val);
            } else {
                break;
            }
        }
    }

    /// Tests if a row vector belongs to the row space of this basis (Corollary 12).
    /// `w ∈ rowspan(M) ⟺ reduce_row(w) == 0`.
    pub fn contains_row(&self, mut row: Vec<(usize, Fr)>) -> bool {
        self.reduce_row(&mut row);
        row.is_empty()
    }

    /// Tests if target standard basis vector `e_j^T` belongs to `rowspan(M(a))` (Corollary 12).
    pub fn is_target_rigid(&self, target_idx: usize) -> bool {
        assert!(target_idx < self.num_cols, "Target index out of bounds");
        self.contains_row(vec![(target_idx, Fr::one())])
    }

    /// Computes the Projected Tangent Dimension $r_T(a)$ for a set of target wires $T$ (Proposition 14):
    /// $r_T(a) = \dim \pi_T(\ker M(a)) = \text{rank}\begin{pmatrix} M(a) \\ E_T \end{pmatrix} - \text{rank}(M(a))$.
    pub fn projected_tangent_dimension(&self, targets: &[usize]) -> usize {
        let mut basis_clone = self.clone();
        let mut free_dim = 0;
        for &t in targets {
            assert!(t < self.num_cols, "Target index out of bounds");
            if basis_clone.insert_row(vec![(t, Fr::one())]) {
                free_dim += 1;
            }
        }
        free_dim
    }

    /// Extracts a non-zero tangent vector $v \in \ker M(a)$ with $v_F = 0$ that moves free target `free_target_idx`.
    /// Sets $v[\text{free\_target\_idx}] = 1$, all other free variables to 0, and back-solves pivot variables.
    pub fn extract_nullspace_vector(&self, free_target_idx: usize) -> Option<Vec<Fr>> {
        assert!(free_target_idx < self.num_cols);

        // If target is already rigid, no such nullspace vector exists
        if self.is_target_rigid(free_target_idx) {
            return None;
        }

        let mut v = vec![Fr::zero(); self.num_cols];
        v[free_target_idx] = Fr::one();

        // Back-solve for pivot variables in reverse order (highest column to lowest)
        for col in (0..self.num_cols).rev() {
            if let Some(ref pivot_row) = self.pivot_rows[col] {
                // pivot_row has: 1 * v[col] + sum_{k > col} coeff_k * v[k] = 0
                // => v[col] = - sum_{k > col} coeff_k * v[k]
                let mut sum = Fr::zero();
                for &(k, coeff) in pivot_row.iter().skip(1) {
                    sum += coeff * v[k];
                }
                v[col] = -sum;
            }
        }

        Some(v)
    }
}

/// Helper: computes `target = target - scalar * source` on sorted sparse rows.
fn sub_scaled_row(target: &mut Vec<(usize, Fr)>, source: &[(usize, Fr)], scalar: Fr) {
    if scalar.is_zero() || source.is_empty() {
        return;
    }

    let mut result = Vec::with_capacity(target.len() + source.len());
    let mut i = 0;
    let mut j = 0;

    while i < target.len() && j < source.len() {
        let t_col = target[i].0;
        let s_col = source[j].0;

        if t_col < s_col {
            result.push(target[i]);
            i += 1;
        } else if t_col > s_col {
            let val = -scalar * source[j].1;
            if !val.is_zero() {
                result.push((s_col, val));
            }
            j += 1;
        } else {
            let val = target[i].1 - scalar * source[j].1;
            if !val.is_zero() {
                result.push((t_col, val));
            }
            i += 1;
            j += 1;
        }
    }

    while i < target.len() {
        result.push(target[i]);
        i += 1;
    }

    while j < source.len() {
        let val = -scalar * source[j].1;
        if !val.is_zero() {
            result.push((source[j].0, val));
        }
        j += 1;
    }

    *target = result;
}

/// Sparse Augmented Jacobian $M(a) = \begin{pmatrix} J(a) \\ E_F \end{pmatrix}$.
#[derive(Clone, Debug)]
pub struct AugmentedJacobian {
    pub m_constraints: usize,
    pub n_wires: usize,
    pub conditioned_indices: Vec<usize>,
    pub matrix: SparseMatrixFr,
}

impl AugmentedJacobian {
    /// Assembles the Sparse Augmented Jacobian $M(a) = \begin{pmatrix} J(a) \\ E_F \end{pmatrix}$ in $O(N)$ field operations.
    ///
    /// For each R1CS constraint $i$:
    /// $$d_a f_i = (B_i \cdot a) A_i + (A_i \cdot a) B_i - C_i$$
    /// And for each conditioned wire $k \in F$, appends $e_k^T$.
    pub fn build(matrices: &R1CSMatrices, witness: &[Fr], conditioned_indices: &[usize]) -> Self {
        let m = matrices.num_constraints();
        let n = matrices.num_instance_variables + matrices.num_witness_variables;
        assert_eq!(
            witness.len(),
            n,
            "Witness length must equal total wire count"
        );

        let total_rows = m + conditioned_indices.len();
        let mut matrix = SparseMatrixFr::new(total_rows, n);

        // 1. Build Jacobian rows J(a)
        for i in 0..m {
            // L_val = A_i . a
            let mut l_val = Fr::zero();
            for &(coeff, col) in &matrices.a[i] {
                l_val += coeff * witness[col];
            }

            // R_val = B_i . a
            let mut r_val = Fr::zero();
            for &(coeff, col) in &matrices.b[i] {
                r_val += coeff * witness[col];
            }

            // d_a f_i = r_val * A_i + l_val * B_i - C_i
            let mut row_entries =
                Vec::with_capacity(matrices.a[i].len() + matrices.b[i].len() + matrices.c[i].len());

            for &(coeff, col) in &matrices.a[i] {
                row_entries.push((col, r_val * coeff));
            }
            for &(coeff, col) in &matrices.b[i] {
                row_entries.push((col, l_val * coeff));
            }
            for &(coeff, col) in &matrices.c[i] {
                row_entries.push((col, -coeff));
            }

            matrix.add_row(row_entries);
        }

        // 2. Build Conditioned rows E_F
        for &k in conditioned_indices {
            assert!(k < n, "Conditioned index out of bounds");
            matrix.add_row(vec![(k, Fr::one())]);
        }

        Self {
            m_constraints: m,
            n_wires: n,
            conditioned_indices: conditioned_indices.to_vec(),
            matrix,
        }
    }

    /// Computes the EchelonBasis for $M(a)$ via sparse Gaussian reduction.
    pub fn compute_echelon_basis(&self) -> EchelonBasis {
        let mut basis = EchelonBasis::new(self.n_wires);
        for row in &self.matrix.rows {
            basis.insert_row(row.clone());
        }
        basis
    }
}

/// Proposition 27 Affine-Line Certificate Verifier and Counterexample Generator.
#[derive(Clone, Debug)]
pub struct AffineLineCertificate {
    pub base_witness: Vec<Fr>,
    pub tangent_direction: Vec<Fr>,
    pub free_target_index: usize,
    pub is_certified: bool,
}

impl AffineLineCertificate {
    /// Verifies Proposition 27 (Singh 2026/1852):
    /// Evaluates the quadratic constraint $Q_i(v) := \ell_i(v) \cdot r_i(v) == 0$ for all $1 \le i \le m$,
    /// where $\ell_i(v)$ and $r_i(v)$ are the linear homogeneous parts of $L_i, R_i$.
    ///
    /// If $Q_i(v) == 0$ for all constraints and $d_a f_i(v) == 0$, then $a + t \cdot v \in V_u(\mathbb{F}_p)$
    /// for ALL $t \in \mathbb{F}_p$.
    pub fn verify_affine_line(
        matrices: &R1CSMatrices,
        base_a: &[Fr],
        tangent_v: &[Fr],
        target_idx: usize,
    ) -> bool {
        assert_eq!(
            tangent_v[0],
            Fr::zero(),
            "Tangent vector cannot move structural constant ONE at index 0"
        );
        assert!(
            !tangent_v[target_idx].is_zero(),
            "Tangent vector must move the specified target wire"
        );

        let m = matrices.num_constraints();
        for i in 0..m {
            // Evaluate linear homogeneous parts \ell_i(v) and r_i(v) (ignore col 0)
            let mut ell_v = Fr::zero();
            for &(coeff, col) in &matrices.a[i] {
                if col > 0 {
                    ell_v += coeff * tangent_v[col];
                }
            }

            let mut r_v = Fr::zero();
            for &(coeff, col) in &matrices.b[i] {
                if col > 0 {
                    r_v += coeff * tangent_v[col];
                }
            }

            // Quadratic boundary condition: Q_i(v) = ell_i(v) * r_i(v) == 0
            let q_i = ell_v * r_v;
            if !q_i.is_zero() {
                return false;
            }
        }

        // Verify that a is a valid satisfying witness
        for i in 0..m {
            let mut l_a = Fr::zero();
            for &(coeff, col) in &matrices.a[i] {
                l_a += coeff * base_a[col];
            }
            let mut r_a = Fr::zero();
            for &(coeff, col) in &matrices.b[i] {
                r_a += coeff * base_a[col];
            }
            let mut o_a = Fr::zero();
            for &(coeff, col) in &matrices.c[i] {
                o_a += coeff * base_a[col];
            }
            if l_a * r_a != o_a {
                return false;
            }
        }

        true
    }

    /// Synthesizes an explicit counterexample witness pair $(a, a + t \cdot v)$ over $\mathbb{F}_p$.
    pub fn synthesize_pair(&self, t: Fr) -> (Vec<Fr>, Vec<Fr>) {
        let n = self.base_witness.len();
        let mut second_witness = Vec::with_capacity(n);
        for i in 0..n {
            second_witness.push(self.base_witness[i] + t * self.tangent_direction[i]);
        }
        (self.base_witness.clone(), second_witness)
    }
}

/// Three-level diagnostic result classification (DEC-047 Section 3 / Singh Section 11).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiagnosticClassification {
    /// Class 1: Pointwise Infinitesimal Freedom ($r_T(a) > 0$).
    Class1InfinitesimalFreedom {
        projected_tangent_dim: usize,
        free_targets: Vec<usize>,
    },
    /// Class 2: Concrete Finite-Field Non-Uniqueness (Proposition 27 certified exploit pair).
    Class2ConcreteNonUniqueness {
        free_target: usize,
        exploit_certified: bool,
    },
    /// Class 3: Pointwise First-Order Rigidity ($r_T(a) == 0$).
    Class3PointwiseRigidity,
}

/// High-level diagnostic runner evaluating target rigidity and Proposition 27 certificates.
pub fn run_jacobian_diagnostic(
    matrices: &R1CSMatrices,
    witness: &[Fr],
    conditioned_indices: &[usize],
    target_indices: &[usize],
) -> (DiagnosticClassification, EchelonBasis) {
    let jacobian = AugmentedJacobian::build(matrices, witness, conditioned_indices);
    let basis = jacobian.compute_echelon_basis();

    let mut free_targets = Vec::new();
    for &t in target_indices {
        if !basis.is_target_rigid(t) {
            free_targets.push(t);
        }
    }

    if free_targets.is_empty() {
        return (DiagnosticClassification::Class3PointwiseRigidity, basis);
    }

    let r_t = basis.projected_tangent_dimension(target_indices);

    // Attempt Proposition 27 Affine-Line Certificate synthesis for each free target
    for &free_target in &free_targets {
        if let Some(v) = basis.extract_nullspace_vector(free_target) {
            if AffineLineCertificate::verify_affine_line(matrices, witness, &v, free_target) {
                return (
                    DiagnosticClassification::Class2ConcreteNonUniqueness {
                        free_target,
                        exploit_certified: true,
                    },
                    basis,
                );
            }
        }
    }

    (
        DiagnosticClassification::Class1InfinitesimalFreedom {
            projected_tangent_dim: r_t,
            free_targets,
        },
        basis,
    )
}

/// Helper: extracts the full witness vector $a \in \mathbb{F}_p^n$ and constraint matrices from `ConstraintSystemRef`.
pub fn extract_witness_and_matrices(cs: &ConstraintSystemRef<Fr>) -> (Vec<Fr>, R1CSMatrices) {
    cs.inline_all_lcs();
    let matrices_map = cs.to_matrices().expect("Failed to extract R1CS matrices");
    let cs_borrow = cs.borrow().unwrap();
    let num_instance_variables = cs_borrow.num_instance_variables();
    let num_witness_variables = cs_borrow.num_witness_variables();

    let mut full_witness = Vec::with_capacity(num_instance_variables + num_witness_variables);
    full_witness.extend_from_slice(cs_borrow.instance_assignment().unwrap());
    full_witness.extend_from_slice(cs_borrow.witness_assignment().unwrap());

    let mut a_all = Vec::new();
    let mut b_all = Vec::new();
    let mut c_all = Vec::new();

    for matrix_list in matrices_map.values() {
        if matrix_list.len() >= 3 {
            a_all.extend(matrix_list[0].clone());
            b_all.extend(matrix_list[1].clone());
            c_all.extend(matrix_list[2].clone());
        }
    }

    (
        full_witness,
        R1CSMatrices {
            num_instance_variables,
            num_witness_variables,
            a: a_all,
            b: b_all,
            c: c_all,
        },
    )
}

/// Helper: converts an FpVar into its column index in the flattened R1CS matrix.
pub fn var_to_col_index(
    var: &ark_r1cs_std::fields::fp::FpVar<Fr>,
    num_instance_vars: usize,
) -> Option<usize> {
    match var {
        ark_r1cs_std::fields::fp::FpVar::Var(alloc) => {
            alloc.variable.get_variable_index(num_instance_vars)
        }
        _ => None,
    }
}
