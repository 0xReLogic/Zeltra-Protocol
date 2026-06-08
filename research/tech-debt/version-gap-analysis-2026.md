# Version Gap Analysis: Nimbus Tech Stack vs 2026 State of the Art

Date: 2026-06-08

## Critical Finding

Nimbus masih punya version gap, tetapi hard-test 2026-06-07/08 memperjelas satu
hal penting: blocker BLS **bukan** karena Arbitrum tidak mendukung EIP-2537.
Arbitrum sudah mengaktifkan BLS12-381 precompile melalui jalur ArbOS
Callisto/Dia. Kegagalan Nimbus berasal dari encoding EIP-2537 Arkworks:
field bytes sempat di-reverse dan G2/Fp2 perlu block remapping.

---

## Version Comparison (Current vs Latest 2026)

| Component | Nimbus Version | Latest 2026 | Gap | Release Date |
|-----------|----------------|-------------|-----|--------------|
| **Stylus SDK** | 0.6.0 (Aug 2024) | **0.10.0** (Jan 2026) | **4 versions** | 18 months |
| **Stylus Audit** | N/A | **OpenZeppelin audit complete** (Dec 2025) | - | Security milestone |
| **ark-bls12-381** | 0.5.0 (nimbus-core) | **0.6.0** (Apr 2026) | **1 version** | 6 months |
| **alloy-primitives** | 0.7.6 / 1.0 | **1.6.0** (May 2026) | **Major version** | 1 year |
| **arkworks stack** | 0.5.0 | **0.6.0** | **1 version** | Apr 2026 |
| **Tokio** | 1.52 manifest / 1.52.3 resolved | **Aligned in batch 1** | ✅ Done | 2026-06-08 |
| **Axum** | 0.8 manifest / 0.8.9 resolved | **Aligned in batch 1** | ✅ Done | 2026-06-08 |
| **Alloy node stack** | 1.8 manifest / 1.8.3 resolved | **Aligned in batch 1** | ✅ Done | 2026-06-08 |
| **zeroize** | 1.8 | **1.8** | ✅ Current | - |

---

## 🔴 CRITICAL: Stylus SDK Gap (4 versions)

### Current: 0.6.0 (August 2024)
- Initial stable release
- Basic features
- Limited optimizations

### Latest: 0.10.0 (January 2026)
- EIP-7702 support (Smart Wallets)
- Better caching strategies
- Advanced storage handling
- Router trait improvements (fallback, receive)
- Better Solidity interoperability
- Performance optimizations
- Memory optimizations

### What We're Missing:
```rust
// Stylus 0.6.0 (current) - BASIC features
stylus-sdk = "0.6.0"

// Stylus 0.10.0 (latest) - ADVANCED features
stylus-sdk = "0.10.0"  // + EIP-7702, better caching, + OpenZeppelin audit (Dec 2025)
```

### 2. **EIP-2537 (BLS12-381 Precompile) - SOTA 2026 Formula**
```
Current understanding: General "cheaper"
SOTA 2026 formula: Gas cost = 37,700 + 32,600 × k

Comparison:
- Old (BN254): 45,000 + 34,000 × k
- New (BLS12-381): 37,700 + 32,600 × k
- Savings: Significant per pairing operation

L2 Adoption:
- Arbitrum: EIP-2537 tersedia melalui ArbOS Callisto/Dia. Jangan treat ini
  sebagai chain-support blocker sebelum test vector precompile langsung gagal.
- Etherlink: Native activation
- Performance: Major improvement vs BN254
```

**Impact:** Missing 18 months of improvements. Encoding BLS12-381 EIP-2537
sudah dibetulkan pada 2026-06-08, tetapi upgrade dependency tetap perlu
dikerjakan terpisah sebelum mainnet.

### 3. **Resolved: EIP-2537 Hard-Test Finding (2026-06-08)**

Hard-test terakhir sempat disimpulkan terlalu cepat sebagai "Arbitrum belum
support EIP-2537". Itu perlu dikoreksi.

Fakta terbaru:

- Arbitrum Sepolia RPC `chain_id=421614` menerima direct call ke precompile
  `0x0b` (`BLS12_G1ADD`) dengan vector generator EIP-2537 dan mengembalikan
  output benar.
