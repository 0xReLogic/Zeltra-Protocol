# Archive: Merkle Mountain Belt (MMB) & DEC-057

- **Origin Paper:** arXiv:2511.13582v1 (*The Merkle Mountain Belt* by Alfonso Cevallos, Robert Hambrock, Alistair Stewart, Nov 2025).
- **Evaluation Document:** `DEC-057-merkle-mountain-belt-evaluation-and-selective-incrementality-synthesis.md`
- **Prototype Implementation:** `mmb.rs`
- **Archival Rationale:**
  - On-chain and ZK circuit adoption rejected: MMB's asymmetric lazy-merging queues double R1CS constraints (>650 constraints/level), which would blow up mobile proving time from 1.1s to 8s (DEC-041).
  - Off-chain increment proof ($O(\log k)$) prototype confirmed up to 6,000x speedup at lag $k = 10,000$, but is archived from production crates (`zeltra-core`, `zeltra-sdk`) for Phase 1 launch to maintain a lean, minimal audit surface for external auditors.
  - Preserved here as an authoritative R&D asset for future scaling phases (1,000,000+ daily transactions).

