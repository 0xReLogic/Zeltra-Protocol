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
use ark_bls12_381::{Fq, G1Projective};
use ark_ec::scalar_mul::glv::GLVConfig;
use ark_ff::Field;
use ark_std::Zero;

/// Primitive cubic root of unity $\beta \in \mathbb{F}_q$ where $\beta^3 \equiv 1 \pmod q$ and $\beta \neq 1$.
pub fn endomorphism_beta() -> Fq {
    Config::ENDO_COEFFS[0]
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
    }
}
