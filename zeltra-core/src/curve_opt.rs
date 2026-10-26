//! Accelerated Eisenstein Endomorphism & Fast Point Tripling on BLS12-381 (G1)
//!
//! Based on mathematical formulas from IACR ePrint 2024/1906 (Proposition 3.1):
//! - Guangwu Xu, Wei Yu, Ke Han, and Pengfei Lu (September 2026).
//!
//! Provides ultra-fast Jacobian projective curve operations for curves with j-invariant 0 ($y^2 = x^3 + b$, $b = 4$):
//! - `fast_phi_p`: Endomorphism $\Phi(P) = (\beta x, y) = [\omega]P$ in $1M$.
//! - `fast_tau_p`: Minimal norm Eisenstein endomorphism $\tau P = P - \omega P$ in $3S + 3M$.
//! - `fast_triple_p`: World-record point tripling $3P = (1 - \omega^2)(\tau P)$ in $4S + 6M$ (replaces $10S + 5M$).

use ark_bls12_381::g1::Config;
use ark_bls12_381::{Fq, Fr, G1Affine, G1Projective};
use ark_ec::scalar_mul::glv::GLVConfig;
use ark_ec::AffineRepr;
use ark_ff::{AdditiveGroup, Field, PrimeField};
use ark_std::Zero;

/// Primitive cubic root of unity $\beta \in \mathbb{F}_q$ where $\beta^3 \equiv 1 \pmod q$ and $\beta \neq 1$.
pub fn endomorphism_beta() -> Fq {
    Config::ENDO_COEFFS[0]
}

/// Primitive cubic root of unity $\omega \in \mathbb{F}_r$ where $\omega^3 \equiv 1 \pmod r$ and $\omega \neq 1$.
///
/// In BLS12-381, this corresponds to the GLV endomorphism scalar $\lambda \in \mathbb{F}_r$ such that
/// $\Phi(P) = (\beta x, y) = [\lambda]P$.
pub fn scalar_omega() -> Fr {
    Config::LAMBDA
}

/// Primitive cubic root of unity squared $\omega^2 \in \mathbb{F}_r$ where $\omega^2 \equiv -1 - \omega \pmod r$.
pub fn scalar_omega_sq() -> Fr {
    Config::LAMBDA * Config::LAMBDA
}

/// Computes the first-order endomorphism $\Phi(P) = (\beta X : Y : Z)$ in Jacobian coordinates.
///
/// Cost: exactly 1 field multiplication ($1M$).
#[inline]
pub fn fast_phi_p(p: &G1Projective) -> G1Projective {
    if p.is_zero() {
        return G1Projective::zero();
    }
    let beta = endomorphism_beta();
    G1Projective::new_unchecked(p.x * beta, p.y, p.z)
}

/// Computes the second-order endomorphism $\Phi^2(P) = (\beta^2 X : Y : Z)$ in Jacobian coordinates.
///
/// Cost: exactly 1 field multiplication ($1M$) with precomputed $\beta^2$.
#[inline]
pub fn fast_phi_sq_p(p: &G1Projective) -> G1Projective {
    if p.is_zero() {
        return G1Projective::zero();
    }
    let beta_sq = endomorphism_beta().square();
    G1Projective::new_unchecked(p.x * beta_sq, p.y, p.z)
}

/// The Eisenstein endomorphism scalar set $S = \{0, 1, -1, \omega, -\omega, \omega^2, -\omega^2\}$ ($|S| = 7$).
///
/// Under DEC-052 / Argo (IACR 2026/2165), $S$ defines the alphabet of unit multipliers for randomized encoding
/// of Groth16 invalidity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EisensteinScalar {
    Zero,
    One,
    NegOne,
    Omega,
    NegOmega,
    OmegaSq,
    NegOmegaSq,
}

impl EisensteinScalar {
    pub const ALL: [EisensteinScalar; 7] = [
        EisensteinScalar::Zero,
        EisensteinScalar::One,
        EisensteinScalar::NegOne,
        EisensteinScalar::Omega,
        EisensteinScalar::NegOmega,
        EisensteinScalar::OmegaSq,
        EisensteinScalar::NegOmegaSq,
    ];

