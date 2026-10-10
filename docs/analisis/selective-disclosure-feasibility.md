# Selective Disclosure for Nimbus: Feasibility Analysis

## Executive Summary

**Problem:** Gimana user bisa deposit ke Nimbus (private) tapi tetep bisa buktiin ke CEX bahwa source funds-nya clean/compliant tanpa expose wallet address atau transaction history?

**Solution Proposal:** Off-chain ZK proof system dimana user generate proof selectively untuk verifier tertentu (e.g., CEX), bukan public proof. Nimbus **hanya cryptographically bind user ke deposit address**, verifier (CEX) check address dengan tools mereka sendiri.

**Key Insight:** Privacy 9.5/10 + Product 10/10 bisa dicapai kalau:
- Deposit on-chain (unavoidable, acceptable tradeoff)
- Spend fully private (existing nullifier + ZK)
- Compliance proof **selective** (user pilih siapa yang liat, bukan broadcast public)
- **NO ASP PARTNERSHIP NEEDED** (CEX pake Chainalysis/tools mereka sendiri)

---

## Use Case: Alice Withdraws to Binance

### Persona
- **Alice:** Privacy-conscious trader
- **Binance:** CEX yang require source-of-funds verification
- **Nimbus:** Privacy protocol (neutral infrastructure)

### Current Flow (Broken)

```
1. Alice deposit 10,000 USDC dari wallet X → Nimbus
   ❌ Problem: Binance ga tau apakah Alice deposit dari clean source
   
2. Alice spend 5,000 USDC dari Nimbus → Binance wallet
   ✅ Private: Nullifier prevents double-spend, balance stays hidden
   
3. Binance: "Prove your source of funds or we freeze withdrawal"
   ❌ Dead end: Alice punya privacy tapi ga bisa prove compliance
```

**Outcome:** Alice stuck. Privacy 10/10 tapi Product 0/10.

---

### Proposed Flow (Selective Disclosure)

```
1. Alice deposit 10,000 USDC dari wallet X → Nimbus
   - On-chain visible: session_id, amount, timestamp
   - Hidden: wallet X identity (until Alice choose to disclose)
   
2. Alice spend 5,000 USDC dari Nimbus → Binance wallet Y
   - Fully private: nullifier + ZK proof
   - Binance receives USDC tapi ga tau source
   
3. Binance: "Prove your source or KYC required"

4. Alice generate ZK proof OFF-CHAIN:
   π = Prove {
     - "I own credential σ from session S"
     - "Session S deposited from address X"  
     - WITHOUT revealing: my balance, other txs, other sessions
   }
   
5. Alice sends π + revealed address X to Binance (via email, support ticket, API)

6. Binance verify π + check address INDEPENDENTLY:
   - Verify BBS+ proof: Verify(π, pk_nimbus_issuer) ✅
   - Extract revealed address: X = 0x123...
   - Query Chainalysis API: is_clean(X) → risk_score = 0.02 ✅
   - Check OFAC sanctions list: X not sanctioned ✅
   - Apply Binance policy: risk_score < 0.1 → APPROVE
```

**Outcome:** Privacy 9.5/10 (deposit on-chain, spend private) + Product 10/10 (CEX compliance satisfied).

---

## Technical Architecture

### Components

#### 1. Proof System: BBS+ Signatures (Selective Disclosure Native)

**Why BBS+ instead of Groth16?**

| Feature | BBS+ | Groth16 (current Nimbus) |
|---------|------|--------------------------|
| Selective disclosure | ✅ Native support | ❌ Reveal all or nothing |
| Proof size | ~200 bytes | ~200 bytes (comparable) |
| Verification speed | ~5ms | ~2ms (slightly faster) |
| Circuit complexity | ⚡ Low (no R1CS) | 🔥 High (R1CS constraints) |
| Message hiding | ✅ Per-message choice | ❌ All messages hidden |
| Maturity | ✅ IETF draft, W3C standard | ✅ Production ready (ZCash) |

**Verdict:** BBS+ lebih cocok untuk selective disclosure use case. Groth16 tetep dipake untuk change note circuit (existing system).

---

#### 2. Credential Structure

```rust
// Credential yang di-sign oleh Nimbus issuer
struct NimbusCredential {
    session_id: [u8; 32],           // Public: on-chain session ID
    deposit_address: Address,        // HIDDEN by default, selectively revealed
    deposit_amount: u64,             // Public: on-chain amount
    deposit_timestamp: u64,          // Public: on-chain block time
    deposit_chain_id: u64,           // Public: which chain (Arbitrum, etc.)
    credential_signature: BBSSignature, // BBS+ signature over all fields
}
```

