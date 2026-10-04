**2019/587**
(PDF)
Last updated: 2021-01-18

**Polygraph: Accountable Byzantine Agreement**
Pierre Civit, Seth Gilbert, Vincent Gramoli

**Category:** Cryptographic protocols

In this paper, we introduce \emph{Polygraph}, the first accountable Byzantine consensus algorithm. If among $n$ users $t<n/3$ are malicious then it ensures consensus; otherwise (if $t \geq n/3$), it eventually detects malicious users that cause disagreement. Polygraph is appealing for blockchain applications as it allows them to totally order blocks in a chain whenever possible, hence avoiding forks and double spending and, otherwise, to punish (e.g., via slashing) at least $n/3$ malicious...

**Relevance to Nimbus:** MEDIUM - Accountable Byzantine agreement untuk guardian cluster consensus dan slashing detection