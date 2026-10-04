# Chimera: A Hybrid GPU Backend for Sumcheck Acceleration in Zero Knowledge Provers

**Authors:** Kashfia Farheen, Nektarios Georgios Tsoutsos  
**Published:** 2026-07-07  
**Category:** Implementation  

## Summary
Zero-knowledge proof systems are increasingly relying on the Sumcheck protocol to avoid the FFT-heavy structure of earlier SNARK designs. Sumcheck is well suited for GPU acceleration; it consists of sequential rounds where each round performs regular, parallelizable operations over large multilinear evaluation tables. The focus is on how to organize this work across rounds: intuitively, the active polynomial state should remain close to the device that processes it, the CPU-GPU boundary...
