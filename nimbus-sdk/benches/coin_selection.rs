use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nimbus_sdk::coin_selection::{select_notes_stochastic_knapsack, KnapsackConfig};
use nimbus_sdk::wallet::note_wallet::{NoteStatus, WalletNote};

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

fn bench_coin_selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("coin_selection");

    for n in [10, 50, 100, 200].iter() {
        let mut candidates: Vec<WalletNote> = Vec::new();
        for i in 0..*n {
            let value = 5_000_000 + (i * 100_000) as u64;
            candidates.push(mock_note(
                &format!("cm{}", i),
                value,
                1,
                NoteStatus::Unspent,
            ));
        }

        let candidate_refs: Vec<&WalletNote> = candidates.iter().collect();

        group.bench_with_input(format!("n={}", n), &candidate_refs, |b, refs| {
            b.iter(|| {
                black_box(select_notes_stochastic_knapsack(
                    black_box(refs),
                    black_box(7_800_000),
                    black_box(100_000),
                    black_box(100_000),
                    black_box(1000),
                    black_box(1),
                    black_box(&KnapsackConfig::default()),
                ))
            })
        });
    }

    group.finish();
}

criterion_group!(benches, bench_coin_selection);
criterion_main!(benches);
