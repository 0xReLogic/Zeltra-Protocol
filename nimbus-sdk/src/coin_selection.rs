//! Multi-UTXO Stochastic Knapsack Coin Selection (DEC-036B / Task B Phase 1)
//!
//! Academic Foundations & References:
//! - OXT Research & Samourai Wallet (2020–2026): "A Statement on Two Discovered Vulnerabilities in Wasabi Wallet:
//!   Toxic Recall and Change Clustering Heuristics".
//! - Zcash ZIP 315 (2021–2024): "Best Practices for Wallet Implementations — Reducing Transaction Linkability and Arity Leakage".
//! - ACM CCS 2024/2025: "Attacking Anonymity Set in Tornado Cash via Wallet Fingerprints" (Stochastic bucket coin selection).
//!
//! Provides a 4-tier hierarchical selection pipeline:
//! 1. Tier 1: Exact match single note (v = T, zero change).
//! 2. Tier 2: Smallest sufficient single note (v > T, single change).
//! 3. Tier 3: 2-note combination via Stochastic Knapsack optimization with entropy noise.
//! 4. Tier 4: Fragmentation lockout error advising `consolidate_notes()`.
//!
//! Includes Economic Dust Floor absorption (< 0.10 USDC absorbed into execution fee markup).

use crate::wallet::note_wallet::{
    NoteEpochStatus, NoteStatus, SelectedSpend, WalletError, WalletNote,
};
use rand::Rng;

/// Minimum non-dust economic change threshold (100,000 base units = 0.10 USDC, DEC-036B).
/// Any change amount strictly less than this threshold is absorbed into the relayer execution fee markup
/// to prevent creating unspendable dust notes and linking transaction clusters.
pub const MIN_DUST_THRESHOLD: u64 = 100_000;

/// Configuration weights for Stochastic Knapsack cost evaluation (DEC-036B Section 4).
#[derive(Clone, Copy, Debug)]
pub struct KnapsackConfig {
    /// Denomination parity penalty weight: alpha * |v_i - v_j|
    pub alpha: f64,
    /// Age preference bonus weight: beta * (epoch differences)
    pub beta: f64,
    /// Maximum random noise entropy sigma: epsilon ~ Uniform(0, sigma)
    pub noise_sigma: f64,
}

impl Default for KnapsackConfig {
    fn default() -> Self {
        Self {
            alpha: 0.5,
            beta: 50_000.0,
            noise_sigma: 10_000.0,
        }
    }
}

/// Evaluates the privacy and efficiency penalty cost of a candidate pair of notes (n_i, n_j)
/// for a target spend amount T.
///
/// Cost(i, j) = (v_i + v_j - T) + alpha * |v_i - v_j| - beta * AgeBonus(n_i, n_j) + epsilon
#[allow(clippy::too_many_arguments)]
pub fn evaluate_pair_cost(
    val_i: u64,
    val_j: u64,
    epoch_i: u32,
    epoch_j: u32,
    target: u64,
    current_epoch: u32,
    config: &KnapsackConfig,
    noise: f64,
) -> f64 {
    let sum = val_i + val_j;
    let excess = sum.saturating_sub(target) as f64;
    let denomination_diff = (val_i as f64 - val_j as f64).abs();
    let parity_penalty = config.alpha * denomination_diff;

    // Prioritize spending older epoch notes (E-1) over current epoch (E)
    let age_i = current_epoch.saturating_sub(epoch_i) as f64;
    let age_j = current_epoch.saturating_sub(epoch_j) as f64;
    let age_bonus = config.beta * (age_i + age_j);

    (excess + parity_penalty - age_bonus + noise).max(0.0)
}

