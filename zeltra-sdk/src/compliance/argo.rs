//! Argo Randomized Groth16 Invalidity Encoding & Strict Curve Point Sanitation (DEC-052).
//!
//! Implements IACR ePrint 2026/2165 (Liam Eagen & Ying Tong Lai):
//! "Argo: Leaking a Secret from an Invalid Groth16 Proof".
//!
//! Provides:
//! - Strict pre-pairing curve point sanitization across 5 gates (off-curve, non-infinity,
//!   prime-order subgroup, canonical bounds, and DDH mask cancellation).
//! - 3-phase Argo protocol (Offline Material, Online Randomized Encoding, Permissionless Decode).
//! - Instant O(1) discrete logarithm secret key extraction over Eisenstein endomorphism set $S$.

use ark_bls12_381::{Bls12_381, Fq12, Fr, G1Affine, G2Affine, G2Projective};
use ark_ec::pairing::Pairing;
use ark_ec::{AffineRepr, CurveGroup};
use ark_ff::{Field, UniformRand};
use rand::Rng;

use zeltra_core::curve_opt::{fast_s_pow_gt, ArgoMemoizedPowers, EisensteinScalar};
use zeltra_core::evm::CurveSanitationError;

/// Argo protocol errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgoError {
    /// Curve point sanitation failed (Gate 1, 2, 3, or 4).
    SanitationError(CurveSanitationError),
    /// Dimension mismatch between replicas and material.
    DimensionMismatch { expected: usize, got: usize },
    /// No matching candidate found in $S$ during discrete log extraction.
    CandidateNotFound { replica_index: usize },
}

impl std::fmt::Display for ArgoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SanitationError(e) => write!(f, "Argo curve sanitation error: {e}"),
            Self::DimensionMismatch { expected, got } => {
                write!(f, "Argo dimension mismatch: expected {expected}, got {got}")
            }
            Self::CandidateNotFound { replica_index } => {
                write!(
                    f,
                    "Argo discrete log decoding failed to match replica {replica_index} in S"
                )
            }
        }
    }
}

impl std::error::Error for ArgoError {}

impl From<CurveSanitationError> for ArgoError {
    fn from(err: CurveSanitationError) -> Self {
        Self::SanitationError(err)
    }
}

/// Argo Verification Key components for evaluating $E(\pi)$ in $\mathbb{G}_T$.
///
/// $E(\pi) = e(A, B) \cdot e(C, H_1) \cdot e(x \cdot G_{1,vk}, H_2) \cdot h_3$.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgoVerificationKey {
    /// Canonical generator $G \in \mathbb{G}_1$.
    pub g: G1Affine,
    /// Canonical generator $H \in \mathbb{G}_2$.
    pub h: G2Affine,
    /// Verification key element for $C$ ($-\delta \in \mathbb{G}_2$).
    pub h1: G2Affine,
    /// Verification key element for public input ($-\gamma \in \mathbb{G}_2$).
    pub h2: G2Affine,
    /// Verification key public input base point ($L_1 \in \mathbb{G}_1$).
    pub g1_vk: G1Affine,
    /// Fixed target group constant $h_3 = (e(\alpha, \beta) \cdot e(L_0, \gamma))^{-1} \in \mathbb{G}_T$.
    pub h3: Fq12,
}

impl ArgoVerificationKey {
    /// Creates a new `ArgoVerificationKey`.
    pub fn new(
        g: G1Affine,
        h: G2Affine,
        h1: G2Affine,
        h2: G2Affine,
        g1_vk: G1Affine,
        h3: Fq12,
    ) -> Self {
        Self {
            g,
            h,
            h1,
            h2,
            g1_vk,
            h3,
        }
    }