    /// Returns the corresponding scalar field element in $\mathbb{F}_r$.
    pub fn to_fr(&self) -> Fr {
        let w = scalar_omega();
        let w2 = scalar_omega_sq();
        match self {
            Self::Zero => Fr::ZERO,
            Self::One => Fr::ONE,
            Self::NegOne => -Fr::ONE,
            Self::Omega => w,
            Self::NegOmega => -w,
            Self::OmegaSq => w2,
            Self::NegOmegaSq => -w2,
        }
    }

    /// Converts an index in $0..7$ to an EisensteinScalar.
    pub fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(Self::Zero),
            1 => Some(Self::One),
            2 => Some(Self::NegOne),
            3 => Some(Self::Omega),
            4 => Some(Self::NegOmega),
            5 => Some(Self::OmegaSq),
            6 => Some(Self::NegOmegaSq),
            _ => None,
        }
    }

    /// Returns the index of this EisensteinScalar in $0..7$.
    pub fn index(&self) -> usize {
        match self {
            Self::Zero => 0,
            Self::One => 1,
            Self::NegOne => 2,
            Self::Omega => 3,
            Self::NegOmega => 4,
            Self::OmegaSq => 5,
            Self::NegOmegaSq => 6,
        }
    }

    /// Evaluates $\phi \cdot P$ in constant time on G1 Projective.
    /// Uses endomorphism $\Phi(x, y) = (\beta x, y)$ which takes at most 1 field multiplication.
    pub fn evaluate_g1(&self, p: &G1Projective) -> G1Projective {
        if p.is_zero() {
            return G1Projective::zero();
        }
        match self {
            Self::Zero => G1Projective::zero(),
            Self::One => *p,
            Self::NegOne => -*p,
            Self::Omega => fast_phi_p(p),
            Self::NegOmega => -fast_phi_p(p),
            Self::OmegaSq => fast_phi_sq_p(p),
            Self::NegOmegaSq => -fast_phi_sq_p(p),
        }
    }

    /// Evaluates $\phi \cdot P$ on G1 Affine in constant time ($O(1)$).
    pub fn evaluate_g1_affine(&self, p: &G1Affine) -> G1Affine {
        if p.is_zero() {
            return G1Affine::zero();
        }
        let beta = endomorphism_beta();
        let beta_sq = beta.square();
        match self {
            Self::Zero => G1Affine::zero(),
            Self::One => *p,
            Self::NegOne => -*p,
            Self::Omega => G1Affine::new_unchecked(p.x * beta, p.y),
            Self::NegOmega => G1Affine::new_unchecked(p.x * beta, -p.y),
            Self::OmegaSq => G1Affine::new_unchecked(p.x * beta_sq, p.y),
            Self::NegOmegaSq => G1Affine::new_unchecked(p.x * beta_sq, -p.y),
        }
    }
}

/// Precomputed memoized powers for Argo discrete log decoding in GT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgoMemoizedPowers {
    pub w: ark_bls12_381::Fq12,
    pub w2: ark_bls12_381::Fq12,
}

impl ArgoMemoizedPowers {
    pub fn new(base: &ark_bls12_381::Fq12) -> Self {
        let omega = scalar_omega();
        let omega_sq = scalar_omega_sq();
        let w = base.pow(omega.into_bigint());
        let w2 = base.pow(omega_sq.into_bigint());
        Self { w, w2 }
    }
}

