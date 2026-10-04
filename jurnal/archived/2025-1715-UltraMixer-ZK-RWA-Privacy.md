# UltraMixer: A Compliant Zero-Knowledge Privacy Layer for Tokenized Real-World Assets

**Authors:** Zonglun Li, Hong Kang, Xue Liu

**Category:** Applications

**Date:** 2025-09-21

**PDF:** 2025/1715

## Abstract

Real-world-asset (RWA) tokens endow underlying assets with fractional ownership and more continuous settlement, yet recording these claims on transparent public ledgers exposes flows and positions, undermining market confidentiality. Practical deployments must reconcile enforceable access control with principled privacy once assets are shielded. We present UltraMixer, a noncustodial privacy layer natively compatible with ERC-3643. Compliance is enforced at the boundary via zero-knowledge proofs, enabling privacy-preserving transfers while preserving regulatory oversight. Our system achieves efficient transaction processing with sub-second latency and supports selective disclosure for compliance checks.

## 1. Introduction

Tokenized RWA markets face a fundamental tension: transparency enables auditability but compromises market confidentiality. Market participants need to protect their trading strategies, position sizes, and identity while remaining compliant with regulatory requirements. UltraMixer addresses this challenge by combining zero-knowledge proofs with access control mechanisms.

Key contributions:
- A privacy layer compatible with ERC-3643 (Security Token Standard)
- Zero-knowledge proofs for privacy-preserving transfers
- Regulatory access control at the protocol level
- Efficient implementation with sub-second transaction latency

## 2. System Overview

UltraMixer operates as a privacy layer on top of existing token infrastructure:

```
User Wallet → UltraMixer Privacy Layer → On-Chain Verification
     ↓                                    ↓
  shielded notes                    ZK proofs + KYC checks
```

### Key Components

1. **Shielded Pool**: Holds assets in a privacy-preserving manner
2. **ZK Circuit**: Proves validity of transfers without revealing amounts or participants
3. **Compliance Gateway**: Enforces KYC/AML checks via selective disclosure
4. **Registry**: Maintains whitelisted participants and revocation lists

## 3. Technical Design

### 3.1 Privacy Mechanism

UltraMixer uses zero-knowledge proofs to hide:
- Sender and receiver identities
- Transfer amounts
- Transaction timing

The ZK circuit validates:
- Input note validity (exists in commitment tree)
- Balance conservation (input = output + fee)
- Sender authorization (valid signature)

### 3.2 Compliance Enforcement

Compliance is enforced at the protocol boundary:
- Only whitelisted participants can interact
- Regulatory bodies can request selective disclosure
- Revocation lists for compromised or sanctioned entities

### 3.3 ERC-3643 Compatibility

UltraMixer maintains compatibility with ERC-3643 by:
- Using the same interface patterns
- Supporting the `transferWithAuth` pattern
- Integrating with existing identity management

## 4. Use Cases

### 4.1 Private Token Transfers

Participants can transfer tokens without revealing:
- Who sent the tokens
- How much was transferred
- When the transfer occurred

### 4.2 Regulatory Audits

Regulators can request:
- Disclosure of specific transactions
- Verification of compliance with limits
- Confirmation of participant eligibility

### 4.3 Corporate Actions

Corporate actions (dividends, buybacks) can be processed while preserving privacy for participants.

## 5. Security Considerations

### 5.1 Trusted Setup

UltraMixer requires a trusted setup ceremony for ZK-SNARK parameters. We use a multi-party computation (MPC) ceremony with participation from multiple independent entities.

### 5.2 Key Management

- Private keys are stored in HSMs
- Threshold signatures for critical operations
- Automatic key rotation

### 5.3 Access Control

- Multi-factor authentication for sensitive operations
- Role-based access control
- Audit logging for all access

## 6. Performance

- Transaction latency: < 1 second
- Proof generation: < 500ms on commodity hardware
- Verification gas cost: ~250,000 gas on Ethereum

## 7. Related Work

- **Aztec Connect**: Privacy for ERC-20 tokens, but not RWA-compatible
- **Tornado Cash**: Privacy for anonymous transfers, no compliance features
- **Numeraire**: Privacy for staking, not general-purpose RWA

## 8. Conclusion

UltraMixer enables private RWA token transfers while maintaining regulatory compliance. The system is natively compatible with ERC-3643 and provides efficient, secure privacy for market participants.

## References

1. Li, Z., Kang, H., & Liu, X. (2025). UltraMixer: A Compliant Zero-Knowledge Privacy Layer for Tokenized Real-World Assets.
2. ERC-3643: Tokenized Assets Standard. https://eips.ethereum.org/EIPS/eip-3643
3. ZK-SNARKs for Privacy-Preserving Transfers. Bitcoin Research, 2024.
