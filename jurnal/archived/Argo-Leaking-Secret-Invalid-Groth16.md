# Argo: Leaking a Secret from an Invalid Groth16 Proof

**Authors:** Liam Eagen, Ying Tong Lai  
**Published:** 2026-09-23  
**Category:** Applications  

## Summary
We present Argo, a randomized encoding scheme for the invalidity of pairing-based SNARKs like Groth16. Argo allows an encoder to encode a proof such that a decoder can derive a secret if the proof is invalid. We prove the selective security of invalidity Argo, as well as the adaptive security of validity Argo, under DDH in one source group of a bilinear pairing in the standard model, a standard assumption that holds in the generic group model used to analyze Groth16. We show how to...
