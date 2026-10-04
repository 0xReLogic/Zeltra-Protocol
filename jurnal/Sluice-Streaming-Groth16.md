**2026/1758**
(PDF)
Last updated: 2026-08-21

**$\textsf{Sluice}$: Prove-Phase Bounded-Memory Groth16 via Read-Write Streaming**
Kyeongtae Lee, Jihye Kim, Hyunok Oh

**Category:** Cryptographic protocols

We present $\textsf{Sluice}$, a read-write streaming Groth16 prover that reduces $\textit{prove-phase}$ random-access working memory from $\mathcal{O}(N)$ to $\mathcal{O}(\log N)$ once the CRS, QAP, and witness are materialized as private streams. It preserves the standard Groth16 interface: a proof of 3 group elements, 3-pairing verification, and unchanged verifier contracts. Our key technical contribution is $\textit{Split-Butterfly-Merge}$ ($\mathsf{SBM}$), an NTT algorithm in the...

**Relevance to Nimbus:** **HIGH** - Groth16 prover optimization dengan bounded memory, directly applicable ke nimbus-core proving