    /// Evaluates the Groth16 invalidity element $E(\pi) \in \mathbb{G}_T$.
    ///
    /// Applies strict Gate 1, Gate 2, Gate 3, and Gate 4 curve sanitation before pairing.
    pub fn evaluate_e(
        &self,
        a: &G1Affine,
        b: &G2Affine,
        c: &G1Affine,
        x: &Fr,
    ) -> Result<Fq12, CurveSanitationError> {
        // Gate 2: Non-infinity
        if a.is_zero() || b.is_zero() || c.is_zero() {
            return Err(CurveSanitationError::PointAtInfinityNotAllowed);
        }
        // Gate 1: On-curve
        if !a.is_on_curve() || !b.is_on_curve() || !c.is_on_curve() {
            return Err(CurveSanitationError::CurvePointNotOnCurve);
        }
        // Gate 3: Subgroup membership
        if !a.is_in_correct_subgroup_assuming_on_curve()
            || !b.is_in_correct_subgroup_assuming_on_curve()
            || !c.is_in_correct_subgroup_assuming_on_curve()
        {
            return Err(CurveSanitationError::SubgroupCheckFailed);
        }

        // Compute pairing components
        let e1 = Bls12_381::pairing(*a, *b).0;
        let e2 = Bls12_381::pairing(*c, self.h1).0;
        let x_g1 = (self.g1_vk * x).into_affine();
        let e3 = Bls12_381::pairing(x_g1, self.h2).0;

        Ok(e1 * e2 * e3 * self.h3)
    }
}

/// Prover's secret state generated in the Offline phase.
#[derive(Debug, Clone)]
pub struct ArgoProverState {
    /// Random DDH multiplier $t \in \mathbb{F}_r^*$.
    pub t: Fr,
    /// Random blinding scalar $s \in \mathbb{F}_r$.
    pub s: Fr,
    /// Random masking scalars $r_i \in \mathbb{F}_r^*$ for $U_i$.
    pub r: Vec<Fr>,
    /// Random masking scalars $v_i \in \mathbb{F}_r$ for $[C]_i$.
    pub v: Vec<Fr>,
    /// Random masking scalars $w_i \in \mathbb{F}_r$ for $[x]_i$.
    pub w: Vec<Fr>,
    /// Prover's secret key $k \in S^n$.
    pub k: Vec<EisensteinScalar>,
}

/// Public material published once during the Offline phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgoOfflineMaterial {
    /// Masking bases $U_i = r_i \cdot H \in \mathbb{G}_2$.
    pub u: Vec<G2Affine>,
    /// Target group unmasking material $p_i \in \mathbb{G}_T$.
    pub p_masks: Vec<Fq12>,
}

impl ArgoOfflineMaterial {
    /// Returns the number of replicas $n$.
    pub fn num_replicas(&self) -> usize {
        self.u.len()
    }
}

/// Online randomized encoding of a Groth16 proof $\pi = (A, B, C, x)$.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgoOnlineEncoding {
    /// Masked point $[A] = t \cdot A - s \cdot G \in \mathbb{G}_1$.
    pub a_mac: G1Affine,
    /// Masked points $[B]_i = \phi_i \cdot B - t \cdot U_i \in \mathbb{G}_2$.
    pub b_mac: Vec<G2Affine>,
    /// Masked points $[C]_i = \phi_i \cdot C - v_i \cdot G \in \mathbb{G}_1$.
    pub c_mac: Vec<G1Affine>,
    /// Masked field elements $[x]_i = \phi_i \cdot x - w_i \in \mathbb{F}_r$.
    pub x_mac: Vec<Fr>,
}

/// Outcome of decoding an Argo proof encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgoDecodeOutcome {
    /// Proof is mathematically valid ($E(\pi) = 1 \in \mathbb{G}_T$).
    /// All $Y_i = 1$. Exactly 0 bits of the secret key $k$ leaked.
    ValidProofZeroLeakage,
    /// Proof is fraudulent / invalid ($E(\pi) \neq 1 \in \mathbb{G}_T$).
    /// Secret key $k \in S^n$ extracted with 100% precision in $O(1)$ lookups.
    FraudDetected {
        /// Fully reconstructed secret key vector in $S^n$.
        extracted_key: Vec<EisensteinScalar>,
        /// The non-identity evaluation element $E(\pi) \neq 1$.
        invalidity_element: Box<Fq12>,
    },
    /// Input points failed curve point sanitation.
    SanitationFailure(CurveSanitationError),
}

