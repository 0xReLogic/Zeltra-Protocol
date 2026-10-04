# VERIDP: Verifiable Differentially Private Training

**Authors:** Behzad Abdolmaleki, Amir R. Asadi, Vahid R. Asadi, Stefan Köpsell, Bhavish Mohee, Nahid Roustaeifar, Maryam Zarezadeh

**Category:** Cryptographic protocols

**Date:** 2026-03-24

**PDF:** 2026/542

## Abstract

Stochastic Gradient Descent (SGD) is the foundation of modern machine learning (ML). In privacy-sensitive settings, gradients can reveal details about individual data points. Differential Privacy (DP) protects sensitive data during ML training by clipping gradients and adding calibrated Gaussian noise. However, existing frameworks assume semi-honest participants, which fails in adversarial or federated environments where malicious actors can bypass or alter the noise addition process...
