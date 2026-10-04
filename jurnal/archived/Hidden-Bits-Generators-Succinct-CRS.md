# Hidden-Bits Generators with Succinct CRS and Openings

**Authors:** Pedro Branco, Nico Döttling, Yao-Ching Hsieh, Abhishek Jain, Akshayaram Srinivasan, Brent Waters  
**Published:** 2026-09-25  
**Category:** Cryptographic protocols  

## Summary
Feige, Lapidot, and Shamir [FOCS'90] presented a generic approach for building non-interactive zero-knowledge proofs via the notion of hidden-bits generators (HBGs). At a high level, an HBG allows a prover to statistically commit to a $k$-bit pseudorandom string in the CRS model with a succinct commitment. Later, the prover can open this pseudorandom string at an arbitrary subset $S \subseteq [k]$ of positions by providing a proof $\pi_S$. The hiding property requires that unopened positions...
