**2026/2060**
(PDF)
Last updated: 2026-09-16

**DelegProof: A Machine-Checked Security Analysis of EIP-7702 Delegation**
Rong Qian, Yu Cheng, Lingyu Gao, Yuchang Zhang, Zengli Guo

**Category:** Applications

EIP-7702, live on Ethereum since the Pectra upgrade, lets externally owned accounts delegate their execution to arbitrary contract code with a single signature. The consequences are measurable: 63% of observed delegations point to malicious contracts, with $2.36M in confirmed losses, and ecosystem guidance already warns about cross-chain replay and front-run initialization. What is missing is a formal account of the problem: the 7702 delegation lifecycle has no formal treatment, and none of...

**Relevance to Nimbus:** **HIGH** - Directly addresses Gap 9 (AI Agent Gasless Transactions), EIP-7702 account delegation untuk SDK