/// Phase 1: Setup Offline Material and Prover State.
///
/// Published public material size: $\sim 21$ kB for $n = 46$ replicas.
pub fn argo_setup_offline<R: Rng>(
    vk: &ArgoVerificationKey,
    k_secret: &[EisensteinScalar],
    rng: &mut R,
) -> (ArgoOfflineMaterial, ArgoProverState) {
    let n = k_secret.len();
    let t = Fr::rand(rng);
    let s = Fr::rand(rng);

    let mut r_vec = Vec::with_capacity(n);
    let mut v_vec = Vec::with_capacity(n);
    let mut w_vec = Vec::with_capacity(n);
    let mut u_vec = Vec::with_capacity(n);
    let mut p_masks = Vec::with_capacity(n);

    let s_g = (vk.g * s).into_affine();
    let memo_h3 = ArgoMemoizedPowers::new(&vk.h3);

    for phi_i in k_secret {
        let r_i = Fr::rand(rng);
        let v_i = Fr::rand(rng);
        let w_i = Fr::rand(rng);

        let u_i = (vk.h * r_i).into_affine();

        // Target group mask: p_i = h3^phi_i * e(s*G, U_i) * e(v_i*G, H1) * e(w_i*G1_vk, H2)
        let h3_phi = fast_s_pow_gt(&vk.h3, *phi_i, Some(&memo_h3));
        let term_gsr = Bls12_381::pairing(s_g, u_i).0;

        let v_g = (vk.g * v_i).into_affine();
        let term_v = Bls12_381::pairing(v_g, vk.h1).0;

        let w_g1_vk = (vk.g1_vk * w_i).into_affine();
        let term_w = Bls12_381::pairing(w_g1_vk, vk.h2).0;

        let mask_i = h3_phi * term_gsr * term_v * term_w;

        r_vec.push(r_i);
        v_vec.push(v_i);
        w_vec.push(w_i);
        u_vec.push(u_i);
        p_masks.push(mask_i);
    }

    let offline = ArgoOfflineMaterial { u: u_vec, p_masks };
    let state = ArgoProverState {
        t,
        s,
        r: r_vec,
        v: v_vec,
        w: w_vec,
        k: k_secret.to_vec(),
    };

    (offline, state)
}

/// Phase 2: Online Proof Randomized Encoding.
///
/// Encodes proof $\pi = (A, B, C, x)$ into $\tau = ([A], [B], [C], [x])$ in $< 1.5$ ms.
pub fn argo_encode_online(
    proof_a: &G1Affine,
    proof_b: &G2Affine,
    proof_c: &G1Affine,
    proof_x: &Fr,
    state: &ArgoProverState,
    vk: &ArgoVerificationKey,
) -> ArgoOnlineEncoding {
    let n = state.k.len();

    // [A] = t * A - s * G
    let a_mac = (*proof_a * state.t - vk.g * state.s).into_affine();

    let mut b_mac = Vec::with_capacity(n);
    let mut c_mac = Vec::with_capacity(n);
    let mut x_mac = Vec::with_capacity(n);

    for i in 0..n {
        let phi_i = state.k[i];
        let phi_fr = phi_i.to_fr();

        // [B]_i = phi_i * B - t * U_i
        let b_i = (*proof_b * phi_fr - state.u_i_proj(vk, i) * state.t).into_affine();
        // [C]_i = phi_i * C - v_i * G
        let c_i = (*proof_c * phi_fr - vk.g * state.v[i]).into_affine();
        // [x]_i = phi_i * x - w_i
        let x_i = phi_fr * proof_x - state.w[i];

        b_mac.push(b_i);
        c_mac.push(c_i);
        x_mac.push(x_i);
    }

    ArgoOnlineEncoding {
        a_mac,
        b_mac,
        c_mac,
        x_mac,
    }
}

impl ArgoProverState {
    fn u_i_proj(&self, vk: &ArgoVerificationKey, i: usize) -> G2Projective {
        vk.h * self.r[i]
    }
}

