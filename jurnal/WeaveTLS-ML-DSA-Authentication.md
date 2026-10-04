**2026/1828**
(PDF)
Last updated: 2026-08-28

**WeaveTLS: High-Throughput Cross-Connection ML-DSA Authentication in Mutual TLS**
Ganqin Liu, Hao Cheng, Jipeng Zhang

**Kategori:** Implementasi

Mutual TLS (mTLS) authenticates both peers dan therefore incurs post-quantum signature costs on every connection. Concurrent handshakes expose independent ML-DSA operations, but executing them jointly is difficult: signing is rejection-divergent, verification uses heterogeneous keys, synchronous TLS APIs expose authentication work one connection at a time. WeaveTLS is wire-transparent architecture executing ML-DSA authentication across concurrent TLS connections.