**2026/1329**
(PDF)
Last updated: 2026-09-19

**Flock: Fast Proving for Batch Boolean Computations**
Benedikt Bünz, Ron Rothblum, William Wang

**Category:** Cryptographic protocols

For many applications of SNARKs, a key bottleneck is proving large batches of standard cryptographic hash evaluations, such as SHA-256, Keccak, or BLAKE3. We introduce Flock, a hash-based SNARK for extremely fast proving of such batched Boolean computations. Flock proves batches of the same R1CS circuit (plus input/output relations between them), can prove hash-chains and Merkle path openings, and in principle can be extended to full-fledged hash-based signature verification. At its core,...

**Relevance to Nimbus:** **HIGH** - Batch hash proving, applicable untuk batch spend verification di nimbus-contracts