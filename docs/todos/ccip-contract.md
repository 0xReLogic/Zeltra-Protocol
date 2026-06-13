# CCIP Contract Security (On-chain)

**Priority:** Tier 2 (Mainnet Safety)
**Status:** In Progress (1/7 complete)

## Context
CCIP contract logic masih ada yang belum di-validate: router non-zero, allowlist, sender decode, payload audit. Off-chain tracking udah selesai.

## Current State
- [x] Gunakan message ID untuk replay protection
- [ ] Wajibkan `ccip_router != Address::ZERO` sebelum menerima message
- [ ] Tolak semua caller jika router belum dikonfigurasi
- [ ] Validasi `source_chain_selector` dengan allowlist
- [ ] Decode dan validasi sender CCIP
- [ ] Bind sender contract yang sah untuk setiap source chain
- [ ] Audit payload encoding antara source relayer dan destination contract

## Requirements
1. Router validation: `require(ccip_router != address(0))` di `_ccip_receive()`
2. Caller validation: `require(msg.sender == ccip_router)`
3. Allowlist: `mapping(uint64 => mapping(address => bool))` untuk source chain + sender
4. Sender decode: parse `sender` dari CCIP message, verify di allowlist
5. Payload audit: verify encoding match antara source dan destination

## Acceptance Criteria
- [ ] Contract reject CCIP message kalau router zero
- [ ] Contract reject caller bukan router
- [ ] Allowlist enforce source chain + sender
- [ ] Payload encoding verified
- [ ] `cargo test` + `cargo clippy` clean

## Reference
- `todo.md` section: P1 - CCIP Contract Security (On-chain)
- `nimbus-contracts/src/spend.rs`: `_ccip_receive()`
- `research/decisions/DEC-015-ccip-cross-chain-security-validations.md`