/// Evaluates $\text{base}^\phi$ in GT for $\phi \in S$ using memoized powers in $O(1)$.
///
/// Does NOT perform slow 255-bit field exponentiations when `memo` is provided.
pub fn fast_s_pow_gt(
    base: &ark_bls12_381::Fq12,
    phi: EisensteinScalar,
    memo: Option<&ArgoMemoizedPowers>,
) -> ark_bls12_381::Fq12 {
    match phi {
        EisensteinScalar::Zero => ark_bls12_381::Fq12::ONE,
        EisensteinScalar::One => *base,
        EisensteinScalar::NegOne => base.inverse().unwrap_or(ark_bls12_381::Fq12::ZERO),
        EisensteinScalar::Omega => {
            if let Some(m) = memo {
                m.w
            } else {
                base.pow(scalar_omega().into_bigint())
            }
        }
        EisensteinScalar::NegOmega => {
            let w = if let Some(m) = memo {
                m.w
            } else {
                base.pow(scalar_omega().into_bigint())
            };
            w.inverse().unwrap_or(ark_bls12_381::Fq12::ZERO)
        }
        EisensteinScalar::OmegaSq => {
            if let Some(m) = memo {
                m.w2
            } else {
                base.pow(scalar_omega_sq().into_bigint())
            }
        }
        EisensteinScalar::NegOmegaSq => {
            let w2 = if let Some(m) = memo {
                m.w2
            } else {
                base.pow(scalar_omega_sq().into_bigint())
            };
            w2.inverse().unwrap_or(ark_bls12_381::Fq12::ZERO)
        }
    }
}

/// Computes the Eisenstein minimal-norm endomorphism $\tau P = P - \omega P$ where $\tau = 1 - \omega$.
///
/// Evaluates in Jacobian projective coordinates using Proposition 3.1:
/// - $X_\tau = 4Y^2 - 3X^3$
/// - $Y_\tau = Y(-2X_\tau + 3X^3)$
/// - $Z_\tau = (1 - \beta)XZ$
///
/// Theoretical Cost: exactly $3S + 3M$ (cheaper than point doubling $5S + 2M$).
pub fn fast_tau_p(p: &G1Projective) -> G1Projective {
    if p.is_zero() {
        return G1Projective::zero();
    }
    let beta = endomorphism_beta();
    let two = Fq::from(2u64);
    let three = Fq::from(3u64);
    let four = Fq::from(4u64);

    let x = p.x;
    let y = p.y;
    let z = p.z;

    let y2 = y.square();
    let x2 = x.square();
    let x3 = x * x2;

    let x_tau = four * y2 - three * x3;
    let y_tau = y * (-two * x_tau + three * x3);
    let z_tau = (Fq::ONE - beta) * x * z;

    G1Projective::new_unchecked(x_tau, y_tau, z_tau)
}