- Direct tests juga lolos untuk `0x0d` (`BLS12_G2ADD`) dan `0x0f`
  (`BLS12_PAIRING_CHECK`) setelah encoding fix.
- Contract fixed berhasil deploy dan activate:
  `0xd9f1f8f53a8e0b5b8bc6361946119de02cf5c159`.
- `spend` valid sukses tanpa mock/bypass:
  `0x39dd200a6205295f190d2bed47ecb74ee6b8f61689d93535d2b715a3a5741498`.

Root cause:

```text
Arkworks G1/G2 field bytes:
  already big-endian for EIP-2537 field encoding; do not reverse bytes.

Arkworks G2 raw block order:
  X1 || X0 || Y1 || Y0

EIP-2537 expected G2 block order:
  X0 || X1 || Y0 || Y1

Correct mapping from Arkworks raw to EIP-2537:
  [1, 0, 3, 2], without byte reversal inside each 48-byte field.
```

Analogi sederhana:

```text
Precompile itu seperti mesin ATM yang sudah aktif.
Kartu kita masuk ke mesin, tapi format chip/PIN yang kita kirim kebalik.
Mesinnya bukan mati; input kita yang tidak sesuai standar mesin. Setelah format
dibetulkan, transaksi diterima.
```

Completed action:

1. Perbaiki `to_evm_g1()` agar tidak reverse field bytes.
2. Perbaiki `to_evm_g2()` agar memakai `[1, 0, 3, 2]` tanpa reverse field bytes.
3. Jalankan direct RPC test untuk:
   - `0x0b` G1ADD known vector;
   - `0x0d` G2ADD known vector;
   - `0x0f` pairing known vector.
4. Deploy ulang dan ulangi hard-test `spend` nyata.

Sources:

- EIP-2537 official spec: https://eips.ethereum.org/EIPS/eip-2537
- Arbitrum ArbOS 50 Dia proposal: https://forum.arbitrum.foundation/t/constitutional-aip-arbos-version-50-dia/29835/1
- Arbitrum EIP-7702/Callisto blog mentioning EIP-2537 live:
  https://blog.arbitrum.foundation/the-smartest-wallet-you-already-own-is-on-arbitrum/

---

## 🟡 SIGNIFICANT: alloy-primitives Gap (Major Version)

### Current: 0.7.6 / 1.0
### Latest: 1.6.0

**What changed in 1.x:**
- New API surface (breaking changes)
- Better type safety
- Improved performance
- Better compatibility with newer Ethereum clients

---

## 🟡 MODERATE: Crypto Stack Gap

### ark-bls12-381: 0.5.0 → 0.6.0
- 0.6.0 released April 2026
- Minor improvements, bug fixes
- Better documentation

### arkworks stack: 0.5.0 → 0.6.0
- Consistent version upgrade across all ark libraries
- Better performance
- Better security audits

---

## ✅ RESOLVED BATCH 1: Web Framework / Node Runtime Gap

### Tokio: 1.36.0/1.35 manifest → 1.52 manifest
- Performance improvements
- Better async runtime
- Better diagnostics

### Axum: 0.7.5 manifest → 0.8 manifest / 0.8.9 resolved
- Minor API changes
- Better middleware ecosystem

### Other aligned node/CLI dependencies
- `nimbus-node`: `serde` 1.0.228, `serde_json` 1.0.150, `alloy` 1.8,
  `tempfile` 3.27.
- `nimbus-cli`: `clap` 4.6, `tokio` 1.52.

Verification:

```text
cargo test --package nimbus-node handlers::quote
cargo check --workspace --all-targets
```

Both passed on 2026-06-08.

---

## What 2026 Protocols Are Using (That We're Not)

### Latest Crypto Privacy Projects (2026):

1. **Nova/SuperNova (Recursive SNARKs)**
   - Batch proof aggregation
   - Lower verification cost
   - Nimbus: Still using non-recursive BLS

2. **Polygon zkEVM (Miden VM)**
   - EVM-compatible zk rollup
   - WASM-based execution
   - Different from Stylus approach

3. **Risc Zero / zkVM**
   - General-purpose zkVM
   - Different use case (general zk computation)

