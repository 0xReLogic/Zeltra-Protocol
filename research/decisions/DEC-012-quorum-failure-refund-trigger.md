# DEC-012: Quorum Failure Refund Trigger

Date: 2026-06-07

## Masalah

Saat ini, user hanya dapat refund dana setelah 24 jam (86400 detik) dari deposit. Tidak ada mekanisme untuk mendeteksi dan merespon kegagalan quorum threshold signing secara lebih cepat. Jika guardian node offline/stuck atau leader crash sebelum deposit, user harus menunggu 24 jam penuh meskipun failure sudah jelas terjadi.

Masalah spesifik:
- Quorum threshold signing (2-of-3, 3-of-5) dapat gagal jika guardian offline/tidak merespons
- Leader dapat crash setelah deposit confirmation tapi sebelum `k` dirilis
- Database corruption atau infrastructure failure dapat mencegah `k` dirilis
- User tidak memiliki visibility ke status quorum signing
- Tidak ada monitoring atau alerting untuk quorum failure

## Invariant bisnis/security

- User dana tidak boleh tertahan di escrow tanpa jalan keluar
- Quorum failure harus dapat di-detect dan di-respond
- Refund path tidak boleh bergantung pada guardian yang gagal
- Refund tidak boleh dapat di-trigger sebelum deposit terkonfirmasi (prevent double-payout)
- Refund otomatis tidak boleh dapat dieksploitasi untuk race condition atau griefing
- Monitoring/alerting untuk quorum failure harus ada untuk operational awareness

## Pilihan yang dipertimbangkan

1. **Timelock 24 jam saja (status quo)** - User tunggu 24 jam regardless dari failure cause
2. **Refund otomatis pada quorum failure detection** - Trigger refund segera setelah detect quorum fail
3. **Intermediate timelock (6 jam) untuk quorum failure** - Shorter timeout untuk failure cases
4. **Hybrid: Detect + gradual escalation** - Monitor signing progress, escalate to refund jika stuck

## Keputusan

Gunakan pilihan 4 untuk production: **Hybrid: Monitoring, Detection, dan Gradual Escalation**

### Implementasi phased:

**Phase 1 (immediate):**
- Tambah signing session monitoring ke relayer
- Track waktu sejak request signing ke quorum
- Log warning jika signing stuck lebih dari X menit
- Tambah health check endpoint untuk signing session status

**Phase 2 (short-term):**
- Implementasi background job untuk detect quorum failure
- Jika quorum gagal tercapai setelah timeout (misal 10 menit), mark session sebagai failed
- Trigger refund otomatis untuk session marked failed
- Terapkan safety check: hanya trigger refund jika deposit confirmed DAN signing failed

**Phase 3 (long-term):**
- Implementasi warning system/alerting (pagerduty, email) untuk quorum failure
- Dashboard monitoring untuk session health
- Grafana/metrics untuk signing success rate dan latency

### Timeout Configuration (Dec-001 extended):

- **Signing request timeout**: 60 detik (current, untuk timestamp validation)
- **Quorum signing timeout**: 10 menit (baru, untuk detect stuck signing)
- **Session expiry**: 1 jam (current, untuk validation di guardian)
- **Refund timelock**: 24 jam (current, remain sebagai safety fallback)
- **Automatic refund window**: 24 jam - quorum_timeout = ~23 jam 50 menit (new, auto-refund gap)

### Safety Guarantees:

1. **Refund hanya jika deposit confirmed** - Cek on-chain state sebelum trigger
2. **Refund hanya jika signing failed** - Tidak trigger jika signing sedang berjalan
3. **Refund idempotent** - Boleh dipanggil berulang tanpa double refund
4. **Timelock fallback tetap ada** - User masih dapat refund manual setelah 24 jam jika auto-refund gagal
5. **Alert jika auto-refund gagal** - Ops harus dapat menangani auto-refund failure

## Alasan

- Pilihan 1 tidak user-friendly - 24 jam terlalu lama untuk failure yang jelas
- Pilihan 2 berisiko race condition jika quorum sebenarnya lambat tapi sukses
- Pilihan 3 masih fixed timeout, tidak adaptif
- Pilihan 4 memberikan visibility, responsiveness, dan safety defense-in-depth

Dengan phased approach:
- Phase 1 memberikan visibility tanpa risk auto-refund
- Phase 2 memberikan responsive refund dengan safety checks
- Phase 3 memberikan operational excellence untuk production

Timelock 24 jam tetap ada sebagai fallback, jadi jika monitoring atau auto-refund gagal, user masih dapat manual refund. Ini mengikuti pattern dari ClawTrust (7-day timeout sebagai fallback) dan FortyTwo (configurable timeout 300-86400 seconds).

## Sumber primer

- Dash Core LLMQ Documentation: SESSION_TOTAL_TIMEOUT = 300 seconds, quorum signing timeouts
  https://docs.dash.org/projects/core/en/21.0.0/docs/guide/dash-features-masternode-quorums.html
- Vote SDK TSS Ceremony: DEALT timeout activation with quorum requirements
  https://github.com/valargroup/vote-sdk/blob/main/docs/tss-ceremony.md
