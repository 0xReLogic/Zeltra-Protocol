# ZKBoost: Zero-Knowledge Verifiable Training for XGBoost

**Authors:** Nikolas Melissaris, Antigoni Polychroniadou, Akira Takahashi, Chenkai Weng, Jiayi Xu

**Category:** Cryptographic protocols

**Date:** 2026-05-12

**PDF:** 2026/202

## Abstract

Gradient boosted decision trees, particularly XGBoost, are among the most effective methods for tabular data. As deployment in sensitive settings increases, cryptographic guarantees of model integrity become essential. We present ZKBoost, the first zero-knowledge proof of training (zkPoT) protocol for XGBoost, enabling model owners to prove correct training on a committed dataset without revealing data or model parameters. Naively re-executing XGBoost training in ZK would incur prohibitive...
