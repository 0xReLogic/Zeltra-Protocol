# Signed Quote + Batch Claim

**Priority:** Tier 1 (Safety + Revenue)
**Status:** Research Complete (2026-06-13), Ready for Implementation
**Approach:** EIP-712 Typed Data Signing (confirmed via Exa/Tavily research)

## Research Findings (2026-06-13)

### Industry Standard: EIP-712
All production gasless relayers use EIP-712:
- **Gelato Relay** — EIP-712 typed data signing
- **Biconomy** — EIP-712 untuk meta-transactions
- **Relay Protocol** — EIP-2612 permit (subset EIP-712) untuk approval
- **GSN (Gas Station Network)** — ERC-2771 + EIP-712

**Why EIP-712:**
- Wallet support bagus (MetaMask, Rainbow render typed data)
- Domain separator prevent cross-chain/cross-contract replay
- Structured data (bukan raw hash) — user bisa liat apa yang di-sign

### Critical Exploit Patterns (MUST prevent)

**1. Biconomy Audit #492 (High Risk) - Unsigned Fee Parameters**
- `tokenGasPriceFactor` TIDAK di-sign oleh user
- Relayer bisa set factor = 1 (harusnya 100) → user bayar **100x lebih mahal**
- **Fix:** Include SEMUA fee parameters dalam signed hash

**2. Biconomy Audit #535 - Gas Estimation Manipulation**
- Relayer append zero bytes ke signature → inflate `msg.data.length`
- Heuristic gas estimation overestimate → relayer dapat reimbursement lebih besar
- **Fix:** Bound gas calculation, jangan rely on `msg.data.length`

**3. DBXen Exploit (March 2026, $133K loss) - msg.sender vs _msgSender()**
- XEN burned dari Forwarder tapi credits ke attacker
- Inconsistent sender resolution dalam same function
- **Fix:** Gunakan consistent sender resolution untuk SEMUA accounting (debit DAN credit)

**4. ERC-4337 Paymaster Attacks (April 2026) - Missing Validation**
- `postOp` ga validate `actualGasCost` matched claimed cost
- Attacker claimed high fees tapi ga transfer tokens
- **Fix:** Verify actual deduction matches claimed cost

### Implementation Approach

**SDK side (TypeScript/JavaScript):**
```typescript
const types = {
  NimbusQuote: [
    { name: "maxExecutionFee", type: "uint256" },
    { name: "quoteExpiry", type: "uint256" },
    { name: "quoteId", type: "bytes32" },
    { name: "relayer", type: "address" },
    { name: "nonce", type: "uint256" }
  ]
};

const signature = await signer.signTypedData(domain, types, quote);
```

**Contract side (Solidity):**
```solidity
function verifyQuote(
    address user,
    uint256 maxExecutionFee,
    uint256 quoteExpiry,
    bytes32 quoteId,
    bytes memory signature
) internal view returns (bool) {
    bytes32 digest = keccak256(abi.encodePacked(
        "\x19\x01",
        DOMAIN_SEPARATOR,
        keccak256(abi.encode(
            NIMBUS_QUOTE_TYPEHASH,
            maxExecutionFee,
            quoteExpiry,
            quoteId,
            relayer,
            nonces[user]++
        ))
    ));
    return ECDSA.recover(digest, signature) == user;
}
```

**Domain separator:**
```solidity
DOMAIN_SEPARATOR = keccak256(abi.encode(
    keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"),
    keccak256("NimbusProtocol"),
    keccak256("1"),
    block.chainid,
    address(this)
));
```

## Context
Relayer bayar gas sendiri tapi ga ada reimbursement dari user. Quote sekarang cuma informational, ga binding. Tanpa ini relayer rugi tiap transaksi.

## Requirements

### 1. Signed Quote (User Binding)
- User sign `max_execution_fee` + `quote_expiry` + `quote_id` + chain ID + relayer identity
- Contract verify signature sebelum execute
- Actual debit ≤ signed amount → reject kalau lebih
- Use EIP-712 typed data signing

### 2. Batch Claim (Relayer Revenue)
- Node track `execution_fee` per batch di `spend_batches` table
- Tambah kolom `claimed` (boolean)
- Background worker: `SUM(execution_fee) WHERE claimed = false`
- Klaim saat threshold tercapai (amount/count/time-based)
- Contract: `claimExecutionFee(uint256 amount)` — owner/relayer only
- Update `claimed = true` hanya setelah tx sukses
- Gas overhead: ~65k per claim (negligible kalau di-batch)

## Acceptance Criteria
- [ ] User sign EIP-712 quote sebelum transaksi
- [ ] Contract reject kalau actual fee > signed max_execution_fee
- [ ] Contract reject kalau quote expired (block.timestamp > quote_expiry)
- [ ] Node track execution_fee per batch di DB
- [ ] Background worker klaim berkala (threshold amount/count/time)
- [ ] No double claim (claimed flag enforce)
- [ ] Positive test: valid quote → spend success → claim success
- [ ] Negative test: overcharge (actual > max) → revert
- [ ] Negative test: expired quote → revert
- [ ] Negative test: invalid signature → revert
- [ ] `cargo test` + `cargo clippy` clean

## Security Requirements (from research)
- [ ] ALL fee parameters included in signed hash (prevent Biconomy #492)
- [ ] Bound gas calculation, jangan rely on `msg.data.length` (prevent Biconomy #535)
- [ ] Consistent sender resolution untuk semua accounting (prevent DBXen exploit)
- [ ] Verify actual deduction matches claimed cost (prevent ERC-4337 paymaster attacks)
- [ ] Domain separator include chain_id + contract_address (prevent cross-chain replay)

## Post-Implementation
- [ ] **Security audit** oleh agent sebelum deploy ke mainnet
- [ ] Test di testnet (Arbitrum Sepolia) dengan real transactions
- [ ] Monitor for edge cases di production

## Reference
- `todo.md` section: P1 - Adaptive Private Spend Batching
- `todo.md` section: P1 - Relayer Execution Fee Batch Claim
- Research: EIP-712 typed data signing pattern
- Exploits: Biconomy audit #492/#535, DBXen (March 2026), ERC-4337 paymaster (April 2026)
