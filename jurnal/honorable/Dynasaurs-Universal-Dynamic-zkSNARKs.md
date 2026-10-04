# Dynasaurs: Efficient Universal Dynamic zkSNARKs from Sparse Linear Arguments

**Authors:** Martí Batista, Álvaro Montes, Nikitas Paslis, Carla Ràfols  
**Published:** 2026-09-04  
**Category:** Public-key cryptography  

## Summary
Dynamic zkSNARKs were recently introduced by Wang et al. [Eurocrypt, 2026]. This primitive extends standard zkSNARKs with an update algorithm that adapts a proof to a new statement in time sublinear in the circuit size, provided the witness changes in few positions. However, existing constructions either need a circuit-specific setup or, in the universal case, send over $130$ group elements and require over $180$ pairings. As is the case for universal zkSNARKs, dynamic ones can be built...