- Lux LP-2106: Timeout triggers dispute window with accountability
  https://lps.lux.network/docs/lp-2106/
- FortyTwo Escrow: refundAfterTimeout with configurable timeout (300-86400 seconds)
  https://docs.fortytwo.network/docs/x402escrow-contract-reference
- ClawTrust Escrow: 7-day timeout-based refunds that work even when paused
  https://clawtrust.mintlify.app/contracts/escrow
- ERC-5507: Refundable tokens with refund deadline
  https://ercs.ethereum.org/ERCS/erc-5507

Accessed: 2026-06-07.

## Sumber pembanding

- THORChain TSS: Blame attribution for timeout failures with slashing
  https://dev.thorchain.org/bifrost/tss.html
- Optimex Vault: Timelock protection where user can reclaim funds after time T
  https://docs.optimex.com/optimex-revolutionizing-bitcoin-finance/primary-building-blocks/native-bitcoin-vault
- Kairo Guard MPC: Identity-based recovery with network-assisted backup
  https://www.kairoguard.com/blog/multisig-vs-mpc-vs-2pc-mpc
- Canton DeFi Custody: Robust to node failure with threshold configuration
  https://cantondefi.pages.dev/security/custody.html

Accessed: 2026-06-07.

## Known risks

- False positive: Signing berhasil setelah auto-refund triggered (mitigasi: safe gap, manual refund fallback)
- Race condition: Deposit confirmed dan refund hampir bersamaan (mitigasi: contract-level checks, DB transaction)
- Monitoring failure: Relayer tidak dapat detect quorum failure (mitigasi: timelock fallback, alerting)
- Database inconsistency: Relayer DB out of sync dengan on-chain state (mitigasi: query on-chain state sebelum refund)
- DDoS/griefing: Attacker coba crash signing untuk trigger refund (mitigasi: rate limiting, session limits)

## Versi/library/network

- Signing session monitoring: nimbus-node/src/database.rs
- Background job: tokio::spawn + tokio::time::interval
- On-chain state verification: nimbus-node/src/evm_client.rs
- Contract refund function: nimbus-contracts/src/lib.rs (existing)
- Metrics: Optional (Phase 3), Prometheus atau StatsD

## Test

Positive:

- Monitoring detect quorum failure setelah timeout
- Auto-refund trigger untuk session yang deposit confirmed dan signing failed
- Auto-refund idempotent - boleh dipanggil berulang
- Manual refund fallback tetap bekerja setelah 24 jam

Negative:

- Tidak trigger auto-refund untuk session yang deposit confirmed dan signing success
- Tidak trigger auto-refund untuk session yang belum deposit confirmed
- Signing yang lambat tapi sukses tidak di-trigger auto-refund (safe gap)
- Alert terkirim jika auto-refund gagal

## Rollback/recovery

Jika auto-refund menyebabkan issue:
1. Disable auto-refund via environment variable `NIMBUS_AUTO_REFUND=false`
2. Monitoring tetap berjalan tapi tidak trigger action
3. Manual refund fallback tetap tersedia
4. Ops dapat menginvestigate log untuk debugging

Jika monitoring menyebabkan performance issue:
1. Disable background job via feature flag
2. Tetap gunakan timelock 24 jam manual
3. Investigate query performance sebelum re-enable

## Implementation

### Phase 1: Monitoring (immediate)

1. Tambah field `signing_started_at` ke signing sessions
2. Tambah function `get_stalled_signing_sessions(threshold_duration)`
3. Tambah endpoint `/api/signing-health` untuk visibility
4. Log warning untuk session yang stuck > threshold

```rust
// database.rs
pub async fn get_stalled_signing_sessions(
    &self,
    threshold_seconds: i64,
) -> Result<Vec<SigningSessionInfo>> {
    let threshold = chrono::Utc::now().timestamp() - threshold_seconds;
    // Query sessions yang deposit_confirmed=1, resolved=0, created_at < threshold
}
```

### Phase 2: Auto-refund (short-term)

1. Background job berjalan setiap 1 menit
2. Detect session yang deposit confirmed + signing timeout
3. Mark session sebagai failed
4. Trigger refund untuk failed session
5. Tambah safety check: cek on-chain state

```rust
// handlers/deposit.rs
async fn background_quorum_monitor(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        let stalled = state.db.get_stalled_signing_sessions(600).await; // 10 menit
        for session in stalled {
            // Cek on-chain state: deposit confirmed, belum spent/refunded
            if state.is_deposit_confirmed_onchain(&session.session_id).await
                && !state.is_spent_or_refunded_onchain(&session.session_id).await
            {
                // Trigger refund
                state.trigger_refund(&session.session_id).await;
            }
        }
    }
}
```

### Phase 3: Alerting (long-term)

1. Integrasikan alerting system (email, pagerduty, slack)
2. Dashboard Grafana untuk signing metrics
3. Alert jika signing success rate drop below threshold
4. Alert jika banyak session stuck

## Dependency pada DEC

- DEC-001 (atomic release flow) - session lifecycle dan deposit confirmation
- DEC-009 (liability invariant) - ensure refund tidak break solvency
- DEC-011 (session validation) - session expiry dan validation logic
- DEC-012 memperluas DEC-001 dengan handling quorum failure cases