/// Executes the 4-Tier Stochastic Knapsack Coin Selection pipeline (DEC-036B).
///
/// # Arguments
/// * `candidates` - List of references to candidate wallet notes.
/// * `merchant_amount` - Net USDC payout to merchant.
/// * `protocol_fee` - Protocol fee (flat 45 bps).
/// * `execution_fee` - Relayer gas reimbursement fee.
/// * `current_time_secs` - Current timestamp for reservation lease expiry checks.
/// * `current_epoch` - Current active epoch ID for rollover eligibility.
/// * `config` - Knapsack tuning parameters.
pub fn select_notes_stochastic_knapsack<'a>(
    candidates: &'a [&'a WalletNote],
    merchant_amount: u64,
    protocol_fee: u64,
    execution_fee: u64,
    current_time_secs: u64,
    current_epoch: u32,
    config: &KnapsackConfig,
) -> Result<SelectedSpend, WalletError> {
    let total_required = merchant_amount
        .checked_add(protocol_fee)
        .and_then(|v| v.checked_add(execution_fee))
        .ok_or_else(|| WalletError::CryptoError("Total required spend amount overflow".into()))?;

    // Filter spendable candidates:
    // 1. Must have valid Merkle witness / leaf index
    // 2. Status must be Unspent or expired Reserved
    // 3. Must not be ExpiredRequiresRollover (DEC-036B B1.4)
    let spendable: Vec<&'a WalletNote> = candidates
        .iter()
        .copied()
        .filter(|n| {
            if n.merkle_path_hex.is_none() || n.leaf_index.is_none() {
                return false;
            }
            if n.check_epoch_status(current_epoch) == NoteEpochStatus::ExpiredRequiresRollover {
                return false;
            }
            match &n.status {
                NoteStatus::Unspent => true,
                NoteStatus::Reserved {
                    lease_expiry_secs, ..
                } => *lease_expiry_secs <= current_time_secs,
                _ => false,
            }
        })
        .collect();

    let total_available: u64 = spendable.iter().map(|n| n.value).sum();

    // Tier 1: Exact Match Single Note (v == total_required)
    if let Some(exact_note) = spendable.iter().find(|n| n.value == total_required) {
        return Ok(SelectedSpend {
            input_commitment_hex: exact_note.commitment_hex.clone(),
            input_value: exact_note.value,
            second_input_commitment_hex: None,
            second_input_value: 0,
            merchant_amount,
            protocol_fee,
            execution_fee,
            total_required,
            change_amount: 0,
            has_change: false,
        });
    }

    // Tier 2: Smallest Single Note (v > total_required)
    let mut single_notes: Vec<&'a WalletNote> = spendable
        .iter()
        .copied()
        .filter(|n| n.value > total_required)
        .collect();

    if !single_notes.is_empty() {
        single_notes.sort_by_key(|n| n.value);
        let selected_note = single_notes[0];
        let raw_change = selected_note.value - total_required;

        // Economic Dust Floor Absorption (B1.3)
        let (adj_exec_fee, change_amount, has_change) = if raw_change < MIN_DUST_THRESHOLD {
            (execution_fee + raw_change, 0u64, false)
        } else {
            (execution_fee, raw_change, true)
        };

        return Ok(SelectedSpend {
            input_commitment_hex: selected_note.commitment_hex.clone(),
            input_value: selected_note.value,
            second_input_commitment_hex: None,
            second_input_value: 0,
            merchant_amount,
            protocol_fee,
            execution_fee: adj_exec_fee,
            total_required: selected_note.value - change_amount,
            change_amount,
            has_change,
        });
    }

    // Tier 3: 2-Note Combination via Stochastic Knapsack (v_i + v_j >= total_required)
    let n = spendable.len();
    let mut pair_candidates: Vec<(usize, usize, f64, u64)> = Vec::new(); // (i, j, cost, sum)

    let mut rng = rand::thread_rng();

    for i in 0..n {
        for j in (i + 1)..n {
            if let Some(sum) = spendable[i].value.checked_add(spendable[j].value) {
                if sum >= total_required {
                    let noise = if config.noise_sigma > 0.0 {
                        rng.gen_range(0.0..config.noise_sigma)
                    } else {
                        0.0
                    };
                    let cost = evaluate_pair_cost(
                        spendable[i].value,
                        spendable[j].value,
                        spendable[i].epoch(),
                        spendable[j].epoch(),
                        total_required,
                        current_epoch,
                        config,
                        noise,
                    );
                    pair_candidates.push((i, j, cost, sum));
                }
            }
        }
    }

    if !pair_candidates.is_empty() {
        // Choose the pair that minimizes the stochastic cost
        pair_candidates.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        let chosen = &pair_candidates[0];

        let note_1 = spendable[chosen.0];
        let note_2 = spendable[chosen.1];
        let sum = chosen.3;
        let raw_change = sum - total_required;

        // Economic Dust Floor Absorption (B1.3)
        let (adj_exec_fee, change_amount, has_change) = if raw_change < MIN_DUST_THRESHOLD {
            (execution_fee + raw_change, 0u64, false)
        } else {
            (execution_fee, raw_change, true)
        };

        return Ok(SelectedSpend {
            input_commitment_hex: note_1.commitment_hex.clone(),
            input_value: note_1.value,
            second_input_commitment_hex: Some(note_2.commitment_hex.clone()),
            second_input_value: note_2.value,
            merchant_amount,
            protocol_fee,
            execution_fee: adj_exec_fee,
            total_required: sum - change_amount,
            change_amount,
            has_change,
        });
    }

    // Tier 4: Balance Fragmentation Lockout or Insufficient Balance
    if total_available >= total_required {
        Err(WalletError::FragmentationLockout {
            requested: total_required,
            available: total_available,
            note_count: spendable.len(),
        })
    } else {
        Err(WalletError::InsufficientBalance {
            requested: total_required,
            available: total_available,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_note(cm: &str, val: u64, epoch: u32, status: NoteStatus) -> WalletNote {
        WalletNote {
            commitment_hex: cm.to_string(),
            value: val,
            owner_key_hex: "0x01".to_string(),
            rho_hex: "0x02".to_string(),
            randomness_hex: "0x03".to_string(),
            leaf_index: Some(0),
            leaf_count: Some(1),
            epoch_id: Some(epoch),
            merkle_path_hex: Some(vec!["0x04".to_string(); 20]),
            status,
            created_at_secs: 1000,
            session_id: None,
        }
    }

    #[test]
    fn test_tier1_exact_match() {
        let n1 = mock_note("cm1", 10_000_000, 1, NoteStatus::Unspent);
        let n2 = mock_note("cm2", 5_000_000, 1, NoteStatus::Unspent);
        let candidates = vec![&n1, &n2];

        // T = 5_000_000 (merchant 4_800_000, fee 100_000, exec 100_000)
        let selected = select_notes_stochastic_knapsack(
            &candidates,
            4_800_000,
            100_000,
            100_000,
            1000,
            1,
            &KnapsackConfig::default(),
        )
        .unwrap();

        assert_eq!(selected.input_commitment_hex, "cm2");
        assert_eq!(selected.input_value, 5_000_000);
        assert!(selected.second_input_commitment_hex.is_none());
        assert_eq!(selected.change_amount, 0);
        assert!(!selected.has_change);
    }

    #[test]
    fn test_tier2_single_note_sufficient_with_change() {
        let n1 = mock_note("cm1", 10_000_000, 1, NoteStatus::Unspent);
        let candidates = vec![&n1];

        // T = 7_000_000, change = 3_000_000 >= 100_000
        let selected = select_notes_stochastic_knapsack(
            &candidates,
            6_800_000,
            100_000,
            100_000,
            1000,
            1,
            &KnapsackConfig::default(),
        )
        .unwrap();

        assert_eq!(selected.input_commitment_hex, "cm1");
        assert_eq!(selected.change_amount, 3_000_000);
        assert!(selected.has_change);
    }

    #[test]
    fn test_tier2_economic_dust_absorption() {
        let n1 = mock_note("cm1", 10_050_000, 1, NoteStatus::Unspent);
        let candidates = vec![&n1];

        // T = 10_000_000, change would be 50_000 (< 100_000 dust threshold)
        let selected = select_notes_stochastic_knapsack(
            &candidates,
            9_800_000,
            100_000,
            100_000,
            1000,
            1,
            &KnapsackConfig::default(),
        )
        .unwrap();

        assert_eq!(selected.input_commitment_hex, "cm1");
        assert_eq!(selected.change_amount, 0);
        assert!(!selected.has_change);
        // Absorbed into execution fee: 100_000 + 50_000 = 150_000
        assert_eq!(selected.execution_fee, 150_000);
    }

    #[test]
    fn test_tier3_knapsack_2_notes() {
        let n1 = mock_note("cm1", 4_000_000, 1, NoteStatus::Unspent);
        let n2 = mock_note("cm2", 5_000_000, 1, NoteStatus::Unspent);
        let n3 = mock_note("cm3", 2_500_000, 1, NoteStatus::Unspent);
        let candidates = vec![&n1, &n2, &n3];

        // T = 8_000_000. No single note is sufficient.
        // n1 + n3 = 6.5M < 8M, n2 + n3 = 7.5M < 8M.
        // Only n1 + n2 = 9_000_000 >= 8_000_000 is sufficient.
        let selected = select_notes_stochastic_knapsack(
            &candidates,
            7_800_000,
            100_000,
            100_000,
            1000,
            1,
            &KnapsackConfig::default(),
        )
        .unwrap();

        assert!(selected.is_joinsplit());
        assert_eq!(selected.total_input_value(), 9_000_000);
        assert_eq!(selected.change_amount, 1_000_000);
        assert!(selected.has_change);
    }

    #[test]
    fn test_tier4_fragmentation_lockout() {
        // 4 notes of 2 USDC each = 8 USDC total.
        // Want to pay 7 USDC.
        // Any 2 notes = 4 USDC < 7 USDC.
        // Total available = 8 USDC >= 7 USDC, but N >= 3 required!
        let n1 = mock_note("cm1", 2_000_000, 1, NoteStatus::Unspent);
        let n2 = mock_note("cm2", 2_000_000, 1, NoteStatus::Unspent);
        let n3 = mock_note("cm3", 2_000_000, 1, NoteStatus::Unspent);
        let n4 = mock_note("cm4", 2_000_000, 1, NoteStatus::Unspent);
        let candidates = vec![&n1, &n2, &n3, &n4];

        let res = select_notes_stochastic_knapsack(
            &candidates,
            6_800_000,
            100_000,
            100_000,
            1000,
            1,
            &KnapsackConfig::default(),
        );

        match res {
            Err(WalletError::FragmentationLockout {
                requested,
                available,
                note_count,
            }) => {
                assert_eq!(requested, 7_000_000);
                assert_eq!(available, 8_000_000);
                assert_eq!(note_count, 4);
            }
            other => panic!("Expected FragmentationLockout, got {other:?}"),
        }
    }

    #[test]
    fn test_epoch_filtering_expired_notes() {
        // Current epoch = 3. Note with epoch 1 is E-2 (ExpiredRequiresRollover).
        let n1 = mock_note("cm1", 10_000_000, 1, NoteStatus::Unspent);
        let candidates = vec![&n1];

        let res = select_notes_stochastic_knapsack(
            &candidates,
            5_000_000,
            100_000,
            100_000,
            1000,
            3,
            &KnapsackConfig::default(),
        );

        assert!(matches!(res, Err(WalletError::InsufficientBalance { .. })));
    }
}