/// Computes ultra-fast point tripling $3P = (1 - \omega^2)(\tau P)$ in Jacobian coordinates.
///
/// Evaluates via Proposition 3.1 without explicitly needing $Z_\tau$:
/// - $X_3 = 4Y_\tau^2 - 3X_\tau^3$
/// - $Y_3 = Y_\tau(-2X_3 + 3X_\tau^3)$
/// - $Z_3 = 3X_\tau(XZ)$
///
/// Theoretical Cost: exactly $4S + 6M$ (replaces double-and-add $10S + 5M$).
pub fn fast_triple_p(p: &G1Projective) -> G1Projective {
    if p.is_zero() {
        return G1Projective::zero();
    }
    let two = Fq::from(2u64);
    let three = Fq::from(3u64);
    let four = Fq::from(4u64);

    let x = p.x;
    let y = p.y;
    let z = p.z;

    // Step 1: Intermediate tau-evaluation
    let y2 = y.square();
    let x2 = x.square();
    let x3 = x * x2;

    let x_tau = four * y2 - three * x3;
    let y_tau = y * (-two * x_tau + three * x3);

    // Step 2: Tripling from tau point
    let y_tau2 = y_tau.square();
    let x_tau2 = x_tau.square();
    let x_tau3 = x_tau * x_tau2;

    let x3_fast = four * y_tau2 - three * x_tau3;
    let y3_fast = y_tau * (-two * x3_fast + three * x_tau3);
    let z3_fast = three * x_tau * x * z;

    G1Projective::new_unchecked(x3_fast, y3_fast, z3_fast)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bls12_381::{G1Affine, G1Projective};
    use ark_ec::{AffineRepr, CurveGroup};
    use ark_std::UniformRand;

    #[test]
    fn test_kat_fast_phi_generator() {
        let g = G1Affine::generator().into_group();
        let phi_g = fast_phi_p(&g);
        let mut expected = g;
        expected.x *= endomorphism_beta();
        assert_eq!(phi_g.into_affine(), expected.into_affine());
    }

    #[test]
    fn test_kat_fast_tau_generator() {
        let g = G1Affine::generator().into_group();
        let tau_g = fast_tau_p(&g);
        let phi_g = fast_phi_p(&g);
        let expected = g - phi_g;
        assert_eq!(tau_g.into_affine(), expected.into_affine());
    }

    #[test]
    fn test_kat_fast_triple_generator() {
        let g = G1Affine::generator().into_group();
        let triple_g = fast_triple_p(&g);
        let expected = g + g + g;
        assert_eq!(triple_g.into_affine(), expected.into_affine());
    }

    #[test]
    fn test_fast_triple_random_points() {
        let mut rng = ark_std::rand::thread_rng();
        for _ in 0..50 {
            let p = G1Projective::rand(&mut rng);
            let triple_p = fast_triple_p(&p);
            let expected = p + p + p;
            assert_eq!(triple_p.into_affine(), expected.into_affine());
        }
    }

    #[test]
    fn test_fast_tau_random_points() {
        let mut rng = ark_std::rand::thread_rng();
        for _ in 0..50 {
            let p = G1Projective::rand(&mut rng);
            let tau_p = fast_tau_p(&p);
            let expected = p - fast_phi_p(&p);
            assert_eq!(tau_p.into_affine(), expected.into_affine());
        }
    }

    #[test]
    fn test_zero_point_fail_closed() {
        let zero = G1Projective::zero();
        assert!(fast_phi_p(&zero).is_zero());
        assert!(fast_tau_p(&zero).is_zero());
        assert!(fast_triple_p(&zero).is_zero());
        assert!(fast_phi_sq_p(&zero).is_zero());
    }

    #[test]
    fn test_scalar_omega_cubic_roots_properties() {
        let omega = scalar_omega();
        let omega_sq = scalar_omega_sq();

        assert_ne!(omega, Fr::ONE, "Omega must be primitive (not equal to 1)");
        assert_ne!(omega_sq, Fr::ONE, "Omega^2 must not equal 1");
        assert_ne!(omega, omega_sq, "Omega and Omega^2 must be distinct");

        // omega^3 == 1
        assert_eq!(omega * omega * omega, Fr::ONE);
        assert_eq!(omega_sq * omega, Fr::ONE);

        // omega^2 + omega + 1 == 0
        assert_eq!(omega_sq + omega + Fr::ONE, Fr::ZERO);

        // All 7 elements in S must be distinct in Fr
        let s_elements = EisensteinScalar::ALL.map(|s| s.to_fr());
        for i in 0..7 {
            for j in (i + 1)..7 {
                assert_ne!(
                    s_elements[i], s_elements[j],
                    "Elements {} and {} in S must be strictly distinct in Fr",
                    i, j
                );
            }
        }
    }

    #[test]
    fn test_eisenstein_scalar_evaluation_on_g1() {
        let mut rng = ark_std::rand::thread_rng();
        let p_proj = G1Projective::rand(&mut rng);
        let p_aff = p_proj.into_affine();

        for s in EisensteinScalar::ALL {
            let fr_val = s.to_fr();
            let expected_proj = p_proj * fr_val;

            let eval_proj = s.evaluate_g1(&p_proj);
            assert_eq!(eval_proj.into_affine(), expected_proj.into_affine());

            let eval_aff = s.evaluate_g1_affine(&p_aff);
            assert_eq!(eval_aff, expected_proj.into_affine());
        }
    }

    #[test]
    fn test_fast_s_pow_gt_memoization() {
        use ark_bls12_381::{Bls12_381, G2Affine};
        use ark_ec::pairing::Pairing;

        // Generate GT element from pairing of random generators
        let g1 = G1Affine::generator();
        let g2 = G2Affine::generator();
        let base_gt = Bls12_381::pairing(g1, g2).0;

        let memo = ArgoMemoizedPowers::new(&base_gt);

        for s in EisensteinScalar::ALL {
            let direct = fast_s_pow_gt(&base_gt, s, None);
            let memoized = fast_s_pow_gt(&base_gt, s, Some(&memo));
            assert_eq!(direct, memoized, "Fast S pow mismatch for scalar {:?}", s);
        }
    }
}