4. **EIP-3074 (ERC-4337) Enhanced**
   - Account abstraction
   - Smart wallets
   - Nimbus: Partial support (documented, not implemented)

5. **Optimism Bedrock (Modular L2)**
   - Fraud proof systems
   - Different from Arbitrum approach

---

## Specific 2026 Features We're Missing from Stylus 0.10.0

### 1. **EIP-7702 Smart Wallets**
```rust
// Stylus 0.10.0 supports EIP-7702
// Nimbus has this documented but not implemented
Router trait supports fallback and receive methods
```

### 2. **Advanced Caching**
```rust
// Stylus 0.10.0 has improved caching
// Nimbus: Basic caching
// Missing: Advanced cache invalidation strategies
```

### 3. **Better Solidity Interoperability**
```rust
// Stylus 0.10.0: Better ABI generation, Solidity imports
// Nimbus: Manual Solidity interfaces
```

---

## Action Plan: Upgrade to 2026 State of the Art

### Priority 1: Stylus SDK Upgrade (CRITICAL)
```toml
# Current
stylus-sdk = "0.6.0"

# Latest (2026)
stylus-sdk = "0.10.0"
```

**Risk:**
- Breaking changes between 0.6 → 0.10
- API surface changes
- Need migration guide

**Benefit:**
- Access to EIP-7702 (smart wallets)
- Better performance
- Security patches
- 18 months of improvements

### Priority 2: Crypto Stack Upgrade
```toml
# nimbus-core
ark-bls12-381 = "0.5.0"  # → 0.6.0
ark-ec = "0.5.0"            # → 0.6.0
ark-ff = "0.5.0"            # → 0.6.0
ark-std = "0.5.0"           # → 0.6.0
ark-serialize = "0.5.0"    # → 0.6.0
```

**Risk:** Minor, should be compatible
**Benefit:** Security patches, performance

### Priority 3: Web Framework Upgrade
```toml
# nimbus-contracts
alloy-primitives = "0.7.6"  # → 1.6.0

# nimbus-node / nimbus-cli
tokio = "1.52"              # done
axum = "0.8"                # done
alloy = "1.8"               # node stack done
```

**Status:** Node/CLI runtime part done. Contract-side `alloy-primitives` remains
separate because it affects Stylus/WASM compatibility and should be migrated in
the same guarded lane as contract dependency work.

---

## Recommendation

### Testnet (Current)
```
✅ Keep Stylus/contract crypto stable while testnet hard-tests continue.
✅ Node/CLI runtime dependencies have been aligned in batch 1.
```

### Production Upgrade (Required)
```
⚠️  Upgrade to latest versions BEFORE mainnet
  - Stylus 0.10.0 (4 versions behind)
  - ark-bls12-381 0.6.0
  - alloy-primitives 1.6.0
  - Tokio/Axum node runtime: done in batch 1

Reason: Production should use latest stable versions
Security patches, performance improvements, EIP-7702 support
```

---

## Migration Strategy

### Phase 1: Dependency Update (1-2 days)
1. Update nimbus-contracts/Cargo.toml
2. Update nimbus-core/Cargo.toml  
3. Update nimbus-node/Cargo.toml ✅ batch 1 done
4. Fix compilation errors from API changes
5. Run cargo test to verify

### Phase 2: Code Migration (2-3 days)
1. Update Stylus-specific APIs (0.6 → 0.10)
2. Update alloy APIs (0.7 → 1.6)
3. Add EIP-7702 support if desired
4. Update caching strategy
5. Test all functionality

### Phase 3: Hard Test (3-5 days)
1. Run all unit tests
2. Deploy to testnet
3. E2E integration test
4. Performance benchmark
5. Gas cost comparison

---

## Conclusion

**Verdict:** YES, Nimbus tech stack BEHIND 2026 state of the art by 1-2 years

**Critical Gap:**
- **Stylus 0.6.0 → 0.10.0** (18 months behind, 4 versions)
- **alloy-primitives** (major version behind)
- **crypto stack** (6 months behind)
- **node/CLI runtime gap** resolved in batch 1 on 2026-06-08

**Recommendation:** Upgrade sebelum mainnet deployment untuk:
- Security patches
- Performance improvements  
- EIP-7702 support (smart wallets)
- Future-proofing
