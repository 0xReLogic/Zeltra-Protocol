**2025/1897**
(PDF)
Last updated: 2026-05-11

**Dynark: Making Groth16 Dynamic**
Tianyu Zhang, Yupeng Ouyang, Yupeng Zhang

**Category:** Cryptographic protocols

Modern zkSNARK constructions require witnesses to be fixed before proof generation. When witness changes, provers must recompute proofs from scratch even if the new witness is close to the old one, creating inefficiency. Dynark makes Groth16 dynamic by enabling efficient updates to proofs as witness changes incrementally. This enables applications with dynamically changing witnesses where small modifications preserve proof structure and reduce computation.