**What user can selectively disclose:**
- ✅ `deposit_address` - prove "I deposited from address X"
- ✅ `deposit_timestamp` - prove "I deposited at time T"
- ❌ Never disclose: spending history, balance, other sessions

**What verifier (CEX) does with revealed address:**
- CEX check address X dengan tools MEREKA SENDIRI:
  - Chainalysis subscription? Check X di situ
  - Internal risk scoring? Run model on X
  - OFAC sanctions list? Check X
  - **Nimbus ga terlibat, CEX decide sendiri**

---

#### 3. Proof Generation (Client-Side)

**User's wallet/SDK generates proof:**

```
Input:
- credential (BBS+ signed by Nimbus issuer)
- revealed_fields = [deposit_address, deposit_timestamp]
- hidden_fields = [user's other sessions, balance]
- verifier_nonce (from CEX, prevents replay)

Output:
- π (ZK proof)
- revealed_messages = {deposit_address: 0x123..., deposit_timestamp: 1735689600}
```

**Properties:**
- Proof valid IFF credential was signed by trusted Nimbus issuer
- CEX learns ONLY revealed fields, nothing else
- Proof bound to verifier_nonce (can't reuse for different CEX)

---

#### 4. Verification (CEX-Side)

**CEX's backend verifies proof:**

```
Input:
- π (ZK proof from user)
- revealed_messages (deposit_address, deposit_timestamp)
- pk_issuer (Nimbus issuer public key - BLS12-381 G2)
- verifier_nonce (CEX generated)

Steps:
1. Verify BBS+ proof: BBS.Verify(π, revealed_messages, pk_issuer, nonce)
2. Extract deposit_address = 0x123...
3. CEX check address dengan TOOLS SENDIRI:
   - Query Chainalysis API: risk_score(0x123)
   - Check internal blacklist/whitelist
   - Check OFAC sanctions list
   - Apply CEX policy: approve/reject
4. (Optional) Check deposit_timestamp within acceptable range (e.g., < 90 days old)

Output: Accept/Reject withdrawal
```

**Security properties:**
- ✅ User can't forge proof (BBS+ signature unforgeable)
- ✅ User can't reuse proof for different CEX (nonce-bound)
- ✅ CEX learns ONLY what user revealed (ZK property)
- ✅ Nimbus never involved in verification (decentralized trust)
- ✅ **CEX use their OWN compliance tools** (not dependent on ASP partnership)

---

---

## Why NO ASP Partnership Needed?

### The Key Mental Shift

**OLD THINKING:**
```
"User needs to prove they're clean" 
→ Need trusted authority (ASP) to certify "clean"
→ Nimbus must integrate ASP
```

**NEW THINKING:**
```
"User needs to prove WHICH ADDRESS they deposited from"
→ CEX already has tools to check if address is clean (Chainalysis, etc.)
→ Nimbus just cryptographically binds address to credential
→ NO ASP NEEDED
```

---

### What Nimbus Provides

**Nimbus guardians attest:**
```
"This credential holder deposited X USDC from address Y at time T on chain Z"
```

That's it. **No judgment on "clean" or "dirty".**

---

### What Nimbus Does NOT Provide

- ❌ Compliance opinion ("this address is clean")
- ❌ ASP endorsement or partnership
- ❌ Regulatory guidance
- ❌ Whitelist/blacklist maintenance
- ❌ Risk scoring

**Nimbus = neutral payment rail, not compliance authority.**

---

### CEX Already Has Compliance Tools

Every major CEX already subscribes to:

| Tool | Cost/Year | What They Get |
|------|-----------|---------------|
| Chainalysis | $100k-500k | Address risk scoring, mixer detection, OFAC screening |
| Elliptic | $50k-200k | Transaction monitoring, sanctions screening |
| TRM Labs | $30k-100k | DeFi risk scoring, real-time alerts |

**They don't need Nimbus to tell them if address is clean. They already have tools for that.**

Selective disclosure just saves CEX from manually asking user "what's your deposit address?" via email/support ticket. Proof = automated compliance check.

---

### Comparison: With vs Without ASP Partnership

| Aspect | With ASP Partnership | Without ASP Partnership |
|--------|---------------------|-------------------------|
| **Nimbus role** | Maintain ASP relationships | Just sign credentials (neutral) |
| **ASP cost** | $10k/month per ASP | $0 (zero cost) |
| **BD overhead** | High (negotiate contracts) | Zero (no partnerships) |
| **Trust model** | CEX must trust Nimbus's ASP choice | CEX trust their own tools |
| **Flexibility** | Limited to partnered ASPs | Any verifier, any standard |
| **Proof content** | "I'm in ASP whitelist X" | "I deposited from address Y" |
| **CEX verify** | Check ASP root on-chain → verify proof | Verify proof → check address themselves |
| **Future-proof?** | No (ASP standards change) | Yes (CEX adapt independently) |
| **Regulatory risk** | Nimbus endorses ASP = liability | Nimbus neutral = no liability |
| **Privacy** | 8/10 (ASP root might leak info) | 9.5/10 (only address revealed) |

---

### Regulatory Advantage

**With ASP Model:**
```
Regulator: "Nimbus, why did you partner with ASP X?"
Nimbus: "We thought they were reputable..."
Regulator: "ASP X just got sanctioned for helping North Korea."
Nimbus: "Shit, we're implicated." ❌
```

**Without ASP Model:**
```
Regulator: "Nimbus, how do you ensure compliance?"
Nimbus: "We don't. We're neutral infrastructure. Each verifier (CEX/bank) 
         applies their own compliance standard using their own tools."
Regulator: "So if bad actor uses Nimbus?"
Nimbus: "They can't cash out at compliant CEX because CEX checks source address.
         We just cryptographically prove which address they used. 
         Liability is on CEX, not us."
Regulator: "Makes sense. You're like SWIFT (neutral payment rail)." ✅
```

**Much stronger legal position!**

---

### Example: Binance Integration (No ASP Needed)

**Binance already has:**
- Chainalysis Enterprise subscription ($500k/year)
- Internal compliance team (100+ people)
- Real-time OFAC monitoring
- KYC/AML policies

**Integration flow:**
1. User generate proof → upload to Binance
2. Binance verify BBS+ proof (5ms)
3. Binance extract deposit_address from proof
4. Binance query Chainalysis API: `risk_score(deposit_address)`
5. If risk_score < threshold → approve
6. If risk_score ≥ threshold → reject (same as if user manually told Binance address)

**No Nimbus API call. No ASP partnership. CEX use tools they already pay for.**

---

## Feasibility Analysis

### ✅ Pros (Why This Works)

1. **Existing Crypto Standards**
   - BBS+ signatures: IETF draft `draft-irtf-cfrg-bbs-signatures`
   - W3C Verifiable Credentials: `bbs-2023` cryptosuite
   - Production implementations: Hyperledger Aries, Dock Network, ZKryptium
   - **No bleeding-edge crypto needed**

2. **Minimal Nimbus Protocol Changes**
   - Issuer already signs credentials (BLS threshold signing exists)
   - Just add BBS+ signature in parallel to existing BLS sig
   - No contract changes (proof verification off-chain)
   - No consensus changes
   - **No ASP partnership needed** (CEX use their own tools)

3. **User Control**
   - User decides: reveal to who, when, what fields
   - CEX can't collude to deanonymize (each proof isolated)
   - User can refuse to disclose (stay fully private, accept CEX rejection)

4. **CEX Integration Lightweight**
   - CEX just needs BBS+ verifier library (open source)
   - No API calls to Nimbus (avoids centralization)
   - No blockchain queries needed (proof self-contained)
   - **CEX use existing Chainalysis/compliance tools** (already subscribed)

5. **Privacy vs Compliance Tradeoff Optimal**
   - Privacy: 9.5/10 (only deposit on-chain, spend hidden, selective disclosure)
   - Compliance: 10/10 (CEX satisfied, user proves clean source)
   - **No better solution exists without sacrificing one side**

---

### ❌ Cons (Challenges)

1. **Dual Signature System Complexity**
   - Current: BLS threshold signing for credentials
   - Needed: BLS + BBS+ signatures
   - **Mitigation:** BBS+ only for new deposits, optional feature flag

2. **CEX Adoption Friction**
   - CEX needs to integrate BBS+ verifier
   - CEX needs to trust Nimbus issuer key
   - CEX needs UI for "upload ZK proof" flow
   - **Mitigation:** Start with 1-2 friendly CEXs (partnerships), provide SDK + docs

3. **Issuer Key Trust Model**
   - CEX must trust Nimbus issuer key = Nimbus threshold guardians
   - If guardians compromised → can forge credentials
   - **Mitigation:** 
     - Guardians already securing $XXM TVL (same trust assumption)
     - Key rotation mechanism (DEC-016)
     - Transparent guardian operations (public logs)

4. **Credential Issuance Timing**
   - BBS+ credential needs to be issued AFTER deposit confirmed on-chain
   - Adds latency: deposit → wait confirmation → request credential → receive BBS+ sig
   - **Mitigation:** 
     - Guardians auto-issue credential after finality (15 blocks Arbitrum = ~15 sec)
     - User can spend immediately with old system, request BBS+ credential later for CEX

5. **Proof Size & Verification Cost**
   - BBS+ proof: ~200-300 bytes (acceptable)
   - Verification: ~5ms CPU (negligible for CEX backend)
   - **Not a blocker**

6. **Address Compliance Check Dependency**
   - CEX must have compliance tools (Chainalysis, Elliptic, etc.)
   - Small CEXs might not afford $100k/year Chainalysis subscription
   - **Mitigation:** 
     - Large CEXs (Binance, Coinbase, Kraken) already have these tools
     - Small CEXs can use cheaper alternatives (TRM Labs, Merkle Science)
     - Or small CEXs just check OFAC list (free, but limited)

---

## Comparison: Alternatives Considered

### Alternative 1: Mandatory ASP Gate

```
ASP verify wallet at deposit time → approve/reject
```

```
✅ Simple: ASP verifies at deposit time
❌ Privacy: ASP knows user wallet address (surveillance)
❌ Centralized: ASP single point of failure
❌ Cost: Nimbus pays ASP $10k/month
❌ Compliance: CEX still might not trust that ASP
```

**Verdict:** Privacy 5/10, Product 7/10. Not competitive with mixers.

---

### Alternative 2: On-Chain ZK Proof at Spend Time

```
User spends → contract verifies proof → logs "clean source" publicly
```

```
✅ Transparent: Everyone can see "this spend was clean"
❌ Privacy: Public signal reveals pattern (all clean spends linkable)
❌ Gas cost: Groth16 verification ~200k gas (~$0.50 per spend)
❌ Inflexible: Can't choose which verifier sees proof
```

**Verdict:** Privacy 6/10, Product 6/10. Expensive + leaks patterns.

---

### Alternative 3: Trusted Third-Party Attestation

```
User → sends wallet history to TTP (e.g., Chainalysis)
TTP → issues "clean source" certificate
User → shows cert to CEX
```

```
✅ Simple: No crypto needed
❌ Privacy: TTP sees EVERYTHING (full wallet history)
❌ Centralized: TTP single point of failure + surveillance
❌ Cost: TTP charges $50-500 per attestation
```

**Verdict:** Privacy 2/10, Product 7/10. Defeats purpose of Nimbus.

---

### Alternative 4: Relay Network for Deposit Privacy

```
User → sends USDC to relay address pool
Relay → deposits to Nimbus from relay address (not user address)
```

```
✅ Privacy: On-chain deposit unlinkable to user
❌ Compliance: CEX can't verify source (relay address meaningless)
❌ Custody risk: Relay holds funds temporarily
❌ Complexity: Need decentralized relay network (6+ months dev)
```

**Verdict:** Privacy 9/10, Product 3/10. Relay network useful but orthogonal to selective disclosure (can combine both).

---

## Recommended Approach: Phased Rollout

### Phase 1: Proof of Concept (2-3 months)

**Goal:** Validate technical feasibility + single CEX partnership

**Deliverables:**
1. Research doc (this file) ✅
2. BBS+ credential schema design
3. Reference implementation:
   - BBS+ issuer (guardian cluster modification)
   - BBS+ client (SDK proof generation)
   - BBS+ verifier (standalone library for CEX)
4. Demo with 1 friendly testnet CEX partner

**Success criteria:**
- User can generate valid proof
- CEX can verify proof in <10ms
- Privacy properties hold (no info leak beyond revealed fields)

**Estimated effort:** 1 engineer × 3 months

---

### Phase 2: Production Pilot (3-4 months)

**Goal:** Mainnet launch with 1-2 CEX partners

**Deliverables:**
1. Guardian cluster BBS+ signing integration
2. SDK update (nimbus-sdk WASM + JS bindings)
3. CEX integration SDK + docs
4. User documentation ("How to prove clean source")
5. Security audit (BBS+ implementation)
6. Launch with partner CEX (e.g., regional exchange)

**Success criteria:**
- 100+ users successfully prove compliance to CEX
- Zero security incidents
- <1% user support tickets re: proof generation

**Estimated effort:** 2 engineers × 4 months + audit $30k

---

### Phase 3: Ecosystem Expansion (ongoing)

**Goal:** Multi-CEX support, standardization

**Deliverables:**
1. Publish BBS+ verifier as open-source library (Rust + JS + Python)
2. CEX integration partnerships (target: 5-10 major CEXs)
3. "Nimbus Clean Source Standard" specification
4. Credential marketplace (optional: user can get credentials from multiple ASPs)

**Success criteria:**
- 5+ CEXs support Nimbus proof verification
- Industry recognition (conference talks, blog posts)
- 10k+ users using selective disclosure

**Estimated effort:** 1 PM + 1 BD + 1 engineer ongoing

---

## Risk Assessment

### Technical Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| BBS+ library bugs (forgery) | 🔴 Critical | Use audited libs (Hyperledger Aries), third-party audit |
| Guardian key compromise | 🔴 Critical | Same as current BLS signing (HSM + MPC), key rotation |
| Proof replay attacks | 🟡 Medium | Verifier nonce binding, short TTL (24h proof validity) |
| Revocation gap (sanctioned address) | 🟡 Medium | CEX real-time sanctions check, 90-day credential expiry |
| Cross-CEX correlation | 🟢 Low | Each proof isolated, different nonces, no linkability |

---

### Business Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| CEX adoption failure | 🔴 Critical | Start with 1-2 friendly CEXs, provide white-glove integration |
| User UX too complex | 🟡 Medium | Abstract proof generation (one-click in SDK), good docs |
| ASP partnership delays | 🟡 Medium | Phase 1 can use admin-registered roots, ASP optional |
| Regulatory change (ban mixers) | 🟡 Medium | Selective disclosure = compliance feature, not evasion |
| Competitor copies approach | 🟢 Low | Acceptable, validates market, Nimbus first-mover advantage |

---

### Regulatory Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| EU MiCA / TFR compliance | 🟡 Medium | Consult EU crypto lawyers, selective disclosure = KYC-friendly |
| US FinCEN mixer guidance | 🟡 Medium | Nimbus ≠ mixer (has compliance layer), legal opinion needed |
| OFAC sanctions enforcement | 🟡 Medium | CEX verifies revealed address against OFAC, Nimbus neutral |

**Key defense:** Selective disclosure is a **compliance enabler**, not evasion tool. User voluntarily proves clean source to CEX. Nimbus provides infrastructure, not judgment.

---

## Open Research Questions

### 1. Revocation Without Privacy Loss?

**Problem:** If address X sanctioned after credential issued, how to revoke?

**Possible solutions:**
- Accumulators (add/remove addresses, user proves non-membership)
- Credential expiry (force re-issuance every 90 days)
- CEX-side revocation list (CEX checks revealed address, not credential itself)

**Recommendation:** Combine expiry + CEX-side checks. Don't solve on-chain (breaks privacy).

---

### 2. Multi-Hop Privacy?

**Problem:** User deposits from clean address X, but X received from mixer Y. Is X still "clean"?

**Answer:** Out of scope for Nimbus. CEX decides policy ("1-hop clean", "2-hop clean", etc.). Nimbus only proves "deposited from address X". CEX check X dengan tools mereka (Chainalysis akan flag kalau X pernah interact dengan mixer).

---

### 3. Cross-Chain Source Verification?

**Problem:** User deposits USDC on Arbitrum, but source was Ethereum mainnet. How to prove?

**Possible solutions:**
- CCIP oracle (trusted bridge data)
- ZK light client proofs (ZK proof of Ethereum state)
- User reveal both addresses (Ethereum source + Arbitrum bridge)

**Recommendation:** Phase 3 problem. Start single-chain (Arbitrum only). CEX can check bridge transaction manually if needed.

---

### 4. Credential Transferability?

**Problem:** Alice proves to Binance. Can Bob reuse Alice's proof?

**Answer:** No. Proof includes spend nullifier (unique per user per spend). Bob can't forge Alice's nullifier without her credential. Safe.

---

### 5. Guardian Collusion Risk?

**Problem:** If 3/5 guardians collude, can they forge credentials for dirty addresses?

**Answer:** Yes (same as current BLS threshold). But:
- Guardians are publicly known (reputation at stake)
- Forgery detectable (CEX reports fake credential, guardian slashed)
- Same trust assumption as existing $XXM TVL custody

**Mitigation:** Increase threshold (4/7, 5/9), rotate guardians quarterly.

---

## Cost-Benefit Analysis

### Costs

| Item | Estimate |
|------|----------|
| Phase 1 R&D (3 months) | $60k (1 eng × $20k/mo) |
| Phase 2 Production (4 months) | $160k (2 eng × 4mo) + $30k audit |
| Phase 3 Ongoing (per year) | $180k (PM + eng, no BD needed) |
| **Total Year 1** | **$430k** |

---

### Benefits

| Benefit | Value |
|---------|-------|
| **Privacy competitive moat** | 10x user trust vs RAILGUN (no ASP surveillance) |
| **CEX integration unlocks TAM** | $12.6M → $126M revenue potential (10x market) |
| **Regulatory defense** | "We're neutral infrastructure" (stronger than ASP model) |
| **Protocol fees unchanged** | User pays same 45 bps, CEX verification free |
| **Network effects** | More CEXs → more users → more CEXs (flywheel) |
| **No ASP dependency** | $0 ASP fees (vs $120k/year), no BD overhead |

**ROI:** If selective disclosure enables 10% of potential CEX users → +$12M revenue/year. Break-even in <2 months.

**Cost savings vs ASP model:** $120k/year ASP fees + $60k/year BD overhead = **$180k saved annually**.

---

## Conclusion

### Is Selective Disclosure Feasible? ✅ YES

**Technical:** BBS+ mature, libraries exist, crypto sound, 200ms proof gen + 5ms verify.

**Business:** CEX adoption main risk, mitigated by friendly partnerships + SDK.

**Regulatory:** Compliance-friendly (voluntary disclosure), stronger than alternatives.

**Privacy:** 9.5/10 (only deposit on-chain, spend hidden, selective disclosure).

**Product:** 10/10 (CEX satisfied, user controls disclosure, Nimbus neutral).

---

### Recommendation: PROCEED with Phase 1 POC

**Next steps:**
1. Get user/founder buy-in on phased approach
2. Research existing BBS+ Rust libraries (ZKryptium vs Hyperledger Aries vs Dock)
3. Design credential schema (fields, TTL, revocation policy)
4. Identify 1 friendly CEX for pilot (regional exchange, not Binance/Coinbase yet)
5. Allocate 1 engineer for 3-month Phase 1

**Decision gate:** After Phase 1 POC, evaluate:
- Did proof generation work? (yes/no)
- Did CEX integration work? (yes/no)
- Did users understand UX? (survey)
- Does partnership look viable? (BD feedback)

If 4/4 yes → proceed Phase 2. If <3/4 yes → pivot or cancel.

---

## Appendix: Related Research

### Academic Papers
1. **Anonymous Attestation Using the Strong Diffie Hellman Assumption Revisited** (BBS+ original paper)
   - https://eprint.iacr.org/2016/663.pdf
   
2. **BBS Signatures** (IETF draft)
   - https://datatracker.ietf.org/doc/draft-irtf-cfrg-bbs-signatures/
   
3. **Verifiable Credentials Data Model** (W3C standard)
   - https://www.w3.org/TR/vc-data-model/

### Existing Implementations
1. **ZKryptium** (Rust, BBS+ native)
   - https://github.com/BryanStarbuck/zkryptium
   - Used by: Hyperledger Identus
   
2. **Hyperledger Aries BBS Signatures** (Rust)
   - https://github.com/hyperledger/aries-bbssignatures-rs
   - Production-ready, audited
   
3. **Dock Network Crypto** (Rust, BBS+ + threshold)
   - https://github.com/docknetwork/crypto
   - Includes threshold BBS+ (relevant for guardian cluster)

### Competitors Using Selective Disclosure
1. **Polygon ID** - ZK identity with selective disclosure (uses Circom + BabyJubJub)
2. **Worldcoin** - Proof of personhood with selective reveal (uses Groth16)
3. **Aztec Network** - Programmable privacy (uses PLONK, not BBS+ but similar concept)

**Key difference:** Nimbus = payment-focused, CEX compliance native, BBS+ optimal for multi-field credentials.

---

## Contact for Questions

- **Technical lead:** [TBD - assign engineer]
- **Business lead:** [TBD - assign BD/PM]
- **Legal counsel:** [TBD - EU + US crypto lawyers]

**Last updated:** 2026-10-04
**Status:** Awaiting founder decision on Phase 1 budget allocation
