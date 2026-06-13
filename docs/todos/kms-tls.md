# KMS & TLS Hardening

**Priority:** Tier 3 (Bisa Nanti)
**Status:** Not Started (0/7 complete)

## Context
HTTP client masih plain TCP (no TLS). Vault/guardian communication bisa kena MITM attack. Key lifecycle belum production-grade.

## Current State
- [ ] Ganti raw TCP HTTP client dengan client HTTPS yang tervalidasi
- [ ] Verifikasi TLS certificate untuk Vault dan guardian RPC
- [ ] Jangan menghapus awalan `https://` lalu mengirim plaintext TCP
- [ ] Gunakan short-lived Vault token atau workload identity
- [ ] Validasi key version dan expected guardian index
- [ ] Implementasikan threshold proactive refresh yang nyata; re-masking memory bukan penggantian share antar guardian
- [ ] Gunakan crate zeroization yang diaudit untuk secret memory

## Requirements

### 1. HTTPS Client
- Replace `http.rs` dengan `reqwest` atau `hyper` dengan TLS support
- Verify TLS certificate (no self-signed kecuali development)
- Support custom CA bundle

### 2. Vault Token Rotation
- Gunakan AppRole authentication (bukan static token)
- Short-lived token (1h TTL)
- Auto-renew sebelum expired

### 3. Key Versioning
- Track key version di DB
- Guardian validate key version sebelum sign
- Reject kalau version mismatch

### 4. Proactive Refresh
- Periodic re-share (tanpa change master secret)
- All guardians participate
- Atomic update (all-or-nothing)

### 5. Zeroization
- Use `zeroize` crate (audited)
- Zeroize secret setelah use
- Zeroize on drop

## Acceptance Criteria
- [ ] HTTPS client works dengan TLS verification
- [ ] Vault token auto-renew
- [ ] Key version validation enforce
- [ ] Proactive refresh works
- [ ] Zeroization applied
- [ ] `cargo test` + `cargo clippy` clean

## Reference
- `todo.md` section: P2 - KMS, Database, dan Secrets
- `nimbus-node/src/http.rs`: current plain TCP client
- `nimbus-node/src/kms.rs`: Vault integration
- Research: reqwest TLS, Vault AppRole, zeroize crate