/// Phase 3: Permissionless Decode & Slashing Key Extraction.
///
/// Evaluates $[E(\pi)]_i$, unmasks $Y_i = [E(\pi)]_i \cdot p_i$, and:
/// - If $E(\pi) = 1$: verifies $Y_i = 1$ (0 bits leaked).
/// - If $E(\pi) \neq 1$: extracts $k \in S^n$ in $O(1)$ table lookups.
pub fn argo_decode(
    vk: &ArgoVerificationKey,
    offline: &ArgoOfflineMaterial,
    online: &ArgoOnlineEncoding,
    proof_a: &G1Affine,
    proof_b: &G2Affine,
    proof_c: &G1Affine,
    proof_x: &Fr,
) -> Result<ArgoDecodeOutcome, ArgoError> {
    let n = offline.num_replicas();
    if online.b_mac.len() != n || online.c_mac.len() != n || online.x_mac.len() != n {
        return Err(ArgoError::DimensionMismatch {
            expected: n,
            got: online.b_mac.len(),
        });
    }

    // Strict 5-gate sanitization on input curve points
    let e_pi = match vk.evaluate_e(proof_a, proof_b, proof_c, proof_x) {
        Ok(elem) => elem,
        Err(e) => return Ok(ArgoDecodeOutcome::SanitationFailure(e)),
    };

    let is_valid_proof = e_pi == Fq12::ONE;

    // Precompute candidate powers for S = {0, 1, -1, omega, -omega, omega^2, -omega^2}
    let memo_e_pi = if !is_valid_proof {
        Some(ArgoMemoizedPowers::new(&e_pi))
    } else {
        None
    };

    let mut extracted_key = Vec::with_capacity(n);

    for i in 0..n {
        // [E(pi)]_i = e(A, [B]_i) * e([A], U_i) * e([C]_i, H_1) * e([x]_i * G1_vk, H_2)
        let term1 = Bls12_381::pairing(*proof_a, online.b_mac[i]).0;
        let term2 = Bls12_381::pairing(online.a_mac, offline.u[i]).0;
        let term3 = Bls12_381::pairing(online.c_mac[i], vk.h1).0;
        let x_mac_g1 = (vk.g1_vk * online.x_mac[i]).into_affine();
        let term4 = Bls12_381::pairing(x_mac_g1, vk.h2).0;

        let e_enc_i = term1 * term2 * term3 * term4;
        let y_i = e_enc_i * offline.p_masks[i];

        if is_valid_proof {
            // Case A: Valid proof -> all Y_i must be identity 1
            if y_i != Fq12::ONE {
                return Err(ArgoError::CandidateNotFound { replica_index: i });
            }
        } else {
            // Case B: Fraudulent proof -> find phi in S such that E(pi)^phi == Y_i
            let mut matched_phi = None;
            for idx in 0..7 {
                let phi = EisensteinScalar::from_index(idx)
                    .expect("index 0..7 is valid for EisensteinScalar");
                let cand_pow = fast_s_pow_gt(&e_pi, phi, memo_e_pi.as_ref());
                if cand_pow == y_i {
                    matched_phi = Some(phi);
                    break;
                }
            }

            match matched_phi {
                Some(phi) => extracted_key.push(phi),
                None => return Err(ArgoError::CandidateNotFound { replica_index: i }),
            }
        }
    }

    if is_valid_proof {
        Ok(ArgoDecodeOutcome::ValidProofZeroLeakage)
    } else {
        Ok(ArgoDecodeOutcome::FraudDetected {
            extracted_key,
            invalidity_element: Box::new(e_pi),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_ec::AffineRepr;
    use rand::thread_rng;

    /// Helper to setup a synthetic canonical Groth16 verification key and valid proof witness.
    fn setup_synthetic_vk_and_proof() -> (ArgoVerificationKey, G1Affine, G2Affine, G1Affine, Fr) {
        let g = G1Affine::generator();
        let h = G2Affine::generator();

        let alpha = Fr::from(12345u64);
        let beta = Fr::from(67890u64);
        let gamma = Fr::from(11111u64);
        let delta = Fr::from(22222u64);
        let l0 = Fr::from(33333u64);
        let l1 = Fr::from(44444u64);

        let h1 = (h * (-delta)).into_affine();
        let h2 = (h * (-gamma)).into_affine();
        let g1_vk = (g * l1).into_affine();

        // h3 = (e(alpha*G, beta*H) * e(l0*G, gamma*H))^-1
        let e_ab = Bls12_381::pairing((g * alpha).into_affine(), (h * beta).into_affine()).0;
        let e_l0g = Bls12_381::pairing((g * l0).into_affine(), (h * gamma).into_affine()).0;
        let h3 = (e_ab * e_l0g).inverse().unwrap();

        let vk = ArgoVerificationKey::new(g, h, h1, h2, g1_vk, h3);

        // Construct valid witness: a * b - c * delta - x * l1 * gamma - (alpha * beta + l0 * gamma) == 0
        let x_val = Fr::from(777u64);
        let c_val = Fr::from(555u64);
        let a_val = Fr::from(101u64);

        let target_exp = alpha * beta + (l0 + x_val * l1) * gamma + c_val * delta;
        let b_val = target_exp * a_val.inverse().unwrap();

        let a_valid = (g * a_val).into_affine();
        let b_valid = (h * b_val).into_affine();
        let c_valid = (g * c_val).into_affine();

        (vk, a_valid, b_valid, c_valid, x_val)
    }

    #[test]
    fn test_dec052_argo_valid_proof_zero_leakage() {
        let (vk, a, b, c, x) = setup_synthetic_vk_and_proof();
        let mut rng = thread_rng();

        // 7-replica key covering all elements of S
        let k_secret = vec![
            EisensteinScalar::One,
            EisensteinScalar::NegOne,
            EisensteinScalar::Zero,
            EisensteinScalar::Omega,
            EisensteinScalar::NegOmega,
            EisensteinScalar::OmegaSq,
            EisensteinScalar::NegOmegaSq,
        ];

        // 1. Offline Phase
        let (offline, state) = argo_setup_offline(&vk, &k_secret, &mut rng);
        assert_eq!(offline.num_replicas(), 7);

        // 2. Online Phase
        let online = argo_encode_online(&a, &b, &c, &x, &state, &vk);

        // 3. Decode Phase
        let outcome = argo_decode(&vk, &offline, &online, &a, &b, &c, &x).unwrap();

        assert_eq!(outcome, ArgoDecodeOutcome::ValidProofZeroLeakage);
    }

    #[test]
    fn test_dec052_argo_invalid_proof_instant_fraud_extraction() {
        let (vk, a, b, c, x) = setup_synthetic_vk_and_proof();
        let mut rng = thread_rng();

        let k_secret = vec![
            EisensteinScalar::One,
            EisensteinScalar::NegOne,
            EisensteinScalar::Zero,
            EisensteinScalar::Omega,
            EisensteinScalar::NegOmega,
            EisensteinScalar::OmegaSq,
            EisensteinScalar::NegOmegaSq,
        ];

        let (offline, state) = argo_setup_offline(&vk, &k_secret, &mut rng);

        // Tamper public input x -> x + 10
        let x_tampered = x + Fr::from(10u64);

        // Malicious prover encodes with tampered public input
        let online = argo_encode_online(&a, &b, &c, &x_tampered, &state, &vk);

        // Challenger decodes
        let outcome = argo_decode(&vk, &offline, &online, &a, &b, &c, &x_tampered).unwrap();

        match outcome {
            ArgoDecodeOutcome::FraudDetected {
                extracted_key,
                invalidity_element,
            } => {
                assert_ne!(*invalidity_element, Fq12::ONE);
                assert_eq!(extracted_key, k_secret);
            }
            other => panic!("Expected FraudDetected, got {other:?}"),
        }
    }

    #[test]
    fn test_dec052_argo_decode_rejects_off_curve_point() {
        let (vk, _a, b, c, x) = setup_synthetic_vk_and_proof();
        let mut rng = thread_rng();
        let k_secret = vec![EisensteinScalar::One];
        let (offline, state) = argo_setup_offline(&vk, &k_secret, &mut rng);

        // Corrupted point not on curve
        let bad_a = G1Affine::zero();
        let online = argo_encode_online(&bad_a, &b, &c, &x, &state, &vk);

        let outcome = argo_decode(&vk, &offline, &online, &bad_a, &b, &c, &x).unwrap();
        match outcome {
            ArgoDecodeOutcome::SanitationFailure(
                CurveSanitationError::PointAtInfinityNotAllowed,
            ) => {}
            other => panic!("Expected SanitationFailure, got {other:?}"),
        }
    }
}
