# AES-Based Grinding for MPC-in-the-Head Signatures

**Authors:** Matthieu Rivain  
**Published:** 2026-08-04  
**Category:** Cryptographic protocols  

## Summary
Grinding is a technique which introduces a proof of work into the Fiat-Shamir transform: by constraining the challenge to satisfy a $w$-bit condition, forging a proof requires about $2^w/\varepsilon$ evaluations of the hash function instead of $1/\varepsilon$, where $\varepsilon$ is the soundness error of the underlying protocol. This allows one to select reduced parameters, yielding shorter proofs and signatures. Grinding is used in FAEST, MQOM and SDitH, the three MPC-in-the-Head schemes...
