# Gas & Latency Benchmark Report (Arbitrum Sepolia)
Generated on: Sat Jun 13 09:08:36 2026

## 1. Gas Receipt Comparison (No Assumptions)
| Batch Size | Total Gas Used | Gas per Spend | Gas Saving % vs Single |
| :---: | :---: | :---: | :---: |
| 1 | 644,870 | 644,870 | 0.00% |
| 2 | 1,103,474 | 551,737 | 14.44% |
| 4 | 1,997,427 | 499,356 | 22.56% |
| 8 | 3,788,555 | 473,569 | 26.56% |

## 2. Latency Metrics (seconds)
| Metric | Batch Size 1 | Batch Size 2 | Batch Size 4 | Batch Size 8 |
| :--- | :---: | :---: | :---: | :---: |
| **API-to-Broadcast p50** | 4.09s | 2.80s | 3.36s | 2.28s |
| **API-to-Broadcast p95** | 4.09s | 2.80s | 3.36s | 2.28s |
| **API-to-Broadcast p99** | 4.09s | 2.80s | 3.36s | 2.28s |
| **API-to-Confirmed p50** | 4.09s | 2.80s | 3.36s | 2.28s |
| **API-to-Confirmed p95** | 4.09s | 2.80s | 3.36s | 2.28s |
| **API-to-Confirmed p99** | 4.09s | 2.80s | 3.36s | 2.28s |