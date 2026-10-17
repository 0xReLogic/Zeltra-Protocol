Scroll to top
2026/2132 (PDF)
Last updated:  2026-09-20
United: Shuffle-Free Multiparty PSU via Oblivious Invertible Sketches
Cryptographic protocols
Truong Son Nguyen, Lucas Piske, and Ni Trieu
Hide abstract
Multiparty Private Set Union (MPSU) allows multiple parties to compute the union of their private sets while revealing no additional information beyond the prescribed output. Existing practical MPSU protocols typically manipulate individual elements and rely on a distributed shuffle to hide element ownership, introducing an 
 round-complexity bottleneck.

We present United, a shuffle-free MPSU framework based on a new data-structure abstraction, Oblivious Insert-Only Invertible Sketches (OIIS). Rather than anonymizing an element-wise intermediate representation, the parties directly construct a protected symmetric sketch of the union. Conditional oblivious insertion ensures that each distinct union element is inserted exactly once while hiding the insertion condition and modified locations.

We provide two realizations of OIIS. Our HE-based construction instantiates OIIS with a standard Invertible Bloom Lookup Table protected by multi-key partially homomorphic encryption, while our MPC-based construction uses additive secret sharing and introduces a new row-local power-sum OIIS technique for efficient conditional insertion. We implement and evaluate both constructions. Across the evaluated configurations, our MPC-based construction achieves up to 
 lower runtime in LAN settings, while our HE-based construction achieves up to 
 lower runtime in WAN settings compared with prior MPSU protocols.
2026/2131 (PDF)
Last updated:  2026-09-20
Forging 1024-bit RSA signatures in nearly SNFS time
Attacks and cryptanalysis
Laura Shea, Miro Haller, Adam Suhl, Nadia Heninger, and Emmanuel Thomé
Hide abstract
The security of RSA is generally understood to be based on the complexity of factoring, and key size parameters are extrapolated from the general number field sieve (GNFS). However, this may not accurately represent RSA security in practical scenarios.

An under-appreciated 2007 algorithm of Joux, Naccache, and Thomé allows an attacker to forge RSA signatures after temporary access to a raw RSA signing/decryption oracle in time close to the *special* number field sieve (SNFS) without factoring the key. We implement and run this algorithm for 1024-bit RSA. In total, the attack took 1380 CPU core-years over five calendar months, and made 
 oracle queries. Most of this time is precomputation; after the precomputation the attacker can forge any signature of choice, offline, in 180 core-years.

We carried out our attack using a hardware security module (HSM) as the signing oracle, thus demonstrating the ability to impersonate the HSM through black-box API interactions, without exfiltrating the key. Blind RSA schemes also provide such a signing oracle.

Extrapolating our empirical running times to larger key sizes, we conclude that the concrete security of RSA with a signing oracle should be 15 to 30 bits lower than the factoring-based security estimates for the 1024-bit to 4096-bit RSA parameters that are common in practice. Even 4096-bit RSA does not appear to meet a 128-bit security level in this attack model. This highlights a gap in current RSA-type security assumptions, and gives classical cryptanalytic evidence in favor of moving away from RSA entirely during the current post-quantum transition.
2026/2130 (PDF)
Last updated:  2026-09-20
On the Necessity of Anamorphic Encryption for Covert Communication
Foundations
Trey Li
Hide abstract
Covert communication can be achieved without anamorphic encryption.
2026/2129 (PDF)
Last updated:  2026-09-20
Cryptanalysis of a Polynomial-Based Redactable Blockchain
Attacks and cryptanalysis
Alexander Ushakov
Hide abstract
We analyze a recently published construction of redactable blockchains
based on multivariate polynomial equations. We show that its public
integrity conditions reveal sufficient algebraic structure to recover
the relevant private evaluation points efficiently.
As a consequence, 
the blockchain unauthorized redaction problem reduces to
univariate root finding over a finite field followed by solving a
linear system.
We also discuss
an alternative attack based on linearization.
2026/2128 (PDF)
Last updated:  2026-09-20
Schnorr Signatures and MuSig2 Are Jointly Secure, Even in Deterministic Wallets
Public-key cryptography
Renas Bacho, Yanbo Chen, Poulami Das, Julian Loss, Tim Ruffing, and Benedikt Wagner
Hide abstract
Modern cryptocurrency wallets use Schnorr signatures together with public, deterministic key derivation: by repeatedly rerandomizing a single master public key, an unlimited number of public keys for incoming payments can be derived without access to secret data. Moreover, some wallets support multi-signature schemes such as MuSig2 for optionally aggregating derived public keys from 
 distinct parties into "
-of-
" public keys, so that funds received on such aggregated keys can be spent only when authorized by all 
 parties. However, these advanced key-management techniques stretch the underlying cryptographic schemes beyond the guarantees provided by existing security proofs. Although deterministic Schnorr wallets and MuSig2 have each been proven secure in isolation, the provable security of their composition---despite its deployment in wallets---has not been established thus far.

The goal of this work is to narrow this gap between theory and practice. We provide the first formal security analysis of the joint use of single-signer Schnorr signatures and MuSig2, both under rerandomization and in deterministic wallets. First, we introduce the notion of a (rerandomizable) joint-signature scheme, in which an honest signer uses the same secret key for single-signer signatures and multi-signatures. Within this model, we prove tight security of Schnorr signatures plus MuSig2 under the algebraic one-more discrete logarithm (AOMDL) assumption in the combination of the algebraic group model (AGM) and the random oracle model (ROM). Finally, we provide a formal model of deterministic wallets with joint-signature schemes and show that Schnorr plus MuSig2 remains secure under public, deterministic key derivation.
2026/2127
Last updated:  2026-09-20
Polynomial Time Cryptanalytic Extraction of Deep Neural Networks in the Limited Architecture Knowledge Setting
Attacks and cryptanalysis
Yiqing Li and Gaoli Wang
2026/2126 (PDF)
Last updated:  2026-09-20
Cryptanalysis of LDVR: Polynomial-Time Adaptive Attacks against Threshold Schnorr Signatures
Cryptographic protocols
Àlex Rodríguez García and Stefano Tessaro
Hide abstract
We present the first polynomial-time attacks against certain parameter choices of the low-dimensional vector representation (LDVR) problem, introduced by Crites, Katz, Komlo, Tessaro, and Zhu (CRYPTO '25) to prove the adaptive security of FROST in the Algebraic Group Model. Building on work by Crites and Stewart (CRYPTO '25), they also show that every attack against LDVR directly yields an adaptive attack—i.e., one leveraging adaptive corruptions—against any threshold Schnorr signature that exposes to the attacker the values 
 for the secret-key shares 
 held by the signers, where 
 is a generator of the underlying group. This class of protocols includes FROST (Komlo and Goldberg, SAC '20), Sparkle (Crites, Komlo, and Maller, CRYPTO '23), Lindell's three-round protocol (CiC '24), along with other masking-free protocols. Consequently, our results give the first polynomial-time adaptive attacks against a class of practical threshold signatures. 

Our attacks require a large, but still polynomial, number of signers: asymptotically, we require 
 signers and corrupt 
 of them. For the prime-order subgroup of Curve25519, concrete and optimized instantiations of our attack require 
 signers and corrupt 
 of them. This exceeds the signer counts commonly encountered in practice, but falls within the scenarios considered by NIST's call for threshold signatures.  At the theoretical level, our results also yield a non-artificial separation between static and adaptive security based on a real-world multi-party protocol. 

Our results arise from a new reduction from LDVR to structured subset-sum problems determined by the evaluation points of the polynomial used to secret-share the signing key. We further show that allowing these evaluation points to be chosen adversarially can lead to substantially more efficient attacks.
2026/2125 (PDF)
Last updated:  2026-09-20
A Survey of Constraint-Based Side-Channel Analysis: From Algebraic Attacks to Exact Probabilistic Inference
Attacks and cryptanalysis
Gaurav Kumar
Hide abstract
Over the last fifteen years, side-channel analysis (SCA) against implementations of both classical and post
quantum cryptography has undergone a quiet but fundamental change of paradigm: from treating leakage
as evidence to be combined with an algorithm’s structure into a system of exact equations to be solved, to
treating it as evidence to be combined into a joint probability distribution to be queried. This survey gives a
systematic account of that evolution through four paradigms: Algebraic SCA (ASCA), which encodes leakage
as hard Boolean or algebraic constraints and solves the resulting system with SAT or Gröbner-basis solvers;
Tolerant/weighted Algebraic SCA (TASCA), which relaxes hard constraints into pseudo-Boolean costs and
recovers a maximum-a-posteriori key via combinatorial optimization; Soft Analytical SCA (SASCA), which
retains leakage as full likelihood functions attached to a factor graph and approximates the posterior with
loopy belief propagation; and Exact SASCA (ExSASCA), which replaces the uncertified approximation of
loopy belief propagation with exact inference on a knowledge-compiled tractable probabilistic circuit. Our
central contribution is a single Bayesian factor-graph formulation under which all four paradigms are shown
to be different representations of the same leakage-augmented joint distribution combined with different
inference queries and solvers: we prove, in a precise but lightweight sense, that ASCA is the zero-temperature
limit of TASCA, that TASCA is a maximum-a-posteriori restriction of SASCA’s model, and that ExSASCA
is SASCA with its approximate solver replaced by an exact one on the identical graphical model. Building
on this formulation, we develop a taxonomy that organizes the literature along a representation axis (hard,
weighted, probabilistic) and a solver axis (satisfiability, combinatorial optimization, approximate message
passing, exact tractable inference), survey the concrete attack literature in each paradigm on both AES and
NTT-based post-quantum schemes (Kyber, Dilithium, Falcon/FN-DSA), and analyze the computational and
security trade-offs of each approach, including its implications for what a security evaluator may soundly
conclude from a failed attack. We close with open problems at the current frontier of tractable probabilistic
inference for cryptographic implementation security.
2026/2124 (PDF)
Last updated:  2026-09-20
Every Signing Leaks: Breaking Falcon via Floating-Point Conversion Leakage
Attacks and cryptanalysis
Yuanyuan Zhou, Weijia Wang, Yiteng Sun, and Yu Yu
Hide abstract
Falcon offers compact signatures and well-studied mathematical security, but its side-channel security remains a critical challenge. In particular, its floating-point Gaussian sampler constitutes a major source of side-channel leakage, while protecting it efficiently is difficult because of its reliance on floating-point arithmetic. However, existing attacks targeting the sampler under realistic noisy-leakage conditions typically require thousands of traces and are evaluated on unoptimized implementations.
In this paper, we present a new attack on Falcon's sampler, which reduces the number of required traces by more than two orders of magnitude compared with prior attacks under realistic noisy-leakage conditions and works on both unoptimized (\texttt{-O0}) and highly optimized (\texttt{-O3}) builds. Our attack targets the floating-point Gaussian centers 
 used by the sampler at the recursion leaves of the \texttt{ffSampling} procedure. Specifically, it exploits signed-exponent leakage arising when 
 is converted from an integer back to floating-point representation, as well as the weaker sign-only leakage. Signed-exponent leakage places the integer part 
 of each center in one of 
 signed power-of-two intervals, whereas sign-only leakage divides it into two intervals according to its sign. We propose CB-MLE, which uses gradient descent to obtain an estimate 
 whose computed Gaussian-center classifications are consistent with the observed classifications under the profiled confusion matrix. An ISD-style search tests subsets of the more reliable coefficients in 
 with LLL/BKZ on the corresponding reduced NTRU lattices to recover the full key.
We collect power traces from the Falcon implementation in the PQClean library running on an ARM Cortex-M4 microprocessor. Our Gaussian-center classifier achieves single-trace classification accuracies of 
 and 
 at the \texttt{-O0} and \texttt{-O3} optimization levels, respectively, confirming exploitable signed-exponent leakage from the Gaussian centers. Using these classification results, the full attack recovers Falcon-
 keys with success rates of 
 from 
 signatures at \texttt{-O0} and 
 from 
 signatures at \texttt{-O3}. For Falcon-
, it reaches 
 from 
 signatures at \texttt{-O0} and 
 from 
 signatures at \texttt{-O3}.
2026/2076 (PDF)
Last updated:  2026-09-20
Symmetry-Graded Digit Extraction Framework for Faster BGV Bootstrapping
Public-key cryptography
Zhenyu Xiong, Mingsheng Wang, Zhedong Wang, and Han Wang
Hide abstract
Bootstrapping is the bottleneck of BGV/BFV homomorphic encryption, and for large plaintext primes 
 its cost is dominated by digit extraction. Recently, this stage has been accelerated along two separate routes. The first lowers the degree of the digit-extraction polynomial: the bounded-support construction of Ma et al. (Eurocrypt'24) confines its support, and the order-four filter of Xiong et al. (to appear in Asiacrypt'26) removes three quarters of its monomials. The second lowers the depth of its evaluation: the Galois norm map of Okada et al. (Asiacrypt'23) and Zhao et al. (Crypto'26) evaluates a degree-
 factor in logarithmic depth. However, how the two routes relate and whether they compose has remained open.

We propose a symmetry-graded framework that views the two evaluation routes as commuting group actions on the digit-extraction polynomial. Under this unified perspective, the existing evaluators correspond to different specializations of 
. For lower degree, we identify a novel rank-two lattice structure underlying digit extraction. The crystallographic restriction limits the filter order to 
. In particular, the methods of Ma et al. and Xiong et al. correspond to 
 and 
, respectively. Moreover, our framework derives a sparser order-six digit-extraction polynomial, enabling order-six symmetry for Mersenne primes. For lower evaluation cost, we construct a composed evaluator that first folds 
 using the scalar filter and then evaluates the folded polynomial via the slot ring's Galois norm map. Since the rotation and Frobenius actions commute, the two optimizations can be combined within the same framework. Consequently, for any 
 we select the optimal 
 and evaluate digit extraction in 
 non-scalar multiplications.

On 13 general cyclotomic rings at 
-bit verified security, our single-threaded HElib implementation improves upon the state-of-the-art evaluator of Ma et al., accelerating digit extraction by 
–
 and thin bootstrapping by 
–
, while modifying only the digit-extraction stage. In particular, on Ma et al.'s set IV with the Mersenne prime 
 and set V with 
, digit extraction is accelerated by 
 and 
, respectively, reducing the total bootstrapping time from 180.4 s to 85.5 s and from 217.6 s to 103.1 s. The implementation is publicly available, and the core analysis is machine-checked in Lean 4.
2026/2123 (PDF)
Last updated:  2026-09-20
Two Standard Deviations Are Necessary for the Kadison-Singer Problem
Foundations
Zhao Song and Song Yue
Hide abstract
Let 
 be the least constant such that, for every finite family of vectors 
 and independent finitely supported real random variables 
, there are values 
 satisfying 
 where 
 denotes the operator norm. This rank-one matrix discrepancy formulation generalizes the signing formulation of the Kadison--Singer problem [KS59], resolved by Marcus, Spielman, and Srivastava [MSS15b].

Let 
 be the least constant such that, for every 
 and every finite family in 
, in every dimension 
, satisfying 
 and 
, there are signs 
 such that 
 Taking the 
 to be independent symmetric signs gives 
, since 
 under these hypotheses. The general formulation allows arbitrary finite real supports and does not require 
. Kyng, Luh, and Song [KLS20] proved 
. We prove 
. This improves the bounds 
 of Xie, Xu, and Zhu [XXZ21].

We conjecture that the optimal constant is .
2026/793 (PDF)
Last updated:  2026-09-20
Oriole: Adaptively Secure Partially Non-Interactive Threshold Signatures from Lattices
Public-key cryptography
Kaijie Jiang, Hoeteck Wee, and Chenzhi Zhu
Hide abstract
We present the first lattice-based, partially non-interactive threshold signature scheme that tolerates the adaptive corruption of up to T −1 signers, where T is the signing threshold. Our construction relies on the MSIS and MLWE assumptions, and has two rounds, of which only the second is message-dependent. We substantially improve upon prior adaptively secure lattice-based schemes (CRYPTO ’24 and EUROCRYPT ’26), which require at least two message-dependent rounds. In addition, compared to prior lattice-based partially non-interactive schemes (CRYPTO ’24, S&P ’25, CRYPTO ’25), we achieve better communication complexity.
2026/2122 (PDF)
Last updated:  2026-09-20
Continuum: Concretely Efficient Asynchronous Dynamic MPC with Guaranteed Output Delivery
Cryptographic protocols
Wenxuan Yu, Minghui Xu, Yuan Lu, Yingzi Gao, and Xiuzhen Cheng
Hide abstract
Traditional secure multiparty computation (MPC) protocols assume a fixed set of participants throughout the computation. Dynamic MPC (DMPC) relaxes this assumption by permitting parties to join or leave during the process. However, practical deployment of DMPC in Internet settings favors tolerating asynchrony and achieving guaranteed output delivery (GOD). Recently, AD-MPC [CCS 2025] introduced an asynchronous DMPC (ADMPC) achieving this goal, but its reliance on per-epoch preprocessing and masking randomness incurs substantial concrete overhead. In this paper, we present Continuum, a concretely efficient ADMPC protocol with GOD. We introduce an aggregated hand-off protocol that ensures transfer consistency across epochs while eliminating per-epoch masking randomness, and a batch multiplication protocol based on the BGW paradigm that defers degree reduction to the hand-off phase and aggregates verification for all multiplications into succinct proofs. Together, these protocols enable a non-interactive computation phase with interaction concentrated in an efficient hand-off phase, overcoming the primary concrete inefficiencies of prior ADMPC designs. We implement Continuum and evaluate its performance on up to 128 geographically distributed servers. In experiments on 22 servers with circuits of depth 6 and width 100, Continuum reduces linear-gate transfer latency from 76.60s to 15.13s (80% reduction) and multiplication-gate evaluation time from 476.62s to 35.67s (93% reduction), compared with AD-MPC, while preserving strong security guarantees.
2026/646 (PDF)
Last updated:  2026-09-20
On Optimal Information-Theoretic Security in Symmetric Encryption under Low-Entropy Keys
Secret-key cryptography
Haibo Cheng, Haijie Su, Dongyi Li, Wenting Li, Ping Wang, and Kaitai Liang
Hide abstract
Low-entropy keys such as passwords and biometrics are pervasive, yet classical information-theoretic guarantees, such as perfect secrecy and entropic security, cannot generally be achieved in this regime. Honey encryption (HE) and homophonic ciphers (HC) tailor encryption to the message distribution to provide meaningful security, but their analyses do not establish optimal guarantees for arbitrary distributions. We ask: for messages and keys drawn independently from arbitrary distributions 
, what is the optimal information-theoretic security achievable by any symmetric encryption scheme, and which conditions characterize its attainment? 

For key confidentiality (KC), the optimal guarantee is that the ciphertext 
 reveals only negligible information about the key 
, i.e., 
. For uniform schemes, this is achieved if and only if decrypting under every fixed key yields a distribution statistically close to 
. HC and HE achieve this guarantee. For message-recovery security (MRS), the optimal guarantee is that no adversary guesses the message with probability above 
, while a trivial adversary achieves at least 
 by guessing the most likely message or decrypting under the most likely key. We construct MRS-OE, a uniform scheme tailored to 
 that attains 
, and prove that 
-agnostic schemes, including HC and HE, cannot achieve optimal MRS in general. We also show that optimal KC and optimal MRS generally cannot be achieved simultaneously. Technically, we establish the KC characterization through flow and cut analysis on a weighted directed acyclic graph. We also introduce a continuous-ciphertext framework that separates structural constraints from discretization error.
2026/1974 (PDF)
Last updated:  2026-09-20
Filtered Supersingular Isogeny Counts and Exact-Coset Response Existence
Public-key cryptography
Ti-Hong Qin, Hong-Yu Tang, Zong-Bin Wang, and Wen-Lun Pan
Hide abstract
We study the existence of bounded-degree supersingular isogenies under prescribed degree filters and exact torsion constraints. In characteristic 
, a second-moment argument using the Brandt trace formula gives asymptotically positive coverage of independent uniform endpoints at 
, for every fixed 
 and every endpoint-independent binary filter retaining 
 cyclic kernels. This avoids the logarithmic loss of a maximum-Petersson-norm estimate, but does not imply coverage tending to one. For curves equipped with a line in their 
-torsion, where 
 is prime, a cumulative Hecke estimate gives nonexistence 
. Combined with a first-moment incidence bound, this separates two scalar-containing subgroups with the same upper-triangular span and additive Hom index 
:their response-existence probabilities tend to one and zero when 
, 
, and 
. The positive statement averages over a uniform source line and also yields an existential fixed-frame result; the exact challenge cardinalities differ. Determinant filters and finite odd-degree bounds at the SQIsign v3.0 parameters illustrate the results. All probabilities concern the stated arithmetic experiments, not the failure probability or security of a concrete signer.
2026/2121 (PDF)
Last updated:  2026-09-20
Lattice-Based Synchronous Signatures: Efficiently Aggregatable and Thresholdizable
Public-key cryptography
Dan Boneh, Binyi Chen, and Trisha Datta
Hide abstract
Aggregate signatures play an important role in proof-of-stake systems, where many validators sign a block. There is a strong desire to aggregate all these signatures into one short signature that is fast to verify.  Since all validators know the block number they are signing, this setting is well suited for synchronous (a.k.a stateful) signatures. Boneh and Kim (2019) showed that lattice-based one-time signatures (OTS) can be aggregated very efficiently. The Chipmunk and Lemur signature schemes extend this
to an 
-time synchronous scheme using a (homomorphic) Merkle tree of 
 one-time public keys. Each leaf of the tree is used to sign one message, and these one-time signatures can be aggregated across many signers. Due to the Merkle tree, every aggregate signature includes a Merkle authentication path of length 
.

We present a different lattice-based approach to constructing a synchronous aggregate signature scheme. The length of an aggregate signature in our scheme is independent of 
.  The resulting signatures are asymptotically shorter than existing schemes, and concretely shorter for some parameter choices. The resulting signature verification algorithm is algebraic, which makes it amendable to efficient threshold signing. Moreover, we are able to prove security in an adaptive corruption model. We thus obtain an efficient lattice-based synchronous 
threshold signature scheme, where signatures from many signers can be aggregated into a single short signature.  The scheme relies on a one time trusted setup to generate the public parameters.
2026/2120 (PDF)
Last updated:  2026-09-20
Multi-Key FHE with Compact Ciphertexts (or: MKFHE Strikes Back)
Applications
Intak Hwang, Seonhong Min, and Yongsoo Song
Hide abstract
Multi-Key Fully Homomorphic Encryption (MKFHE) is a variant of fully homomorphic encryption (FHE) that allows computations over data encrypted under different keys. Its support for non-interactive, dynamic participation and on-the-fly computation makes it an attractive primitive for multi-party applications. However, MKFHE has been largely overlooked in practice as its performance scales poorly, since the ciphertext size grows linearly with the number of participants in all existing constructions. Resolving this issue was a major open problem for almost a decade.

In this work, we close this gap by providing the first construction of an MKFHE scheme with compact ciphertexts: the size of ciphertexts remains constant throughout the entire homomorphic evaluation, even when the number of participating parties changes during computation. As a result, our construction retains all the advantages of MKFHE while simultaneously offering fast evaluation performance similar to that of a single-key scheme.

Our methodology is generic and applies to a broad class of RLWE-based FHE schemes. Concretely, we construct efficient multi-key variants of RGSW and FV-like schemes, and further develop a compiler that transforms a wide range of RLWE-based FHE schemes into their multi-key counterparts.

We implement our multi-key FV scheme in Go. Empirical results show that our scheme achieves 
 faster multiplication and 
 faster automorphism for 16 parties compared to the state-of-the art construction by Kwak et al. (CCS '23), demonstrating the concrete efficiency of our construction.
2026/2119 (PDF)
Last updated:  2026-09-20
CKKS Bootstrapping in less than 10ms
Public-key cryptography
Rending Ouyang, Jian Liu, Peimin Gao, Pan Xiao, Heng Zhang, Man Ho Au, Cong Zhang, Kui Ren, and Chun Chen
Hide abstract
Bootstrapping enables deep homomorphic computation in CKKS by replenishing multiplicative levels. It remains one of the scheme's most expensive operations. HEAAN reports a bootstrapping latency of 6.97ms, substantially faster than other publicly documented alternatives.  However, the algorithmic and implementation techniques underlying this result have not been disclosed, leaving it unclear how such performance is achieved.

In this work, we present a fully specified CKKS bootstrapping construction that achieves performance comparable to that reported by HEAAN. Building on SHIP (presented in Eurocrypt 2025), we introduce several algorithmic innovations that reduce communication and computational overhead while enabling efficient parallel execution.  Our implementation bootstraps a single complex-valued ciphertext in 8.2ms and achieves an amortized latency of 4.6ms per ciphertext for a batch of 64.  These results demonstrate practical sub-10ms CKKS bootstrapping.
2026/2118 (PDF)
Last updated:  2026-09-20
I Prove, Therefore I Am: Spatiotemporal Multi-Party Computation
Foundations
Ziqing Guo, Fuyuki Kitagawa, and Xiao Liang
Hide abstract
Secure multiparty computation (MPC) enables mutually distrustful parties to compute on private digital inputs. We initiate the study of 
 
, extending this paradigm to functionalities whose inputs additionally depend on physical facts such as the parties' locations, times, or trajectories. Such protocols must simultaneously hide spatiotemporal information and ensure its physical consistency: a malicious party should not be able to make the functionality operate on a spatiotemporal input inconsistent with its actual physical state.

The main conceptual challenge is to formulate extraction of spatiotemporal information within the simulation-based security framework. We introduce 
 
 
 
, following the principle "I prove, therefore I am:" rather than defining physical presence directly through a mathematical relation, we define it operationally through the ability to complete a sound spatiotemporal verification protocol. Accordingly, an extractor recovers a spatiotemporal point from a successful prover and certifies its physical validity by using the extracted prover to succeed in an auxiliary spatiotemporal verification protocol. Building on this notion, we define universally composable (UC) security for spatiotemporal MPC, capturing privacy, physical consistency, and composability.

We provide constructions achieving this new MPC notion. We first construct UC-secure commit-and-prove protocols for spatiotemporal knowledge: in the CRS model under LWE against quantum provers without pre-shared entanglement, and in the QROM against quantum provers with unbounded pre-shared entanglement. Using these protocols, we obtain general-purpose UC-secure spatiotemporal MPC from semi-honest post-quantum MPC. We also extend our framework to UC-secure spatiotemporal multiparty quantum computation over private quantum inputs and classical spatiotemporal information.
2026/2117 (PDF)
Last updated:  2026-09-20
Provable dual attack on LWE via lattice projection
Attacks and cryptanalysis
Hongyuan Qu, Chengliang Tian, Geng Wang, and Guangwu Xu
Hide abstract
The Learning with Errors (LWE) problem is a cornerstone of post-quantum cryptography, and the dual attack is a central tool for evaluating its concrete hardness. In provable dual attacks, a major bottleneck is the large number of discrete Gaussian samples required over the dual lattice. While recent works have focused on accelerating the guessing step via modulus switching, the sampling bottleneck has remained largely unoptimized. In this paper, we address this issue from a new perspective. First, we observe that Banaszczyk's inequality can be significantly improved when the first minimum of the lattice is suitably large. This yields a refined distinguishing inequality, which provides sharper conditions for attack success. Second, we propose a new provable dual attack framework based on lattice projection. Instead of sampling over the entire dual lattice, we sample over a carefully chosen sublattice, whose dual is the projection of the primal lattice onto a lower-dimensional subspace. This reduces the lattice dimension and thus substantially accelerates discrete Gaussian sampling. We rigorously analyze the parameter conditions for the success of our framework, and by enumerating all feasible parameters we achieve an improvement of 
--
 bits over the original provable dual attack framework.
2026/2116 (PDF)
Last updated:  2026-09-19
Smooth evaluations of cyclotomic quadratics
Foundations
Bruno Sterner
Hide abstract
In this note we find large primes 
 such that 
 is 
-smooth for the cyclotomic polynomials 
, with 
 and 
, and a sufficiently small smoothness bound 
. These primes hence provide the first instantiation of integer factoring challenges that, a priori would be difficult to factor but, are actually easy to factor with traditional algorithms not typically used in practice.
2026/2115 (PDF)
Last updated:  2026-09-19
Fast Quantum-Circuit Superoptimization
Implementation
Aws Albarghouthi
Hide abstract
Optimizing quantum circuits is critical: circuits must fit within the resource limits of a quantum computer, and every unnecessary operation increases their cost and probability of failure.We present a simple optimization algorithm for quantum circuits that (1) is very fast, (2) scales to millions of operations, and (3) matches or outperforms the optimization quality of the best existing optimizers and superoptimizers.

Our key insight is that we can compactly represent circuit equivalence classes as a map which associates a projective unitary with a smallest representative circuit implementing it. We give a sound algorithm for synthesizing such maps, which we call minimal unitary representative maps (MURMs). Armed with a MURM, our optimizer inspects every bounded subcircuit in a linear pass and replaces it with a minimal variant. Incremental computation and symbolic arithmetic make every replacement fast and sound. Our evaluation shows that our approach achieves significantly greater circuit reductions than leading optimizers, runs orders of magnitude faster, and scales to circuits with millions of operations.
2026/1088 (PDF)
Last updated:  2026-09-19
FlipFields-New Building Blocks for Cryptograpic Primitives?
Foundations
Christopher Wolf
Hide abstract
---while looking strange at first glance, this is certainly true in GF(2). In this paper we propose the two field-like structures \FlipInts\ and \FlipPolys\ as potential alternative building block for crypographic schemes---in particular in the post-quantum setting. Both structures have 
 elements for some positive integer 
 and are derived from the natural numbers 
 in the first case and the univariate polynomial ring GF(2)
 in the second case. We call the generalization of these two structures FlipFields. In addition, we see examples how they can be used for post-quantum cryptography, in particular Unbalanced Oil and Vinegar, Learning with Errors and Saber. There is also a discussion of cryptographic primitives that are most likely not suitable for FlipFields or at least not easy to tweak. As the structures are very new, this paper also includes a list of OpenProblems.
2026/2114 (PDF)
Last updated:  2026-09-19
How to instantiate Fiat-Shamir Provably and Practically?
Cryptographic protocols
Riddhi Ghosal, Aniket Kate, Pratyay Mukherjee, Pratik Sarkar, and Rohit Sinha
Hide abstract
We propose a new variant of the Fiat–Shamir transformation where the Fiat-Shamir hash is instantiated by an external verifiable random function (VRF) service. We prove that soundness holds as long as the VRF secret key remains hidden from the prover, i.e., assuming no collusion between the prover and the VRF evaluator. Crucially, due to the public verifiability of VRF outputs, the VRF computation itself need not be trusted beyond the key-generation phase. This yields the first NIZK compiler in a non-idealized model that is both practical and provably sound.   

Our transformation can be instantiated using extensively deployed blockchain-based VRF services. Moreover, it naturally supports distributed VRF services, allowing the no-collusion assumption to be relaxed: soundness holds even if the prover colludes with up to a threshold number of evaluators. In this setting, distributed key generation for the VRF further eliminates trust in the setup. 

A distinctive feature of our approach is that the verification step is local and does not require accessing the VRF. This is particularly advantageous for recursive proof systems such as incrementally verifiable computation, where the verification circuit must be embedded within the circuit for which a proof needs to be generated. This enables construction of the first provable and practical recursive proof -- bypassing well-known impossibilities in the random oracle model.

We demonstrate the real-world utility of our framework with an implementation for the GKR protocol.
2025/888 (PDF)
Last updated:  2026-09-19
Bootstrapping GBFV with CKKS
Public-key cryptography
Jaehyung Kim
Hide abstract
Generalized BFV (GBFV) replaces the integer plaintext modulus of BFV with a polynomial plaintext modulus, yielding a flexible tradeoff between precision and SIMD parallelism. This framework encompasses both BFV and CLPX, with CLPX corresponding to the extreme large-precision endpoint where very large plaintext moduli can be supported without increasing the underlying RLWE dimension. However, bootstrapping remains a bottleneck throughout the large-precision GBFV regime: existing GBFV bootstrapping methods rely on BFV-style linear transformations, whose noise growth becomes prohibitive as the plaintext precision grows. We introduce a GBFV bootstrapping method that treats the noise of a GBFV ciphertext as a CKKS plaintext and bootstraps it through CKKS. The main obstacle is that the BFV-from-CKKS bootstrapping technique of Kim et al. (CCS'24) relies on integer plaintext scaling and does not directly apply to GBFV, whose scaling factor involves a polynomial plaintext modulus. We overcome this by developing a noise-extraction and recovery procedure tailored to polynomial plaintext moduli. Our implementation bootstraps large-precision GBFV instances, including CLPX for the first time, supporting plaintext moduli of more than 
 bits in less than 
 seconds. We also introduce GBFV-to-CKKS and CKKS-to-GBFV conversions, interpreting the former as an approximate digit-decomposition toolkit for non-arithmetic operations over large GBFV plaintext spaces.
2026/2113 (PDF)
Last updated:  2026-09-19
Post-Quantum PKE and More from a Noisy Unstructured Linear Algebraic Assumption: Beyond LWE and LPN
Public-key cryptography
Riddhi Ghosal, Paul Lou, and Amit Sahai
Hide abstract
Noisy linear algebraic assumptions (NLAs), such as 
 and Alekhnovich’s 
, have long served as the most reliable sources of post-quantum hardness. However, a series of recent classical attacks on assumptions once believed to be quantum-secure, coupled with the rapid progress in quantum technology, underscores the urgent need for assumptions that are plausibly hard even if  both 
 and  
 turn out to be broken.

A fascinating recent work of Ghosal et al. (Eurocrypt 2025) takes an important first step in this direction, constructing a PKE scheme by combining two NLAs that remain plausibly secure even if 
 and Alekhnovich’s parameter regime for 
 were to be polynomially broken. A critical weakness of their work, however, is that it is still 
-broken: namely, 
an oracle that  breaks 
 with 
 noise rate  can break their NLA, for some 
.
We ask: can this barrier be overcome?

We answer this question positively, by introducing an NLA that is plausibly hard given an oracle that  breaks even constant noise rate 
, while still being plausibly hard given an oracle that breaks 
. Along the way, we  improve upon the work of Ghosal et al. significantly in several dimensions. 

Specifically, we introduce a single new natural NLA and provide constructions of (i)  public-key encryption and (ii) two-round maliciously secure oblivious transfer protocol from this single assumption. We also construct a one-time circular-secure secret-key encryption from a binary variant of this assumption.

Our assumption takes the form 
, where 
 is a random unstructured square matrix, the secret 
 has short and sparse entries, and the error 
 is a mixture of two components: a small-but-dense error combined with a large-but-sparse error. 

We provide a comprehensive initial cryptanalysis, 
ruling out (1) efficient combinatorial attacks that exploit sparsity such as information set decoding, (2) dimension reduction attacks, and (3) dual lattice attacks.
We also prove, via a reduction, that our assumption, and its binary variant, is at-least as hard as 
. Additionally, we
show that for a broad range of parameters including those used by our constructions, our assumption does not appear to fall within the class of “lattice” assumptions, that is, there seems to be no reduction to approximate shortest vector problem solvers, and, crucially, it is plausibly hard even given an oracle that breaks  for any noise density parameter.
2026/1329 (PDF)
Last updated:  2026-09-19
Flock: Fast Proving for Batch Boolean Computations
Cryptographic protocols
Benedikt Bünz, Ron Rothblum, and William Wang
Hide abstract
For many applications of SNARKs, a key bottleneck is proving large batches of standard cryptographic hash evaluations, such as SHA-256, Keccak, or BLAKE3. We introduce Flock, a hash-based SNARK for extremely fast proving of such batched Boolean computations. Flock proves batches of the same R1CS circuit (plus input/output relations between them), can prove hash-chains and Merkle path openings, and in principle can be extended to full-fledged hash-based signature verification. At its core, Flock combines new optimizations for the lincheck and zerocheck protocols with an aggressively optimized proof-of-concept implementation co-designed by coding agents.

On a single core of an M4 Max processor, Flock proves 82k evaluations of the BLAKE3 compression function, 42k SHA-256 compressions, and 30k Keccak permutations per second — less than a 
 overhead over native execution. On ten cores, throughput exceeds 660k BLAKE3 compressions per second; in proving SHA-256, Flock is more than 
 faster than Binius64, the prior state of the art, and more than 
 faster than the fastest elliptic curve-based SNARK we measured against.
2026/2112 (PDF)
Last updated:  2026-09-19
Mist: Transparent Polynomial Commitments from Class Group Subset-Product Accumulators
Cryptographic protocols
Sriram Sridhar
Hide abstract
We construct the first polynomial commitment in groups of unknown order that simultaneously requires no trusted setup, has quasilinear prover time, and has constant-size opening proofs. Our main technical contribution is a transparent class group realization of the subset-product accumulator relation introduced by Morphic accumulators (CRYPTO ’26). It supports accumulating a fixed family of class group elements and proving prescribed algebraic relations among the accumulated elements, yielding constant-size range arguments with quasilinear prover time.

    Combining these range proofs with the DewTwo framework (CRYPTO ’25) gives the first transparent polynomial commitment with quasilinear prover time and constant-size openings. Concretely, our construction also gives the shortest known opening proofs (1.3 KB) among transparent polynomial commitments. We prove security under falsifiable class group assumptions and also develop new extraction techniques over dyadic rationals, which may be of independent interest.
2026/2064 (PDF)
Last updated:  2026-09-19
Area-Time Efficient NTRU Prime Decapsulation: ASIC Evaluation of the First Five-Way Char-3 Multiplier
Implementation
Esra Yeniaras
Hide abstract
Streamlined NTRU Prime (sntrup761) is a lattice-based key encapsulation mechanism that, although not a NIST standard, remains widely deployed in critical internet infrastructure. It is the post-quantum key-exchange default in OpenSSH, standardized in RFC 9941, and used well beyond SSH, in Red Hat Enterprise Linux, the liboqs library, PQConnect, and commercial VPNs. Its decapsulation performs a polynomial multiplication over the characteristic-three ring 
, so faster methods for this operation directly improve these protocols. We use the Yeniaras-Cenk 5-way multiplier (U1-hybrid), which has the lowest arithmetic complexity among characteristic-three multipliers and gives a 35.52% scalar-C software speedup over Bernstein's three-way method (B1). Yet every prior NTRU Prime hardware design, on FPGA or the single existing ASIC, uses only schoolbook or a single layer of two-way Karatsuba. Three-way splits have been implemented only in software; no 5-way split had been implemented at all before this work. We present the first ASIC evaluation of the Yeniaras-Cenk 5-way multiplier (U1-hybrid), synthesized to the Nangate 45 nm library and compared against the state-of-the-art parallel schoolbook multiplier of Peng et al. On the area-delay product (ADP), the Yeniaras-Cenk 5-way multiplier is 6.8x better than the Peng schoolbook at the multiplier level, and 1.27x better in the sntrup761 decapsulation core, which it completes in 2471 cycles against Peng's 3829, a 35.5% core reduction (12.4% once the fixed hash is included). We also compare against a 3-way Karatsuba baseline (Bernstein's B1), noting that no optimized hardware B1 exists and we do not build one; the comparison there is at the operation-count level only. Synthesizing U1 and B1 as combinational circuits isolates where the five-way advantage comes from. It is not gate count: each 
 product expands into four 
 products, so at the same architecture the two use nearly the same area. The gain is in depth, as those four 
 products run in parallel: 138.27 ps against 57.95 ps for one 
 multiplier, 2.4x rather than 4x. The advantage is thus a hardware effect: it grows with design parallelism and matches the scalar-C software figure across two parameter sets. All Verilog, testbenches, and synthesis scripts are openly available.
2026/2111 (PDF)
Last updated:  2026-09-19
Computing C(N, R) mod m for Arbitrary Composite Moduli and N <= 10^18: A Prime-Power Decomposition Engine with Two Memory Regimes and Division-Free Arithmetic
Implementation
Vadik Malik, Rudr Pratap, and Sarthak Vashishtha
Hide abstract
Binomial coefficients modulo an integer, 
, are a primitive of combinatorial counting, yet the two textbook methods collapse at scale: the Pascal recurrence costs 
 time, and the factorial-table method costs 
 memory, requires a prime modulus, and requires 
. Composite moduli are harder still, because factorials are not invertible modulo prime powers and the exact power of 
 dividing the coefficient must be tracked. We present a complete, provably correct engine for arbitrary 
 and 
.

Methodology. The modulus is factored as 
 by trial division, deterministic Miller-Rabin, and Pollard-Brent rho. For each prime power we prove and implement the decomposition 
, where 
 is the 
-free part of the factorial and 
 is Kummer's carry count; the same count yields an 
 short-circuit that decides 
 before any multiplication. A periodicity lemma derived from Gauss's generalization of Wilson's theorem compresses the state of Pascal's triangle modulo 
 to 
 prefix products, giving two regimes with proven bounds: a table regime answering queries in 
 multiplications after 
 preprocessing, and a direct regime costing 
 multiplications for 
 with 
 memory. Residues are recombined by the Chinese Remainder Theorem. The hot loops are division-free: Montgomery arithmetic exploits an incremental-form identity so that every unit costs one multiply and one add, and moduli 
 reduce by a bit-mask. Four modular-inverse strategies are compared, including Fermat-Hensel lifting and an inverse-prefix table built with a single inversion.

Validation. 3,139,684 oracle checks against Pascal's triangle, Lucas' theorem, Gauss's theorem, and literature constants, plus 3,004 exact big-integer comparisons, passed with zero failures; all 101 benchmark checksums are bit-identical across a Windows GCC 16 12th-gen Intel laptop and a Linux GCC 11 Xeon server.

Results. At 
 a query costs 1.4 to 6.9 microseconds for composite moduli and scales linearly in 
 at 15 to 36 ns per base-
 digit. For prime moduli the table regime answers queries in 0.09 to 0.27 microseconds independently of 
, and building the table costs 0.36 to 0.85 of a single worst-case direct query, so it pays for itself immediately. Montgomery arithmetic is 2.0 to 3.1 times faster than 128-bit modulo and bit-masking up to 6.7 times; Fermat-Hensel inversion is 16 to 20 times faster than the extended Euclidean algorithm at 
 and 1.4 to 2.0 times faster for large odd prime powers. Against the Pascal recurrence the engine is already 7,100 to 12,800 times faster at 
; at 
 the recurrence would need 4.6 to 5.8 CPU-years and a 4 GB row, whereas the engine answers in about 2 microseconds from a 244-byte table. With uniformly random 
, the Kummer short-circuit resolves 95% to 99.998% of prime-power solves in 
 time, a rate predicted exactly by Fine's theorem. We also report the honest wall: for a prime 
 with 
 no table fits in memory and a query costs 3.4 to 4.5 seconds—the intrinsic 
 bound that only sub-linear factorial algorithms could remove.
2024/273 (PDF)
Last updated:  2026-09-19
Perfect/Statistical 2-Party Computation with Improved Circuit Privacy
Cryptographic protocols
Jonathan Trostle
Hide abstract
We present protocols where one entity, the server, evaluates a circuit with encrypted inputs from the second party, the client. We give
secret key somewhat additive homomorphic schemes where the client has perfect privacy (server is computationally unbounded). The key idea for client privacy is the permutation table which consists of rows of vectors modulo a prime integer m: The initial row is (1, d2, ..., dc) where di-1|di,
di > N(a + 1)di-1, for an integer N which is a power of 2 and integer a, 2 <= i <= c. Subsequent rows are integer multiples of the 1st row, modulo m. The permutation table has a subset of rows (vectors) whose components are relatively short (facilitating addition without overflowing m) and which map to every possible vector modulo N (giving perfect
privacy since every plaintext vector is possible given a ciphertext vector from the table.)

We give a 2-party computation (2PC) protocol that also incorporates server inputs where the client has perfect privacy, assuming the server
is semi-honest in the setup protocol. Client privacy is statistical if the server is malicious during the setup protocol. Server privacy only holds against a computationally bounded adversary since it depends on the
hardness of a variant of the HSSP (Hidden Subset Sum Problem) and the DDH (Decisional Diffie Hellman Assumption). We leverage the Castagnos Laguillaumie linear homomorphic public key encryption for setup. The 2PC protocol maintains circuit privacy, without circuit expansion, except for leaking the number of AND gates to the client. Scaling the
2PC protocol via separate encryption parameters for smaller subcircuits allows the ciphertext size to remain constant as circuit size grows.
2026/2110 (PDF)
Last updated:  2026-09-19
Forkcipher-Based Committing Security from Explicit Collision Security Assumptions
Secret-key cryptography
Elena Andreeva, Maria Eichlseder, Simon Gerhalter, Marcel Nageler, Oliver Christoph Popa, and Andreas Weninger
Hide abstract
Context-committing security binds an authenticated encryption (AE) ciphertext uniquely to its encryption context. Many conventional AE schemes lack this guarantee, while most generic committing transforms rely on idealized primitives.

We develop committing AE from explicit forkcipher collision-resistance properties: 
 and zero-message collision resistance (
). For an 
-to-
-bit forkcipher, we establish matching query complexities of order 
 and 
, respectively. MILP- and SAT-based differential and reduced-round collision analyses of 
 support the corresponding generic security levels for the full-round primitive. We also analyze collisions in ButterKnife and ZIP-AES.

We lift 
 and 
 through FCPRF and FixM to fixed-length collision-resistant PRFs. We introduce nonce-based PRF security to capture the nonce-respecting pseudorandomness needed by our constructions, and construct FHashN, proving its variable-input-length collision resistance, pseudorandomness, and nonce-based pseudorandomness.

We present AEaH-2K, a two-key variant of AEaH combining secure AE with a variable-input-length collision-resistant PRF. Including the AE key in the commitment input gives confidentiality, authenticity, and CMT-4 security in the standard model. We also construct 
, an AEAD mode using a single forkcipher, and prove nonce-respecting privacy, authenticity, and CMT-4 security. With FHashN based on FCPRF, 
 achieves 
-bit confidentiality and 
-bit authenticity and CMT-4 security; with FixM, all 3 reach 
 bits. All construction proofs reduce to explicit pseudorandomness and collision-resistance assumptions.
2026/1627 (PDF)
Last updated:  2026-09-19
Adaptively CCA-Secure Threshold Decryption from Lattices with Polynomial Modulus
Public-key cryptography
Yuxin Zhang, Yunxiao Zhou, Shuai Han, Shengli Liu, and Xinyi Huang
Hide abstract
Threshold Decryption (TD) enables a set of decryptors, each holding a secret key share, to collaboratively decrypt ciphertexts. In this paper, we resolve the open problem raised by Devevey et al. [PKC 2021] of constructing an adaptively secure TD with polynomial modulus from lattices. As identified by Hwang et.al. [ePrint 2026/1585], solving this open problem is a particularly challenging task. The most challenging part is how to limit the leakage of secret key shares from decryption queries while keeping the modulus a polynomial. 

In this paper, we resolve the above open problem with two TD schemes TD1, TD2 from lattices in the random oracle model.

 -- TD1 is a non-interactive TD scheme achieving adaptive CCA-security with polynomial modulus for a small number of decryptors and bounded decryption queries. It can be extended to achieve robustness and works well in the asynchronous settings. 

 -- TD2 is a non-interactive TD scheme achieving adaptive CCA-security with polynomial modulus for bounded decryption queries. Compared with TD1, it supports any polynomial number of decryptors and works well in the synchronous settings.

To our best knowledge, our TD1 and TD2 are the first non-interactive TD schemes achieving adaptive security and polynomial modulus from lattices. The core techniques behind our schemes include the polynomial noise flooding technique and the coupled refined Leftover Hash Lemma (LHL) working for adaptive noisy linear leakage, which may be of independent interest.
2026/2109 (PDF)
Last updated:  2026-09-19
Attacking CRT-RSA with Small Exponents via Unravelled Linearization
Attacks and cryptanalysis
Zhaopeng Ding, Zhaopeng Dai, Yanshuo Zhang, Ziyang Yan, and Ying Chen
Hide abstract
We develop a new Coppersmith lattice attack on balanced CRT-RSA with small CRT exponents 
 using unravelled linearization. The new attack matches the practical effectiveness of the Takayasu–Lu–Peng attack (JoC 2019) while using lattices of roughly half the dimension. In contrast to their attack, for which there is a noticeable gap between the theoretical predictions and experimental results, our theoretical estimates closely match the experimental results for all tested parameter sets. Our analysis further shows that the asymptotic bound remains 
, so the new attack does not achieve the asymptotic improvement anticipated by Takayasu, Lu, and Peng. Nevertheless, performing LLL reduction on lower-dimensional lattices substantially reduces the running time, allowing us to attack much larger 
 in practice.
Furthermore, through elegant variable substitutions, we extend our attack to two most significant bit (MSB) leakage models, obtaining the first attack exploiting MSB leakage of 
 and an improved attack exploiting MSB leakage of 
. In both models, the asymptotic bounds improve as the amount of leakage increases and recover the Takayasu–Lu–Peng bound in the absence of leakage.
2025/590 (PDF)
Last updated:  2026-09-19
: Secure Collaborative Graph Processing Made Scalable
Applications
Siddharth Kapoor, Nishat Koti, Varsha Bhat Kukkala, Arpita Patra, and Bhavish Raj Gopal
Hide abstract
Secure graph processing enables computing on graphs while hiding the graph topology as well as the associated node/edge data. This facilitates collaborative analysis among multiple data owners, who may only hold a private, partial view of the global graph. Several works address this problem using the technique of secure multiparty computation (MPC) in the presence of 2 or 3 parties. However, when moving to the multiparty setting, as required for collaborative analysis among multiple data owners, the existing solutions are no longer scalable. Specifically, the runtime of the state-of-the-art scales linearly with the number of parties. Additionally, it has an expensive initialisation phase, which requires secure sorting operations known to be expensive in MPC. 
Thus, we propose 
, a generic framework for secure graph processing that improves efficiency and scalability for the multiparty setting. Further, 
 is designed to have a lightweight initialisation, which eliminates the need for secure sorting. Unlike any of the prior works, achieving a round complexity in MPC that is independent of the number of parties is what makes 
 scalable. Finally, we implement and benchmark the performance of 
 for the application of PageRank computation and showcase its efficiency and scalability improvements over the state-of-the-art. Concretely, we witness improvements of up to 
 in runtime in comparison to the state-of-the-art. Further, we observe that 
 takes under a minute to perform 10 iterations of PageRank on a graph of size 
 that is distributed among 
 data owners, making it highly practical for secure graph processing in the multiparty setting.
2026/2108 (PDF)
Last updated:  2026-09-19
-KLPT
: Improved Norm Bounds for the Quaternion Isogeny Path Problem and an Application to Ring Signatures
Cryptographic protocols
Kohei Nakagawa and Hiroshi Onuki
Hide abstract
Computing smooth-degree isogenies between supersingular elliptic curves with known endomorphism rings is a fundamental task in isogeny-based cryptography. Via the Deuring correspondence, this task is equivalent to finding a connecting ideal of smooth norm between the corresponding quaternion maximal orders. KLPT-type algorithms address this problem by transforming a known connecting ideal into an equivalent ideal of smooth norm. Although connecting ideals of norm approximately 
 are expected to exist, the best known such algorithms for general input only achieve norm 
. In this paper, we propose 
-KLPT
, a new KLPT-type algorithm that lowers this bound to 
. Our construction builds on 
-KLPT, which attains a smaller norm bound but requires two input ideals of the same prime norm. To extend 
-KLPT to arbitrary inputs, we introduce a new subroutine IdealNormReduce, which reduces the common norm of an ideal pair. By iterating IdealNormReduce, we obtain two ideals with a common norm below 
. Then we apply 
-KLPT to the resulting ideal pair to obtain a desired connecting ideal of smooth norm.  As an application, we construct Delfar, a fully anonymous isogeny-based ring signature scheme. Compared to Erebor-full, Delfar achieves shorter signatures and lower signing and verification costs for large rings.
2026/2107 (PDF)
Last updated:  2026-09-19
Enhancing SAT Solving to Find (Near) Collisions in 6-Round SHA-3 Variants
Attacks and cryptanalysis
Yi Tu, Ling Song, Huaijin Wu, Jian Guo, Jian Weng, and Chaoping Xing
Hide abstract
The Keccak hash function, designed by Bertoni et al., was selected as the new generation of Secure Hash Algorithm (SHA-3) in 2012. For NIST-standardized SHA-3 instances (SHA3-224/256/384/512, SHAKE128, SHAKE256), practical collision attacks have reached five rounds; six-round SHAKE128 was known only as a theoretical attack with cost 
 six-round evaluations~\cite{tuyi2022sha3}. In this paper, we present the first  classical practical collision attack on six-round SHAKE128 with 
 and total complexity about 
. Building on the Dinur-framework Dinur et al.~\cite{dinur2012new} and the SAT tooling of Guo et al.~\cite{tuyi2022sha3}, we introduce a parameterized colliding-trail SAT model--- collision length in digest and weight bounds are chosen at setup together with leaner differential encodings. Specifically, we introduce an enhanced heuristic strategy to identify suitable differential trails and employ multiple techniques to optimize the connectors: for the connector phase, we minimize weighted propagation cost of second round and third round, and penalize costly 
 transitions; candidate 
-compatible equations are chosen by a bit-granular greedy routine that maximizes connector degrees of freedom. In addition to SHAKE128, we report six-round \textbf{near}-collisions on SHA3-224 and SHA3-256 (four and six differing digest bits, respectively), and practical collisions on four- and five-round SHAKE256 with the full 
-bit digest.
2026/2106 (PDF)
Last updated:  2026-09-19
Couplet: Multivariate Polynomial Commitments with Two-Group-Element Openings
Cryptographic protocols
Weijie Wang, Alin Tomescu, Rex Fernando, and Charalampos Papamanthou
Hide abstract
We present Couplet, a new pairing-based multivariate polynomial commitment scheme (PCS). Couplet is the first multivariate PCS whose opening proofs are solely two group elements. In addition, for the case of multilinear polynomials, Couplet is the first constant-proof PCS with a prover that requires only a sublinear number of group operations. We achieve those properties simultaneously at the expense of a quasilinear, polynomial-specific, one-time preprocessing phase, whose cost can be amortized over multiple subsequent openings. On BN254, our implementation opens multilinear polynomials with 
 variables about 
 faster than Mercury after preprocessing, breaking even in total prover time after roughly 100 openings. Our approach generalizes to batched evaluation proofs, yielding a constant-size proof for multiple evaluation points without using random oracles. Applications include verifying repeated inferences of a committed model as well as more efficient auditable authenticated dictionaries. Our main technique involves compressing Papamanthou--Shi--Tamassia (PST) evaluation proofs, giving a single commitment which encodes all 
 quotient polynomials which are normally given out separately in a PST proof. In doing this, we reduce the verification to a univariate sumcheck problem, which we solve with techniques from Groth16 using a single additional group element. We prove extractability in the algebraic group model under a parameterized discrete-logarithm assumption. We give hiding variants and prove perfect zero-knowledge of the opening protocols. All opening protocols are noninteractive and require no random oracle.
2026/1673 (PDF)
Last updated:  2026-09-19
Linearly Homomorphic Secret Sharing and Multi-Party Computation with Unanimously Verifiable Deletion
Cryptographic protocols
Yilei Chen, Liheng Ji, and Han Luo
Hide abstract
Certified deletion enables a party to prove that it has erased the sensitive information contained in a quantum state. Bartusek and Raizes (CRYPTO 2024) gave the first secret-sharing scheme with privately verifiable deletion. Subsequently, Katz and Sela (EUROCRYPT 2025) constructed secret-sharing schemes with publicly verifiable deletion under computational assumptions. Constructing such a publicly verifiable scheme without cryptographic assumptions remains open.

In this work, we introduce an intermediate notion called unanimously verifiable deletion.  While private verification relies on the dealer and public verification allows any third party to verify deletion, unanimous verification assigns the verification responsibility to the parties in the secret-sharing scheme themselves. We construct an information-theoretically secure linearly homomorphic threshold secret-sharing batch scheme with adaptive unanimously verifiable deletion. In particular, to obtain its linear-homomorphic property, we exploit a linearly homomorphic structure of BB84-type states that enables linear transformations on encoded values while preserving certified deletion. More generally, this structure can be used to upgrade certain primitives with certified deletion to their linearly homomorphic counterparts.

As another major contribution, we demonstrate an application of this batch scheme by constructing outsourced multi-party computation (MPC) protocols for arbitrary arithmetic circuits. Our outsourced MPC has semi-honest classical clients and malicious quantum servers, achieves adaptive unanimously verifiable deletion security in the trusted-preprocessing and security-with-abort setting against a rushing adversary, and supports public output reconstruction. The adversary may adaptively corrupt up to 
 clients. For the servers, during the online execution, at most 
 corrupted servers may remain undeleted and at least 
 honest servers must remain undeleted, for any 
. After successful finalization, all servers may be corrupted without revealing additional information about the remaining clients' inputs beyond the public output and the corrupted clients' inputs.
2026/2105 (PDF)
Last updated:  2026-09-19
Actively Secure PPML with Soft-Boundary Overflow Detection
Cryptographic protocols
Yuntian Chen, Tianpei Lu, Keyu Ji, Zhiyuan Ning, Zhanyong Tang, and Cong Wang
Hide abstract
MPC-based privacy-preserving machine learning (PPML) typically operates over finite rings rather than floating-point arithmetic, making arithmetic overflow an inherent risk. Existing overflow-detection techniques generally require the evaluation of nonlinear functions, resulting in substantial computation and communication overhead. We propose Soft Boundary Detection, which reduces boundary verification to probabilistic functions over partitioned intervals, with a cost comparable to that of linear computation. Building on this abstraction, we develop an efficient mechanism for detecting arithmetic overflow in PPML workloads. Moreover, we show that Soft Boundary Detection naturally extends to the malicious-adversary setting. In ring-based computation, it further avoids the need to choose a ring space substantially larger than the actual data range to ensure soundness against malicious behavior. Our framework provides two key benefits: (1) highly efficient arithmetic overflow detection, and (2) active security without requiring ring extension or enlarging the underlying ring space. Experimental results demonstrate that our approach substantially strengthens security while retaining high efficiency, providing a practical solution for robust and actively secure PPML.
2026/2104 (PDF)
Last updated:  2026-09-19
On the Complexity of Statistical and Multiplicative Zero-Knowledge
Foundations
Daniel Alabi and Marshall Ball
Hide abstract
Multiplicative zero-knowledge (
) replaces the negligible statistical distance required by statistical zero-knowledge (
) with symmetric 
 approximate max-divergence between real and simulated verifier views. Prior work established efficiency gains from this relaxation; we study its computational power.

With negligible completeness, soundness, and additive simulation errors,

where 
 is the input length. Taking the union over polynomial leakage
bounds gives

even with zero additive simulation error. For every fixed 
, a polynomially padded 
 language remains 
-complete and belongs to 
.

A linear-leakage protocol for 
 separates 
 from 
 unless the polynomial hierarchy collapses to its second level. Padding yields 
-complete separators at every fixed positive polynomial exponent. Under a subexponential weakening of the nonuniform nondeterministic Strong Exponential Time Hypothesis, every polylogarithmic level  with fixed  strictly contains . The logarithmic threshold is tight relative to oracles: every polynomial-time computable, polynomially bounded superlogarithmic leakage budget admits an oracle separating classical  from quantum statistical zero-knowledge. Finally, fraction statistical knowledge complexity exactly characterizes one-sided real-to-simulator multiplicative simulation, with the sharp knowledge parameter and negligible statistical error.
2026/1028 (PDF)
Last updated:  2026-09-19
Collusion-Resistant Asymmetric Anamorphic Encryption: Framework, Generic Construction, and Concrete Instantiations
Public-key cryptography
Zhikang Xie, Rupeng Yang, Man Ho Au, Zuoxia Yu, and Willy Susilo
Hide abstract
(Receiver) anamorphic encryption enables covert communication through innocent-looking ciphertexts, even against a dictator that compels users to surrender their private decryption keys. In this work, we study the asymmetric form, where the covert channel operates in a manner analogous to PKE. However, a generic construction achieving collusion-resistance (where the dictator can corrupt a sender to additionally obtain the sender double key) under only mild restrictions on the underlying PKE remains lacking.

To address the problem, we develop a general framework for constructing collusion-resistant asymmetric anamorphic encryptions from a new cryptographic abstraction called witness PRF for PKE. The framework works in the anamorphic extension setting, which enables covert communication from (already deployed) public keys. Importantly, our security model addresses the shortcomings of prior collusion models.

The first instantiation of our framework yields a generic asymmetric anamorphic extension for any PKE scheme with high min-entropy ciphertexts. In contrast to prior generic constructions of fully asymmetric anamorphic encryption, which rely on indistinguishability obfuscation and do not consider collusion, our construction achieves collusion-resistance under assumptions believed to be weaker. We also give practical instantiations for widely deployed PKE schemes, including ElGamal, Regev, Linear, and CSIDH-based encryptions, without relying on heavy cryptographic mechanisms. Notably, some of these extend the range of PKE schemes known to admit practical asymmetric anamorphism.
2026/2103 (PDF)
Last updated:  2026-09-18
LaBinius – Fast Lattice-Based Binary Polynomial Commitment Scheme
Cryptographic protocols
Michał Osadnik and Gregor Seiler
Hide abstract
Lattice-based proof systems are one of the leading routes to quantum-safe succinct proofs. Existing lattice-based schemes come with inherent limitations arising from the coupling between the commitment and constraint domains. First, commitment efficiency is restricted by the need for invertible challenge differences, which precludes the use of fully splitting moduli that enable the fastest lattice arithmetic. Second, the characteristic of the constraint field must equal the commitment modulus. This limits the efficiency of proving binary computations, including standard hash function evaluations.

We present 
, a lattice-based polynomial commitment scheme, in which the evaluation arithmetic is separated from that of the commitment. The commitment runs over a composite modulus, and all of its prime factors are NTT-friendly. The evaluation claim lives in a binary extension field, without bitness checks or overhead over a binary arithmetisation.

We implement the scheme with AVX-512 kernels and connect it to the 
 and 
 front ends in an almost black-box way, which lets us use their arithmetisations tailored to binary fields. As a result, we obtain a SNARK for standard hashes. Proving 
-
, 
-
 and 
 on one core, our prover takes at most 
 times as long as the hash-based prover supplied with each system, and we obtain a single-core throughput of over 
 
 compressions per second with verification in under 
 milliseconds.

With 
 as a recursive compressor, the prover takes less than twice as long as the hash-based provers at the largest sizes we consider, while the proof size is below 
 KiB. This is about five times smaller than the proofs of 
 and 
 at those sizes and constitutes the smallest quantum-safe proof of a standard hash evaluation to date.
2026/2102 (PDF)
Last updated:  2026-09-18
Scalable Small ECDLP Solver via Windowed-Batch Inversion for Additively Homomorphic Encryption
Applications
Murat Cenk, Muhammad ElSheikh, Irem Keskinkurt Paksoy, and M. Anwar Hasan
Hide abstract
Decryption in EC-ElGamal and Twisted ElGamal based Additively Homomorphic Encryption (AHE) requires solving a small exponential Elliptic Curve Discrete Logarithm Problem (ECDLP) that recovers a bounded plaintext scalar 
 from 
. This recovery step is the performance-critical path in various applications, such as electronic voting systems (e-Voting), private federated learning, and privacy-preserving blockchain confidential payment systems including Zether (EC-ElGamal), Solana Token-2022 Confidential Transfers (Twisted ElGamal), and the forthcoming XRPL XLS-96 standard (EC-ElGamal). The state-of-the-art FastECDLP (Tang et al., 2023) employs the baby-step giant-step algorithm (BSGS) to solve ECDLP in 
-bit range by utilizing a baby-step table 
 of 
 affine points and achieves fast decryption by precomputing a giant-step table 
 of all 
 affine points where 
. Even with the signed-range halving of Tang et al., the giant-step table for secp256k1 requires 5.91 GB for a 58-bit search space and exponentially increases to 378 GB for a 64-bit search space. Thus, as noted by Tang et al., a 64-bit range remains impractical on a typical workstation due to the memory requirements. We present three algorithmic contributions that together eliminate 
 and enable practical 64-bit decryption. (i) A Jacobian coordinate loop advances the giant-step point iteratively in Jacobian coordinates, removing the need to precompute the giant steps in affine coordinates. (ii) Windowed batch inversion batch-inverts a window of a small 
 accumulated 
-coordinates at once, reducing the per-thread working memory required for the giant-step computation from 
 (the size of 
 in FastECDLP) to 
 regardless of bit size. (iii) Co-Z arithmetic computes two giant steps simultaneously by maintaining a shared 
-coordinate, reducing the field multiplication cost per step. We validate our solver on two machines, a laptop-class Apple M1 Pro and a desktop-class Intel Core Ultra 9 285K, with consistent results across both. On the Intel machine, our complete solver outperforms a faithful re-implementation of FastECDLP at 54-bit and 58-bit (
, up to 
 faster) without storing the giant-step table 
. At 63-bit and 64-bit, Tang et al.'s giant-step table 
 requires 189 GB and 378 GB respectively, making their approach infeasible on typical desktop or laptop computers. So far, our solver is the only feasible approach, completing full 63-bit and 64-bit decryption in 53 sec and 105 sec, directly overcoming the memory limitation identified by Tang et al. We also discuss two additional arithmetic optimizations, an inversion-chaining scheme and a co-Z Jacobian addition, that reduce the per-step arithmetic cost.
2026/2101 (PDF)
Last updated:  2026-09-18
Interactive Secret-Key PIR
Foundations
Nir Bitansky, Geoffroy Couteau, and Noam Mazor
Hide abstract
Private information retrieval (PIR) inherently requires public-key cryptography. A recent line of work suggests that this barrier can be avoided in secret-key PIR, where the client first preprocesses an N-bit database and retains only a short secret key. This line of work has yielded communication O(N^\epsilon) for any constant \epsilon under the Learning Parity with Noise (LPN) assumption in a high-noise regime not known to imply public-key cryptography, and communication O(N^{1/2}) under one-way functions. Whether compression beyond N^{1/2} can be achieved without relying on structured assumptions such as LPN has remained open.

We show that interaction enables polylogarithmic communication in the random oracle model. We construct a secret-key PIR protocol with O(log N ) rounds and polylogarithmic total communication. Alternatively, for any constant \epsilon, we obtain a constant-round protocol with communication O(N^\epsilon). We also obtain protocols with similar communication in
the plain model under the weakest version of LPN, with maximal noise rate 1/2 − o(1).

Our main idea, inspired by the free-XOR technique for circuit garbling, is to make secret-key preprocessing homomorphic under XOR, while relying on security against related-key attacks.
2026/2100 (PDF)
Last updated:  2026-09-18
Garbling Groth16 with Native Group Operations
Cryptographic protocols
Nakul Khambhati, Aaron Feickert, Christian Lewe, and Mukesh Tiwari
Hide abstract
The Groth16 verification equation has a compact algebraic description, yet garbling its Boolean implementation can require tens of gigabytes. We show how to garble this computation using native elliptic-curve group operations.  Motivated by proof verification in trust-minimized Bitcoin bridges, we construct a projective partial garbling scheme for conditional disclosure on invalid Groth16 proofs. For a fixed verification key and public statement, evaluation reveals a garbler-held secret when the supplied proof is invalid and hides the secret when it is valid. The proof itself remains public.

The main technical challenge is handling a pairing whose two arguments are supplied by the evaluator. We introduce a rekeying gadget that resolves this using a single private scalar multiplication. Building on Argo MAC (Eagen and Lai, 2026), BABE (Garg et al., 2026), and Duty-Free Bits (Khambhati et al., 2026), we prove a composition theorem for partial garbling across input representations and apply it to transform group encodings into bitwise projective encodings. This requires extending the projectivization technique of Duty-Free Bits to quadratic extension fields. The resulting garbled program has size 
 bits, where 
 is the computational security parameter. We prove privacy under the decisional Diffie-Hellman assumption, in the random oracle model.

Our BN254 implementation produces a 2.4 MiB garbled program, including projectivization, compared with the 48 GB reported for Boolean garbling of Groth16. Thus, we improve over prior state of the art in Groth16 garbling by over four orders of magnitude.
2026/2099 (PDF)
Last updated:  2026-09-18
Succinct Arguments for QMA in the Quantum Random Oracle Model
Foundations
Alessandro Chiesa and Zihan Hu
Hide abstract
Succinct arguments are a fundamental cryptographic primitive for verifying computational claims with small communication. In the classical setting, succinct arguments for NP can be constructed from unstructured hardness alone (e.g., hash functions) by compiling probabilistically checkable proofs (PCPs) or interactive oracle proofs (IOPs) for NP via the commit-and-open paradigm. In contrast, known succinct arguments for QMA rely on ``structured'' cryptographic primitives, or on the quantum PCP conjecture.

We construct the first succinct argument for QMA in the quantum random oracle model (QROM) without relying on additional cryptographic assumptions or unproven conjectures. This yields succinct arguments for QMA from unstructured hardness alone, showing that ideal hash functions not only suffice for succinct arguments for NP but also for QMA.

Underlying our result is an efficiency-preserving transformation that compiles quantum interactive oracle proofs (QIOPs), a recently introduced interactive generalization of quantum PCPs, into quantum arguments for the same language, via a natural quantum commit-and-open paradigm. Our transformation applies to every QIOP with public-query soundness, a notion that we formalize to capture a natural requirement of the commit-and-open paradigm and is satisfied by a known QIOP for QMA. As a key ingredient in our transformation, we formalize and construct extractable vector commitments for quantum states with local openings in the QROM, which may be of independent interest.
2024/979 (PDF)
Last updated:  2026-09-18
Volatile and Persistent Memory for zkSNARKs via Algebraic Interactive Proofs
Cryptographic protocols
Alex Ozdemir, Evan Laufer, and Dan Boneh
Hide abstract
In verifiable outsourcing, an untrusted server runs an expensive computation and produces a succinct proof (called a SNARK) of the results. In many scenarios, the computation accesses a RAM that the server maintains a commitment to (persistent RAM) or that is initially zero (volatile RAM). But, SNARKs for such scenarios are limited by the high overheads associated with existing techniques for RAM checking. We develop new proofs about volatile, persistent, and sparse persistent RAM that reduce SNARK proving times. Our results include both asymptotic and concrete improvements---including a proving time reduction of up to 51.3
 for persistent RAM. Along the way, we apply two tools that may be of independent interest. First, we generalize an existing construction to convert any algebraic interactive proof (AIP) into a SNARK. An AIP is a public-coin, non-succinct, interactive proof with a verifier that is an arithmetic circuit. Second, we apply Bézout's identity for polynomials to construct new AIPs for uniqueness and disjointness. These are useful for showing the independence of accesses to different addresses.
2026/2098 (PDF)
Last updated:  2026-09-18
Faster Incremental Authenticated Dictionaries: Removing the Query Bottleneck
Cryptographic protocols
Michael A. Bender, John Kuszmaul, and William Kuszmaul
Hide abstract
Known lower bounds prohibit the construction of very-efficient authenticated dictionaries that support both insertions and deletions. It remains an open question, however, whether it may be possible to construct very efficient authenticated dictionaries that are \emph{insertion-only}. The current paper answers this question in the affirmative, providing the constructions that strictly dominate the classic Merkle tree in the incremental setting.

Our first result is an authenticated incremental dictionary, called the exponential Verkle forest, that supports retrieval queries with time and proof size 
 and that supports insertions with time and proof size 
. The data structure also offers an \emph{amortized} bound of 
 for insertions. 

Building on this, we also present a full-query version of the dictionary, supporting both retrieval and exclusion queries. This data structure continues to support retrieval queries in time and proof size 
, while supporting all other operations in time and proof size 
.

Our results take a purely data-structural perspective on authenticated data structures, seeking to achieve the best bounds possible with the cryptographic primitives (collision-resistant hashing and position-binding vector commitments) that already exist. We also give partial evidence, in the form of a lower bound that applies to a natural class of data structures, that our 
 query bound is optimal for any solution with linear construction time.
2026/2096 (PDF)
Last updated:  2026-09-18
Perpetual Encryption
Public-key cryptography
Yevgeniy Dodis and Daniel Jost
Hide abstract
Traditional public-key encryption (PKE) schemes have a static secret key. This means that the secret key owner can decrypt any old ciphertext in perpetuity. Schemes with changing secret keys, supporting so-called epochs, have been considered for a variety of reasons, such as post-compromise security or more simply supporting communication among a dynamically changing group. This complicates perpetuity, especially in settings where the epoch number itself is supposed to be confidential. 

We address this concern by introducing perpetual encryption (PE). The scheme leverages a trusted group manager to distribute states for each epoch such that: (1) without such a secret state, ciphertexts look pseudorandom and, hence, hide the epoch number; and (2) parties in a later epoch can still decrypt encryptions for old epochs in sublinear time (in the number of epochs). More stringently, we require that whenever a party in an earlier epoch decrypts to a message m then any party in a later epoch must arrive at the same message, even for adversarially crafted ciphertexts. 

We build an efficient, unbounded-epoch PE scheme based on anonymous PKE, hashing, and other standard symmetric primitives. We also extend our scheme to protect against malicious group managers, at the expense of limiting the maximum number of epochs. All our schemes have fixed-sized states and decryption times, independent of the number of epochs.
2026/1665 (PDF)
Last updated:  2026-09-18
Deterministic Almost-All Recovery for Square Four-Tensor Isomorphism
Foundations
Jingchuan Ma, Yanhua Liu, and Qiaoyun Huang
Hide abstract
We give deterministic exact recovery for almost all square four-tensors over every finite field. For every 
 and prime power 
, an explicit 
-invariant family contains a 
 fraction of all 
 coefficient arrays, with an explicit failure bound uniform in 
. Membership and isomorphism decision and recovery against every target are deterministic in 
 bit time, with four invertible factors over the original field. The source distribution is uniform without any conditioning on flattening ranks. The proof combines linked extension-field eigenmatrices, a submodule sieve under the joint law of a matrix and its inverse, and recovery from the kernels of singular original flattenings. Partial traces of matrix products provide a factorization-free algorithm in large characteristic. Complementary results give explicit positive source fractions in smaller dimensions, including a 
 family with deterministic recovery. The algorithms invert the coefficient-uniform square-tensor group action with the stated success probabilities. A deterministic exhaustive fallback gives total correctness and a polynomial running-time bound with high probability; no polynomial expected running time is claimed.
2026/2095 (PDF)
Last updated:  2026-09-18
Cryptanalytic Extraction of Neural Networks Without Known Architecture Assumption
Attacks and cryptanalysis
Yantian Shen, Yi Chen, Anyu Wang, Hongbo Yu, and Xiaoyun Wang
Hide abstract
Cryptanalytic model extraction aims to reconstruct a functionally equivalent model through black-box interactions with the victim model. Under the fundamental assumption that the network architecture is completely known, existing attacks achieve the goal by recovering the model parameters.  In this paper, we explore whether this assumption can be removed practically. Focusing on ReLU fully connected networks, which are widely studied in this field, we propose a guess-and-determine framework that jointly recovers the network architecture (including network depth and hidden-layer dimensions) and the model parameters. This framework is based on a simple yet effective high-level idea: after designing a parameter recovery attack under the known-architecture assumption, we can analyze the architecture-sensitive traces observed during parameter recovery to recover the network architecture. We identify two such traces in differential extraction attacks: (i) a zero suffix in the merged weight vectors produced by signature recovery, whose length reveals the hidden layer dimension; and (ii) an equality pattern in the preimage-based sign recovery, which occurs only under the true hidden layer dimension. These two signals give rise to two routes for network architecture recovery.  For the second-to-last layer, we further propose two methods, one for identifying it, and one for recovering its dimension. Practical end-to-end attacks are implemented on a wide range of ReLU neural networks, including both expansive and non-expansive networks. To the best of our knowledge, this is the first time the feasibility of achieving functionally equivalent extraction on deep neural networks, after removing the known-architecture assumption, has been demonstrated in practice.
2026/2094 (PDF)
Last updated:  2026-09-18
Tree Encodings IV: Depth-Unbounded Attribute-Based Encryption and Delay Encryption
Public-key cryptography
Damiano Abram, Giulio Malavolta, and Lawrence Roy
Hide abstract
Assuming the polynomial-time hardness of Decomposed LWE, a variant of the learning with errors (LWE) problem, we construct ciphertext-policy attribute-based encryption for depth-unbounded (but bounded-space) predicates. Previously, attribute-based encryption for depth-unbounded predicates was only known from an insecure version of evasive LWE, or by additionally assuming the cryptographic hardness of discrete logarithms, which makes such schemes quantum-insecure.

Adapting these techniques, we also obtain the first construction of lattice-based delay encryption [Burdges and De Feo, EUROCRYPT'21], additionally assuming the existence of any space-bounded sequential function. Finally, as a contribution of independent interest, we show how to instantiate the latter from a circular variant of decomposed LWE. This also gives the first delay encryption that does not require explicit sequentiality assumptions.
2026/2093 (PDF)
Last updated:  2026-09-18
Tree Encodings III: Time/Space Hardness from Lattice Problems
Foundations
Damiano Abram, Giulio Malavolta, and Lawrence Roy
Hide abstract
Assuming the polynomial-time hardness of a variant of the short integer solution (SIS) problem, we show the existence of sequential and memory-hard functions. Such an assumption postulates the hardness of a problem against polynomial-time algorithms, regardless of their memory and depth. Yet, we show that this assumption implies that 
, that 
, and that the 
 hierarchy is proper, i.e., that 


Our main technical tool is a new variant of homomorphic computation over lattice encodings, which does not impose a bound on the circuit depth and is not based on bootstrapping.
2026/1317 (PDF)
Last updated:  2026-09-18
ProtogaLattice: Lattice-based Algebraic Folding
Cryptographic protocols
David Balbás, Anca Nitulescu, and Maxime Plançon
Hide abstract
Folding schemes are gaining traction recently as they are suited to prove streaming computations with low memory footprint. In particular, there has been a growing interest in post-quantum folding schemes for more expressive relations, and with improved proof sizes. While the landscape is vast, every lattice-based construction, such as Latticefold+, (Super)Neo, and Cyclo, heavily rely on the sumcheck protocol. Sumcheck gives efficient proving times, but the verifier circuits become very large, partially because of the many random oracle invocations required. When folding sequential computations, this significantly inflates the recursive overhead, as the prover must run the verification at every iteration.

We present ProtogaLattice, a new lattice-based folding scheme for general high-degree polynomial relations that drastically reduces the size of the verifier's circuits. Our folding backbone deviates from the sumcheck approach and uses algebraic techniques. Our contribution is twofold: 
i) a folding scheme that combines multiple instances of polynomial relations into accumulators for a bounded number of iterations, and 
ii) a bootstrapping protocol to reduce the norm of the witnesses underlying these accumulators, which can be used in complement to our bounded-depth folding scheme to unlock unlimited depth.

A full iteration of ProtogaLattice for witnesses of length 
 gives a 4
 shorter proof size than the state-of-the-art, with only 
 KB. Additionally, ProtogaLattice requires less random oracle evaluations and achieves a smaller verifier circuit than its sumcheck counterparts.
Our techniques open new directions towards building efficient lattice-based proofs for expressive relations and that present small recursion overheads, challenging the established sumcheck-based approaches.
2026/1140 (PDF)
Last updated:  2026-09-18
Tree Encodings II: Correlation Intractability for all Batched Relations
Foundations
Damiano Abram, Giulio Malavolta, and Lawrence Roy
Hide abstract
The Fiat-Shamir transform is a central tool in cryptography and understanding its soundness is both a theoretically challenging and practically pressing question. Correlation intractable hash functions offer a method to instantiate Fiat-Shamir in the standard model. In short, a hash function is correlation-intractable for a relation 
 if it is computationally hard to find an input 
 such that 
.

In this work we present the first construction of a correlation-intractable hash function family, where the complexity of the hash does not depend on the complexity of the relation. Besides being better aligned with the way Fiat-Shamir is used in practice, our construction implies correlation intractability for all (possibly inefficient) batched searchable relations, i.e., searchable relations that can be decomposed as a direct product of other searchable relations. Using complexity leveraging, we then compile our construction into correlation intractability for all sufficiently sparse batched relations. All of our results follow from the hardness of the decomposed short-integer solution (DSIS) problem, the natural search analogue of decomposed LWE, which we show to be at least as hard. 

As a direct consequence from prior work, our result implies that {sufficiently many parallel repetitions} of any perfectly complete public-coin three-message proof {for a language outside 
 are not zero-knowledge}. Moreover, we obtain non-interactive zero-knowledge (NIZKs) arguments for 
 from the sub-exponential hardness of DSIS.
2026/741 (PDF)
Last updated:  2026-09-18
Tree Encodings I: How to Authenticate a Truth Table
Public-key cryptography
Damiano Abram, Giulio Malavolta, and Lawrence Roy
Hide abstract
We propose a new method to construct homomorphic authentication codes for truth tables of read-once matrix branching programs, extending the celebrated homomorphic lattice encodings [Boneh et al., Eurocrypt 2014]. Our approach relies on the hardness of the decomposed learning with errors problem (DLWE), a recently introduced modification of Regev's LWE assumption. 
    
We then use this new technical tool to make progress on several open problems in the literature. Specifically, we obtain:

1. A constrained pseudorandom function (PRF), where the evaluation of the PRF on the master key does not depend on the complexity of the constraint, except for its circuit depth.

2. A way to securely compress and re-expand LWE samples in the plain model.

3. An adaptively secure broadcast encryption scheme, with ciphertext and secret keys growing poly-logarithmically with the size of the encrypted set.

4. A pseudorandom obfuscation for all puncturable PRFs, additionally assuming the existence of sub-exponentially secure indistinguishability obfuscation (iO).

None of the above mentioned primitives was known to exist from lattice assumptions. As a bonus result, we also obtain a conceptually simple and direct heuristic construction of iO based on lattice techniques, which is not based on the functional encryption-to-iO paradigm. We provide evidence that this approach can be used to build provably secure obfuscation for simple functionalities such as sampling lattice preimages using a hidden trapdoor.
2026/168 (PDF)
Last updated:  2026-09-18
Beyond Feedforward Networks: Cryptanalytic Extraction of RNNs
Attacks and cryptanalysis
Longxiang Wei, Hao Lei, Xiaokang Qi, Xiaohan Sun, Lei Gao, Kai Hu, Wei Wang, and Meiqin Wang
Hide abstract
Recurrent neural networks (RNNs) play an important role in time series modeling, signal processing, and resource constrained applications. Their parameters encode valuable functionality and constitute proprietary intellectual property. Cryptanalytic extraction tests whether black box queries reveal functionally equivalent parameters. Such analysis is important for assessing the security of deployed RNNs. However, prior cryptanalytic attacks mainly target feedforward networks such as fully connected networks (FCNs) and convolutional neural networks (CNNs). RNNs reuse their recurrent parameters at every time step, which existing extraction pipelines do not address.

We present cryptanalytic extraction of ReLU RNNs, including single-layer and stacked (a.k.a. multi-layer) RNNs, in both the hard-label and raw-output settings. To the best of our knowledge, this is the first such extraction. Using controlled inputs, we construct equivalent feedforward models whose depths depend on the number of recurrent layers, independently of the sequence length. However, parameter recovery from the equivalent models may yield recurrent weights with incompatible neuron orderings and positive scalings for the previous and current hidden states. We develop recurrent parameter alignment to make these transformations consistent and enable parameter reuse across time. Stacked RNNs are harder, because one equivalent model contains coupled branches, which prevent direct use of FCN sign recovery and complicate signature recovery. We therefore further develop two techniques, recurrent parameter recovery and folded bias difference. The former uses the recovered input weights to guide clustering and recover the remaining recurrent parameters, including their signs. The latter handles neurons that are otherwise difficult to recover.

We demonstrate end-to-end extraction across five RNN architectures with up to three recurrent layers in both observation settings. An additional raw-output experiment extends the sequence length from 20 to 1024 using the same target parameters.
2026/2091 (PDF)
Last updated:  2026-09-18
Keep Track of Your Errors: Solving ILWE and Improving Attacks on ML-DSA
Attacks and cryptanalysis
Mohamed ElGhamrawy, Thomas Eisenbarth, Julius Hermelink, Anja Rabich, Florian Sieck, Silvan Streit, Jonas Thietke, and Zhiyuan Zhang
Hide abstract
With the widespread adoption of NIST's new signature standard ML-DSA imminent, understanding its vulnerability to side-channel attacks is increasingly important. Current approaches that are based on deriving (Concealed) Integer Learning with Errors (ILWE) samples from side information require hundreds of thousands of signatures even in noise-free conditions, limiting their practicality.

In this work, we show that the number of signatures required in these attacks has been drastically overestimated. Keeping track of the error probability distribution when deriving ILWE samples allows for a soft-analytic approach to solving ILWE. Concretely, we show that distribution hints (Eurocrypt 2025) may be derived from ILWE samples and the corresponding belief propagation-based solver can recover the secret key efficiently. We then conceptually compare previous solvers to our soft-analytic approach. Furthermore, we provide an information-theoretic analysis and answer an open question---on how to filter out relations---posed in a different line of attacks against ML-DSA (Crypto 2025); thereby, we can propose improvements in the application of these solvers.

We evaluate our approach across various ILWE and Concealed ILWE parameter sets. In addition, we analyze a recently discovered timing leakage in ML-DSA and show how to solve for the secret key with far fewer signatures. In a noise-free attack setting, our approach reduces the average number of signatures required by a factor of 62---from 139 million to 2.25 million---compared with linear regression, as originally proposed, with larger improvements under noise. Thus, we show that attacks that derive ILWE and Concealed ILWE instances require far fewer signatures than previously believed, and their impact has been underestimated.
2026/1164 (PDF)
Last updated:  2026-09-18
Algebraic Cryptanalytic Extraction on Hard-Label Neural Networks
Attacks and cryptanalysis
Zirui Chen, Shi Tang, Zhengchao Gao, Yongjia Su, Lingyue Qin, and Xiaoyang Dong
Hide abstract
Although the state-of-the-art model extraction attack on the hard-label Fully-connected Neural Network (FCN) by Carlini et al. at EUROCRYPT 2025 has polynomial-time complexity in theory, its dual-point clustering relies on singular value decomposition (SVD) with a time complexity of 
, resulting in huge runtime in practice. To address this computational bottleneck, this work transforms Carlini et al.'s geometric-view hard-label attack into an algebraic framework, and proposes two efficient clustering methods: Normal Rank Check (NRC) and Approximate Signature Vector (ASV). The NRC and ASV methods replace Carlini et al.'s heavy SVD-based rank checking with simple rank checking or inner-product operations, reducing the clustering complexity to 
 on average. Furthermore, this paper presents the first model extraction attack against hard-label max-pooling Convolutional Neural Networks (CNNs) by combining the ASV method with the kernel-centric clustering scheme instead of the neuron-centric clustering, which fully exploits the property of weight sharing in convolutions and fills a cryptanalysis gap. Experiments on FCNs and the max-pooling LeNet-5 demonstrate that our NRC/ASV methods drastically cut clustering time, and improve the overall efficiency in the model extraction.
2026/2089 (PDF)
Last updated:  2026-09-18
Is it safe to lie? Revisiting Incoercible Multi-Party Computation in the Universal Composability Framework
Foundations
Saskia Bayreuther, Robin Berger, Eva Hetzel, and Jörn Müller-Quade
Hide abstract
In a multi-party protocol, incoercibility aims to protect protocol participants by allowing them to use their honest inputs even in the presence of a coercer. A coercer is an adversary who pressures protocol parties into deviating from the protocol. One goal of an incoercible protocol is that the coercer cannot distinguish if the coerced party obeys their command or is deceiving: The coerced party should be able to plausibly deny their deception to ensure their safety. In the past, several attempts at modeling incoercibility in the Universal Composability framework have been made. In Alwen, Ostrovsky, Zhou, and Zikas’ model (CRYPTO 2015), the deception is modeled as a mapping of protocol messages that are sent and received by the coerced party beyond the reach of the coercer. Similar to proofs in plain UC, in order to prove the incoercibility of a protocol, one shows the indistinguishability of ideal and real worlds. Alwen et al.’s incoercibility definition makes use of four worlds: In both the ideal and the real worlds either coercion or deception takes place. If the environment can neither distinguish between the two coercion worlds nor between the two deception worlds for all ideal deception strategies, the protocol incoercibly UC-realizes the ideal functionality. While this shows that it is always possible for a coerced party to deviate from the coercer’s instructions and deceive them, it does not cover if this deception can be detected by the coercer.
In this paper, we tackle the question how the perceived difference between deception and coercion can be modeled in UC. In particular, we make the following contributions. First, we refine the incoercibility notion by Alwen et al. by limiting the deception strategies to ones that allow the coerced party to plausibly deny the deception. We propose a multi-party computation protocol, derived from the one presented by Alwen et al., that fulfills this notion. The protocol uses hardware tokens to make UC-protocols, now including reactive functionalities, incoercible, where coerced parties can deceive the coercer about inputs as well as outputs. Secondly, we define a notion for plausible deniability, which we call Γ-deniability, and show that when the evaluated function is a differentially private mechanism, the deceiving party can plausibly deny their actions.
2026/1027 (PDF)
Last updated:  2026-09-18
Computing Asymptotic Bounds for the Automated Coppersmith Method via Linear Programming
Attacks and cryptanalysis
Zhaopeng Ding, Zhaopeng Dai, Baofeng Wu, Yanshuo Zhang, and Kejun Zhang
Hide abstract
Coppersmith's method is a foundational technique for finding small roots of modular equations, and determining asymptotic bounds for the recoverable roots is a central and challenging part of its analysis. In this paper, we transform the computation of asymptotic bounds for the Automated Coppersmith method, proposed by Meers and Nowakowski (ASIACRYPT 2023), into a linear programming problem, thereby obtaining a provably correct and explicitly computable formula. As applications of our method, we obtain improved asymptotic bounds for five cryptanalytic settings: the Commutative Isogeny Hidden Number Problem, the Modular Inversion Hidden Number Problem, the Elliptic Curve Hidden Number Problem, the Linear Congruential Generators with  unknown multiplier, and the Leveled Isogeny Problem with Hints for POKÉ. We believe that our method could be useful for evaluating the security of a broader range of cryptographic settings.
2026/1064 (PDF)
Last updated:  2026-09-18
Post-Quantum Security of Practical Correlation-Robust Hashing
Cryptographic protocols
Akinori Hosoyamada, Haruhisa Kosuge, and Keita Xagawa
Hide abstract
Correlation-robust (CR) hashing and its variants are central components in efficient secure-computation protocols, including OT extension, garbled-circuit optimizations such as Free-XOR and half-gates, and GGM-style tree constructions.
In practice, these hashes are typically instantiated from block ciphers such as AES.
Prominent constructions include the Matyas-Meyer-Oseas (MMO) construction and its variants such as 
.
Existing analyses, however, are classical and do not cover quantum adversaries with superposition access to the underlying random permutation or ideal cipher.

We establish post-quantum security for these practical block-cipher-based CR hashes.
In the quantum ideal cipher model (QICM), we prove multi-user tweakable correlation robustness with leakage (mTCRL) for the MMO construction, and multi-user tweakable circular correlation robustness with leakage (mTCCRL) for 
 and the Encryption-with-Feed-Forward (
) construction.
These results also imply the corresponding leakage-free and single-user guarantees: CR and TCR for MMO, and CR, CCR, TCR, and TCCR for 
 and 
.
Security in the quantum random permutation model (QRPM) follows as a special case.

We further verify that, in the QICM/QRPM, the above MMO-type constructions securely instantiate CR-type hashes used in various existing protocol analyses.
This covers several representative analyses involving OT extension, half-gates garbling, (correlated) GGM trees, and certain distributed point/comparison function constructions.
When the remaining components are post-quantum secure or modeled as ideal functionalities, the resulting protocol instantiations are post-quantum secure in the corresponding model.

Technically, our proof relies on the reprogramming-and-resampling technique of Alagic et al.~(Eurocrypt 2022).
To handle adaptive key leakage, as needed for some malicious OT-extension analyses, we introduce the conditional min-entropy with leakage (cmel) advantage, which separates leakage-induced entropy loss from the quantum ideal-cipher analysis.
Without leakage, our bounds guarantee security up to roughly 
 queries, where 
 is the min-entropy of the secret shift.
2026/2088 (PDF)
Last updated:  2026-09-18
Detecting split surfaces, RM surfaces, and minimum walks on isogeny graphs
Public-key cryptography
Eda Kırımlı and Gaurish Korpal
Hide abstract
We study the detectability of split surfaces in isogeny graphs of principally polarized superspecial abelian surfaces, a question relevant to the security analysis of dimension-
 isogeny-based cryptography. Our approach uses refined Humbert invariants to replace explicit isogeny computations with primitive representation problems for positive definite quadratic forms in five variables. We develop algorithms to detect 
-splittings and to compute the minimum 
-splitting level of a superspecial Jacobian without constructing the corresponding isogeny path. The same framework detects embeddings of real multiplication (RM) orders through primitive representations of discriminants of real quadratic orders. For RM, we use exhaustive small-prime data together with 
 random polarizations per prime for 
, testing primitive representations of square-free discriminants 
; the first nontrivial primitively represented discriminant is typically small. 
We apply these methods experimentally in two regimes. For 
, where the irreducible principal polarizations are known exhaustively, the automorphism data recovered from the refined Humbert invariant reproduces the counts of Ibukiyama--Katsura--Oort for the irreducible polarizations. For large parameters, we reach primes of 
 bits, where the splitting degrees produced satisfy 
 with a distribution whose shape is stable across the whole range, so the detected 
-isogeny has degree about 
; the smallest observed largest prime-power divisor of 
 remains small up to  bits and increases sharply from  bits onward.
2026/1254 (PDF)
Last updated:  2026-09-18
Top Gun: Degree Annihilation Attacks on Poseidon
Attacks and cryptanalysis
Antonio Sanso and Giuseppe Vitto
Hide abstract
Poseidon is one of the most widely deployed arithmetization-oriented cryptographic permutations and plays a central role in modern zero-knowledge proof systems. Although several algebraic attacks on reduced-round variants have been proposed, the security of the recommended parameter sets remains intact. A central difficulty in such attacks is controlling the degree growth of the polynomial representations induced by the permutation.

In this work, we develop degree annihilation, extending coefficient cancellation in round-skipping attacks to cubic Poseidon. Degree annihilation reduces the contribution of existing rounds by imposing algebraic constraints that force dominant degree terms to vanish. This yields polynomial systems of substantially lower effective degree.

We first present a simple bivariate form of degree annihilation and show how it combines naturally with classical round-skipping techniques. For cubic S-boxes with six full rounds, making the first partial-round input affine reduces the final degree bound by a factor of three. We then generalize the technique to multivariate settings, where systems of control equations are used to annihilate successive partial-round degree contributions. We find control solutions using elimination, resultants, and Gröbner basis techniques.

As a proof of concept, we apply the framework to reduced-round Poseidon instances and obtain new CICO-2 attacks. More broadly, our results suggest that constructing algebraic varieties that actively control degree growth may provide a direction for the cryptanalysis of arithmetization-oriented primitives.
2026/2087 (PDF)
Last updated:  2026-09-18
Batched and Weighted Threshold Encryption with Silent Setup
Public-key cryptography
Amit Agarwal, Rutchathon Chairattana-Apirom, Sourav Das, and Babak Poorebrahim Gilkalaye
Hide abstract
Batched Threshold Encryption (BTE) enables a committee of parties to jointly decrypt any subset of ciphertexts from a large set, while all other ciphertexts remain private. BTE has applications in blockchains, particularly in designing encrypted mempools, where transactions are encrypted until included into a block to prevent maximal extractable value (MEV) attacks. 
Existing BTE constructions, however, encounter one of the following usability downsides: (a) reliance on expensive distributed key generation (DKG) protocols, (b) dependency on a batch label during ciphertext generation, (c) ciphertexts that are only decryptable if they are encoded with respect to a non-conflicting index in the batch, or (d) no support for weighted threshold policies.

In this work, we present a BTE scheme that simultaneously addresses all of the above issues. It has a silent setup (key generation does not require interaction between parties), is batch and index independent, and supports weighted threshold policies. Our construction is based on prime-order groups with bilinear pairings with security proof in the Generic Group Model. Our ciphertext size is also the shortest among existing silent batched threshold encryption schemes with only 
 group elements in addition to the message length.
2026/1579 (PDF)
Last updated:  2026-09-18
Slipway: Accessing Finite Subspace Trails in Poseidon
Attacks and cryptanalysis
Giuseppe Vitto
Hide abstract
Poseidon is an algebraic permutation designed for efficient use in proof systems. Its nonlinear layer consists of power-map S-boxes. In a full round, the S-box is applied to every state coordinate; in a partial round, it is applied to only one coordinate, reducing the arithmetization cost. Each round also applies an MDS linear layer to diffuse information across the state.

To study algebraic degree, we let the input depend on variables and follow the resulting family of states through the permutation. If the coordinate entering a partial-round S-box is constant across that family, the S-box adds no degree in the family variables. Directions with this property over several consecutive partial rounds form finite subspace trails. Such trails exist for every linear layer, but their existence does not by itself explain how a constrained family can pass through the preceding full rounds and enter them without first acquiring high degree.

We address this reachability problem by constructing a constrained input family and a round-constant-dependent MDS matrix together. The prescribed matrix images carry the family through the four initial full rounds and into a chosen finite trail. After an explicit change of variable, the state at the end of the full-round prefix is linear in the new root variable, so the prefix acts as a controlled reparametrization rather than as a source of degree growth. We call this effect full-round absorption.

For the KoalaBear instance 
, we construct a two-parameter family whose first two input coordinates are zero. On this family, the four initial full rounds act as a reparametrization and deliver the variable directions into a two-dimensional trail, so those four rounds and the next fourteen partial S-boxes add no degree. For the exhibited control, the polynomials representing the first two output coordinates have exact degree 
, rather than the expected degree 
. We exhibit a common base-field root, yielding a complete CICO-2 solution for the full-round Poseidon instance.

The resulting matrices are MDS and satisfy the relevant matrix checks prescribed by the Poseidon designers, yet they make a finite trail reachable through the full-round prefix. We generalize the construction to CICO-
, derive bounds on trail dimension and matrix images, and provide a concrete MDS matrix that meets the CICO-3 matrix-image bound with equality.
2026/2086 (PDF)
Last updated:  2026-09-18
Dynamic Participant Updates to Access and Adversary Structures for Ideal Secret Sharing Schemes Based on Linear Codes
Foundations
Xu Qiuxia, Tang Chunming, and Yi Zongxiang
Hide abstract
The paper investigates secret sharing schemes based on linear codes, focusing primarily on the access structures and adversary structures arising from dynamic participant updates, as well as on whether such access structures can be realized by ideal linear secret sharing schemes. First, we characterize  the maximal adversary structure corresponding to the access structure obtained by adding new participants to every authorized subsets of a given access structure.Second, we prove that the original access structure can be realized by a linear code 
 if and only if the access structure obtained after adding 
 new participants can be realized by a linear code 
. In addition, we  establish the correspondence between the codewords of these two linear codes. Furthermore, We prove that the original access structure can be realized by an ideal secret sharing scheme if and only if the expanded access structure can be realized by one as well. Finally, we demonstrate that every linear secret sharing scheme realizing a disjoint access structure is ideal. We also prove that access structures whose any two minimal authorized subsets share all but one participant can be realized by an ideal linear secret sharing scheme. In addition, we determine the adversary structure corresponding to this class of access structures. More generally, we obtain the maximal adversary structure when  new participants are added to merely part of the minimal authorized subsets.
2026/1836 (PDF)
Last updated:  2026-09-18
FHE for ALU over Large Prime Moduli and Application to One-Round Threshold ECDSA
Public-key cryptography
Yuchen Wei, Kaisheng Ma, Mingyu Gao, and Hongren Zheng
Hide abstract
Fully Homomorphic Encryption (FHE) has served as a theoretical building block for cryptographic primitives, but concrete instantiations remain limited by the cost of generic homomorphic computation. One example is the universal thresholdizer that compiles any (deterministic) signature scheme into a multi-party threshold signature scheme with a one-round signing protocol (Boneh et al., Crypto'18), where the signing algorithm is homomorphically evaluated without communication with other parties. Practically, however, the signing algorithm itself often involves different types of computation. For example, ECDSA requires a hash function, point arithmetic over the high-precision defining field 
, and base-point group arithmetic over 
. Generic schemes like DM/CGGI take days to complete the homomorphic evaluation. Recent schemes like Boneh--Kim (Crypto'25), Cha--Park--Lee (Eurocrypt'26) and Gao--Zheng (Crypto'26) either support only one computation type, or lack efficient conversion methods.

We propose a new FHE scheme whose arithmetic logic unit supports both large prime modulus arithmetic and bit-level operations. We then specialize it to the 
 mode, the 
 mode and the Boolean mode for ECDSA. Our key component is a homomorphic canonicalization operation that restores the unique bit representation of the encrypted messages. It requires a constant number of CKKS bootstrapping operations regardless of the bit-width of the prime moduli, and it therefore enables efficient high-precision conversions like 
-to-
 modular reduction and 
-to-Boolean conversion. The conversion to Boolean mode is essential to allow noise flooding in the threshold decryption process.

For the workload of 
 modular multiplication followed by conversion to bit representation in GPU, we estimate a 14.5
 and 
19000
 better amortized latency compared with Montgomery-based approach in Cha--Park--Lee and TFHE-rs based approach, respectively. Our end-to-end homomorphic ECDSA signing experiment takes 56s on an NVIDIA RTX Pro 6000 GPU. To the best of our knowledge, it is the first concrete FHE-based construction towards threshold ECDSA with one-round signing.
2026/1919 (PDF)
Last updated:  2026-09-18
Second Round Key Recovery and a Floating-Point Attack on DNN-based AES Implementations
Attacks and cryptanalysis
Sisung Kim, Minjae Lee, and Dongjae Lee
Hide abstract
At EUROCRYPT 2026, Gérault et al. introduced natural DNN implementations of block ciphers: ReLU networks that agree exactly with the underlying cipher on binary inputs but extend it continuously to real-valued inputs. They showed that this extension exposes the first round key to recovery, and proposed a generic Secure Blackbox Transformation that turns an arbitrary DNN-based implementation into a provably secure implementation. We revisit both sides of their result. On the attack side, the known attacks on the unprotected implementation reach only the first round key. Under the same threat model, we refine the absorption property of the natural S-box and combine it with a chosen-plaintext collision test, validated empirically, to recover the second round key of AES-256 in about 
 queries. On the defense side, their transformation is provably secure over the reals, but that guarantee does not carry over to finite precision, where any neural-network implementation must ultimately run. We give a floating-point attack that recovers the first round key of the protected AES-128 implementation in 128 chosen queries, driven by an input value that the defense fails to round to a bit. Such a value exists in bfloat16, float16, float32, and float64, so moving to higher precision does not remove the weakness. We confirm the recovery in all four formats. Finally, we block the attack with a clamp placed before the defense's rounding step, which keeps every finite input in 
 in the working precision and changes neither the cipher nor the exact-real security guarantee.
2026/2085 (PDF)
Last updated:  2026-09-18
Quantum implementation of Keccak-f(25) for emulator and quantum hardware, and application to quantum password cracking
Implementation
Mathilde Chenu and Mario Chizzini
Hide abstract
In this work, we present a quantum implementation of Keccak-
, a toy version of SHA-3 introduced in the specification of Keccak, with the objective of using as few qubits as possible so that the resulting implementation can be run both on  quantum emulators and on quantum hardware. 
  The code, written in Qiskit, uses 
 qubits representing the internal state. All computations are performed in-place, meaning that no ancillary qubit is required. 
  We use this implementation to compare the number of logical and  instruction-set-architecture quantum gates required to run Keccak-
, both on emulators and on quantum hardware, as well as for Grover's algorithm on the hash function. 
  Finally, we use our implementation to highlight the specificity of quantum password cracking using Grover's algorithm with Keccak-
.
2026/364 (PDF)
Last updated:  2026-09-18
SPRINT: New Isogeny Proofs of Knowledge and Isogeny-Based Signatures
Cryptographic protocols
Thomas den Hollander, Shai Levin, Marzio Mula, Robi Pedersen, Daniel Slamanig, and Sebastian A. Spindler
Hide abstract
Zero-knowledge proofs of knowledge are a fundamental building block in many isogeny-based cryptographic protocols, such as signature schemes based on identification-to-signature transformations, or multi-party ceremonies that avoid a trusted setup, in particular for generating supersingular elliptic curves with unknown endomorphism rings.

In this paper, we construct SPRINT, an efficient polynomial IOP-based proof system that encodes the radical 
-isogeny formulas into a system of multivariate polynomials. When combined with the recent polynomial commitment scheme (PCS) DeepFold, our construction yields substantial improvements over state-of-the-art isogeny proofs of knowledge. For the SQIsign NIST-I prime 
, our implementation takes only a few milliseconds for proving and verification, with proof sizes around 80 kB. Compared to the previous state-of-the-art proof system by den Hollander, Mula, Slamanig & Spindler (PQCrypto'26), we achieve speedups ranging from 
 to 
 for the prover and from 
 to 
 for the verifier, while achieving proof sizes that are 
 to 
 smaller, across different instantiations and parameter sets.

Moreover, we study the weak simulation extractability of our proof system, which we can use as a starting point for a modular construction of signatures. We show that any Fiat–Shamir compiled interactive proof with a so-called canonical simulator is weakly simulation-extractable. We expect this general result to be applicable to other proof systems and thus of independent interest.

Building on SPRINT and our wSE result, we introduce a new family of signature schemes whose security solely relies on the 
-isogeny path problem, a foundational problem in isogeny-based cryptography.

As a concrete instantiation, we construct a signature scheme using DeepFold as the PCS. Across the different parameter sets, a prototype implementation of our scheme sees significantly faster signing times than the highly optimized NIST specification for SQIsign, at the cost of much larger signature sizes and excluding a one-time precomputation phase. When this phase is instead included with the signing time, we still get almost on-par timings. Meanwhile our signature scheme relies on weaker assumptions and is constant-time when instantiated with a suitable PCS. Even though our signatures are large, the framework offers flexibility for tradeoffs and optimizations – both within a given PCS and by switching to alternative PCS constructions. In particular, it will naturally inherit efficiency gains from future advances in plausibly post-quantum secure PCS constructions.
2026/2084 (PDF)
Last updated:  2026-09-18
Faster SVP in Polynomial Space
Foundations
Yansong Feng, Yiming Gao, and Jiaqi Liu
Hide abstract
Kannan's algorithm, as analyzed by Hanrot and Stehl\'e in 2007, solves the exact Euclidean shortest vector problem in polynomial space and 
 
 time. In the classical setting with polynomial space, we obtain the first improvement on this bound via a randomized algorithm that runs in 
 
 time.

The main idea is to represent a fixed shortest vector in many ways as a difference of samples, thereby enabling the low-space collision search of Lyu and Zhu (SODA 2023) to replace exhaustive enumeration in the original analysis.
2026/1793 (PDF)
Last updated:  2026-09-18
CVP Is NP-Complete for Principal Cyclotomic Ideals
Foundations
Jiaqi Liu, Yansong Feng, and Yanbin Pan
Hide abstract
We prove that exact Euclidean decision-CVP is 
-complete on the coefficient lattices of nonzero principal ideals in the power-of-two cyclotomic rings 
.  Our deterministic reduction from Exact Cover by 3-Sets (X3C) produces a target and a squared threshold 
 such that the closest squared distance is exactly 
 in YES instances and at least 
 in NO instances.  This also implies 
-hardness of exact search-CVP under polynomial-time Turing reductions.

We also transfer the resulting principal-ideal CVP instances to full-rank principal ideals of the cyclic quotient ring 
, where 
.  Their coefficient lattices are invariant under cyclic coordinate shifts.  The lift preserves principality and multiplies corresponding squared distances by eight.  Thus, on principal cyclic ideal lattices, exact decision-CVP is 
-complete and exact search-CVP is 
-hard.

We also obtain uniformly computable fixed cyclotomic and cyclic families in which only the target and threshold depend on the X3C collection.  Consequently, a polynomial-time solution to exact decision-CVPP on either family would imply 
 and collapse the polynomial hierarchy to 
.  To our knowledge, the cyclic results answer Micciancio's questions of whether exact decision-CVP is 
-hard on cyclic lattices and on a fixed family of cyclic lattices, even when restricted to full-rank principal cyclic ideals.

Finally, under the coefficient embedding, we prove that exact decision-module-SIVP is 
-complete on free rank-two modules over the same cyclotomic rings.
2026/2083 (PDF)
Last updated:  2026-09-18
Two-Bit Lifting for Ternary SIS: Polynomial-Time Collision Attacks on LtHash
Attacks and cryptanalysis
Rui Ding, Xiaorui Gong, Hao Jiang, and Lili Tang
Hide abstract
LtHash is a lattice-based incremental hash function introduced by Bellare and Micciancio at EUROCRYPT 1997. It can be used in distributed systems to efficiently maintain digests of changing data. Its set-collision resistance rests on the average-case hardness of the ternary short integer solution (SIS) problem in the random oracle model. LtHash implementations typically use power-of-two groups 
 for efficient machine-word arithmetic and SIMD parallelism. Previous security estimates were based on Wagner-type generalized birthday attacks with sub-exponential running time.

Chen, Liu, and Zhandry (CLZ, EUROCRYPT'22) describe a folklore algorithm attributed to Regev that solves ternary SIS modulo 
. Applying this algorithm to LtHash yields polynomial-time collision attacks requiring 
 hash queries, a result that has not been previously documented. We improve the Regev--CLZ algorithm by canceling two modulus bits per layer instead of one, while preserving ternary coefficients. This reduces the number of input columns---and hence the number of LtHash queries---from 
 to 
. This refutes the uniform average-case formulation of Liyan Chen et al.'s conjecture (ITC'26) that the Regev--CLZ bound is optimal. Our technique also extends to inhomogeneous SIS with the same bound.

For LtHash16 with 
, used at Facebook and in Solana, our attack requires approximately 
 hash queries. Its expected computational cost is at most 
 scalar operations over 
, compared with our estimates of approximately 
 operations for the Regev--CLZ algorithm and 
 for the state-of-the-art generalized birthday attack of Tang et al. (CRYPTO'26), under their respective operation models. Our query and scalar-operation costs are both far below the level suggested by the original claim of at least 200-bit security.
2026/2082 (PDF)
Last updated:  2026-09-18
Incremental Keyword Private Information Retrieval from d-ary Segmented Cuckoo Filters
Cryptographic protocols
Bao Ninh
Hide abstract
We introduce RisePIR, the first preprocessing keyword private information retrieval (PIR) scheme that absorbs insert, update, and delete on its key-value store at a cost proportional to the number of mutations alone. Where the static state-of-the-art schemes ChalametPIR and 
 re-run their preprocessing from scratch on every change, RisePIR patches its preprocessing in place and stays as practical as both in the online phase. We give a generic construction of incremental preprocessing keyword PIR from any updatable preprocessing index PIR, an index PIR that efficiently supports in-place update. The construction rests on our 
-ary Segmented Cuckoo Filter, a cuckoo filter that confines the 
-th candidate bucket of every key to the 
-th of 
 segments and stays dynamic under partial-key cuckoo hashing.

Instantiating the construction over FrodoPIR and SimplePIR, each equipped with the entry-level hint patch of iSimplePIR, gives RisePIR-F and RisePIR-S, both secure under decisional LWE. On a store of about one million keys, both variants patch their hint in place in 0.06–4.7 ms per mutation, five to six orders of magnitude faster than the 56–686 s full re-run of the preprocessing phase. Their online phase stays competitive with ChalametPIR and 
 in both asymptotic and concrete cost. We deploy RisePIR-S as a privacy-preserving eth_getBalance service over the 
 accounts of Ethereum mainnet, so that a wallet reads a balance without revealing to its RPC provider which account it holds. The running service answers a private query in 0.69 s and absorbs each block's changes in 5.6 ms, well inside the 12 s block interval. The filter and RisePIR ship as an open-source Rust implementation, together with the benchmark harness behind every number in this paper.
2026/2081 (PDF)
Last updated:  2026-09-18
FRI-Style Rank Metric PCS via FFT for Gabidulin Encoding
Cryptographic protocols
Songsong Li, Zhe Li, Shu Liu, Chaoping Xing, and Chen Yuan
Hide abstract
Polynomial commitment schemes (PCSs) are fundamental cryptographic primitives, with applications to SNARKs, data availability, and privacy preserving protocols. Many efficient transparent PCSs are built from interactive oracle proofs of proximity (IOPPs) for codes in the Hamming metric. In constructions based on the fast Reed-Solomon IOPP (FRI), low degree polynomials are encoded as Reed-Solomon (RS) codewords over suitable evaluation domains, and FRI is used to test proximity to the RS code. The efficiency of this approach relies on fast Fourier transform (FFT) based encoding and recursive proximity testing through folding operations.

Motivated by applications of rank metric codes in post quantum cryptography, random linear network coding, and distributed storage, we develop an analogous framework based on Gabidulin codes, the rank metric counterparts of RS codes. These codes encode 
-linearized polynomials of bounded 
-degree over 
 by evaluating them at 
-linearly independent points in this field. We construct a recursive FFT for these polynomials and show that its algebraic structure supports both proximity testing for Gabidulin codes and succinct verification of polynomial evaluations, yielding a FRI-style PCS for 
-linearized polynomials. Our contributions are as follows.

\begin{enumerate}
\item[(1)] \textbf{FFT algorithms for 
-linearized polynomials.} 
For every 
-smooth positive integer 
, we construct an 
-linearly independent evaluation set 
 of size 
. On this set, we present FFT and inverse FFT algorithms for evaluating and interpolating 
-linearized polynomials of 
-degree less than 
 over 
. Both algorithms require 
 operations in 
. Consequently, Gabidulin codes defined over 
 admit encoding with the same complexity.

\item[(2)] \textbf{A FRI-style IOPP for Gabidulin codes.}
The recursive structure of our FFT induces a folding operation that reduces a Gabidulin code instance of length 
 to one of length 
. Combining this operation with recent proximity gap results for Gabidulin codes, we construct a FRI-style IOPP, which we call the Fast Gabidulin IOPP (FGI). The soundness analysis accounts for the Frobenius twists introduced by folding and their effect on rank metric proximity.

\item[(3)] \textbf{A FGI based PCS for 
-linearized polynomials.}
The usual quotient based opening reduction in PCSs for ordinary polynomials does not directly preserve for 
-linearized polynomials. We instead use the FGI folding structure to recursively reduce the proximity claim and the evaluation claim together. Combined with Merkle tree commitments, this yields a transparent PCS for 
-linearized polynomials with polylogarithmic proof size and verification time. 
\end{enumerate}
2025/1845 (PDF)
Last updated:  2026-09-18
HE-based On-the-Fly MPC, Revisited: Universal Composability, Approximate and Imperfect Computation, Circuit Privacy
Foundations
Ganyuan Cao, Sylvain Chatel, and Christian Knabenhans
Hide abstract
On-the-fly multi-party computation (OtF-MPC, López-Alt, Tromer, and Vaikuntanathan, STOC 2012) allows clients to join a computation dynamically without remaining online, while outsourcing the computation to an untrusted but powerful server. Existing OtF-MPC constructions rely on composing multi-party homomorphic encryption with zero-knowledge or succinct arguments. While both components have seen substantial efficiency and expressivity improvements in recent years, the security of their composition has not been revisited under these modern refinements.

In this work, we revisit OtF-MPC through the lens of composable security. We extend the original analysis to systematically capture recent advances in these two building blocks, and to reason about their secure composition. We focus on multi-group homomorphic encryption (MGHE), a unifying abstraction that generalizes threshold and multi-key HE, and emerges as the natural backbone for OtF-MPC.

Our contributions are fourfold.
(1) We introduce the first Universal Composability (UC) ideal functionality for MGHE, jointly modeling client and server privacy, approximate or imperfect computation, and threshold security. (2) We define fine-grained security notions for MGHE and show that they are sufficient to realize this functionality against semi-malicious adversaries. (3) We revisit generic compilers that combine MGHE with any-simulation-extractable zkSNARKs, and prove that they realize OtF-MPC against fully malicious adversaries. (4) We introduce a split-witness ideal functionality for NIZKs, which separates the witness component that must be supplied by the ideal adversary from an existential component that only needs to be certified. We show that a Naor--Yung-style composition of MGHE and split-witness NIZKs realizes OtF-MPC while avoiding full extraction from the NIZK
2026/2080 (PDF)
Last updated:  2026-09-18
Lightweight Cryptography for Secure UAV Data Integrity Using Ascon-XOF128 and SHAKE128
Cryptographic protocols
Mehul Kumar Das, Varun Shukla, Prabhavi Tripathi, Vivek Shukla, Divya Mishra, and Atul
Hide abstract
The increasing deployment of unmanned aerial vehicles (UAVs) in communication, surveillance, and other mission-critical applications has created a growing requirement for efficient mechanisms to preserve the integrity of exchanged data. UAV platforms may operate under computational, memory, energy, and communication constraints, making lightweight cryptographic techniques an important consideration for secure data processing. This paper investigates the applicability of lightweight cryptography for UAV data integrity using two extendable-output functions (XOFs), Ascon-XOF128 and SHAKE128. A mathematical framework is developed to represent UAV messages, cryptographic processing, integrity verification, and relevant security and performance metrics. The two XOF constructions are analyzed with respect to their internal structures, sponge-based processing, security properties, computational characteristics, and suitability for resource-constrained UAV communication. A comparative evaluation framework is established using message-size and output-length variations, with performance and integrity-related measures including execution time, throughput, computational cost, memory usage, communication overhead, and Hamming-distance-based analysis. The study aims to provide an evidence-based comparison of Ascon-XOF128 and SHAKE128 for UAV data-integrity workloads and to identify the practical trade-offs between lightweight resource requirements and cryptographic integrity performance.
2026/2079 (PDF)
Last updated:  2026-09-18
Period-Aligned Secure Cosine Evaluation Based on Two-Party Computation
Cryptographic protocols
Qingyu Mo
Hide abstract
The cosine function is widely used in machine-learning applications,
but evaluating it on secret-shared data is difficult. Conventional
approaches use polynomial approximations or lookup tables, whose costs
grow substantially over large input domains. More recent protocols first
reduce the private input modulo the cosine period, but securely performing
this modular reduction also incurs significant cost. We present a
period-aligned two-party cosine protocol. Based on the cosine addition rule, it
rescales each share so that a wrap of the input modulus corresponds to
a full 
 period and therefore does not change the recombined cosine.
Each party evaluates sine and cosine locally on its share, while the
secure computation uses only two cross-party products and one fixed-point
output conversion. We evaluate the protocol directly on synthetic inputs
and within private RFF-based RBF-SVM prediction. Comparisons with the protocol of Xing et al. (NDSS 2025) and Guo et al. (USENIX Security 2026) demonstrate better efficiency of our proposed protocol.
2026/1667 (PDF)
Last updated:  2026-09-18
Resolving the Worst-Case Complexity of Linear Secret Sharing
Foundations
Oded Nir
Hide abstract
A secret-sharing scheme allows a dealer to distribute a secret among 
 parties such that only predefined “authorized” sets of parties can reconstruct the secret, and all other “unauthorized” sets learn nothing about it. Families of authorized sets are called access structures, and a scheme is called linear if its sharing map is linear in the secret and the dealer’s randomness.
We show that every 
-party access structure can be realized by a linear secret-sharing scheme over every finite field with at most 
 field elements per share. Taking the field to be 
 yields the same bound in bits for one-bit secrets.  A known counting lower bound for monotone span programs shows that for every fixed finite field, almost all access structures require share size 
 in every linear scheme, which makes our upper bound tight. Our scheme is considerably simpler than previous schemes that obtained share size 
 with 
. 

We also present a variant of this linear construction that is tailored for monotone k-DNF access structures (also known as 
-upslices). Then, by combining it with existing non-linear schemes, we derive a scheme for all access structures with share size 
. This improves the previous upper bound of 
 by Applebaum and Nir (CRYPTO 2021), and establishes a separation between the worst-case non-linear and linear exponents.

Additionally, we prove that almost all monotone functions require monotone span programs of size 
 simultaneously over all fields, finite or infinite, improving the previous bound of 
 for this setting. In particular, almost all access structures require information ratio 
 in linear schemes over every finite field, including fields whose size depend on 
.

Some of the constructions and proofs were discovered in conversations prompted by the author with GPT-5.6 Sol and Claude Fable 5.
2026/2078 (PDF)
Last updated:  2026-09-18
Radical Ring-LWR: Efficient Key Encapsulation and Signatures from Structured Rounding
Public-key cryptography
Joost Renes, Joppe W. Bos, Haochen Huang, Selim Kirbiyik, Alberto Ovena, Sujoy Sinha Roy, Frederik Vercauteren, Peng Wang, Fangyu Zheng, and Chenxin Zhong
Hide abstract
State-of-the-art lattice-based cryptography requires a power-of-two cyclotomic field that limits the attainable security levels, or a module structure for which the cost grows quadratically in the module rank. Radical rings were recently proposed as a solution in the context of Learning With Errors (LWE) based Key Encapsulation Mechanisms (KEMs) with heuristic hardness arguments for the Ring-LWE security and failure probability. We develop the Learning With Rounding counterpart, Radical Ring-LWR (RR-LWR). We give a proved closed-form bound on the distortion incurred from the error sampling independent of the radical ring parameters: this places RR-LWR inside the regime identified by Peikert as safe for instantiating Ring-LWE/LWR. We instantiate two schemes: Mithril, an IND-CCA KEM, and Octarine, an EF-CMA signature scheme. They are built on a single radical-ring arithmetic foundation based on powers of two (favorable for sampling, rounding and masking) and we provide security reductions in the QROM to RR-LWR and SelfTarget-RR-SIS, a radical-ring variant of SIS. We show that the KEM failure probability estimators used for power-of-two cyclotomics are insufficient and provide an exact solution tailored to the RR-LWR setting. Finally, we instantiate Mithril and Octarine with concrete parameters and benchmark optimized AVX2 and Arm Cortex-M4 implementations, showing that both are competitive with (and in some cases significantly faster than) the MLKEM and MLDSA standards.
2026/2077 (PDF)
Last updated:  2026-09-18
Practical Group Signatures from Tag-Based NTRU Sampler
Public-key cryptography
Corentin Jeudy
Hide abstract
The post-quantum migration for key agreements and signatures being well underway, the focus naturally shifts to other properties and primitives that still lack efficient solutions. One such area is that of privacy-enhanced primitives, with a growing number of post-quantum constructions. Among the most fundamental are group signatures, which represent an important milestone of anonymity and accountability towards more involved designs. However, despite recent progress, most compact lattice constructions are either computationally intensive or suffer large key materials, or both. 
In this paper, we introduce a new lattice group signature scheme that reaches compact signatures and keys, while being runtime-efficient in all its procedures. Our key element is a new tag-based gadget sampler that properly interfaces with NTRU, then benefiting from the desirable properties of such trapdoor frameworks for advanced constructions while leveraging the compactness of NTRU. Although applied to group signatures, our sampler may be of independent interest and used for more expressive privacy-driven primitives like anonymous credentials. We also introduce two assumptions, which can be seen as hybrid versions of NTRU and ISIS, that independently underly the security of our group signature. Despite their similarities with the recently introduced NTRU-ISIS
 assumption, they appear strictly harder and even admit reductions from R-ISIS in some parameter settings.
2026/1858 (PDF)
Last updated:  2026-09-18
On the Mismatch between Neural-Discovered Differential-Linear Features and Long-Round Distinguisher Construction
Attacks and cryptanalysis
Thomas Peyrin, Zilong Wang, Liu Zhang, and Chenlu Zheng
Hide abstract
To the best of our knowledge, existing differential-neural cryptanalysis have not yet shown a clear round advantage over the strongest comparable classical analyses. Recent Fourier-based interpretability results show that, under a difference-only representation, features extracted from differential-neural distinguishers can be interpreted as classical differential-linear masks. This suggests a possible route toward longer-round classical cryptanalysis and motivates our question: can such neural-discovered masks serve as useful candidates in the search for long-round differential-linear distinguishers of ARX ciphers?

As a prerequisite to the long-round study, we first characterize the short-round differential-linear candidates exposed by difference-only differential-neural distinguishers. We introduce Conv1DFully to facilitate mask-level analysis by removing the residual tower and reorganizing the first convolution along the ciphertext-difference bit dimension. On Speck32/64, the dominant differential-linear feature remains preserved after these modifications. On SipHash, we compare Fourier masks extracted from trained distinguishers with an exhaustive evaluation of a low-Hamming-weight output-mask space. The neural-extracted masks are concentrated among high-correlation differential-linear approximations, including several of the strongest candidates examined. These experiments provide a controlled basis for treating neural-extracted masks as candidates in the subsequent long-round analysis.

We then examine their utility in the known 18-round Speck128/128 distinguisher with a 5+8+5 decomposition. Under the same middle input difference, an 8-round difference-only differential-neural distinguisher recurrently exposes several masks with substantially stronger local middle correlations than the classically selected mask. However, after 5-round single XOR-linear extensions, these masks yield considerably weaker overall 18-round correlations. We further impose sparsity guidance on the first convolutional layer to promote low-Hamming-weight candidates. Under this guidance, the intermediate mask used in the classical 18-round distinguisher is recovered in the first-layer candidate set in 9 of 30 independent runs, showing that the neural model can reproduce a long-round-useful classical candidate. Nevertheless, this recovery is not stable, and the final neural decision rule still favors locally stronger features rather than the classically selected mask. These results indicate that differential-neural distinguishers can assist long-round candidate generation, while reliable recovery and long-round-aware prioritization remain unresolved.
2026/1718 (PDF)
Last updated:  2026-09-18
On Post-Quantum Multi-Key Security of GCM
Secret-key cryptography
Akinori Hosoyamada
Hide abstract
This paper studies the post-quantum multi-key security of Galois/Counter Mode (GCM) in the Quantum Ideal Cipher Model (QICM).
GCM is one of the most widely used AEAD schemes.
In practice, widely deployed cryptosystems are often instantiated under many independent keys, making the multi-key setting practically relevant.
A trivial reduction from the multi-key setting to the single-key setting incurs a security loss proportional to the number of keys.
In particular, in the post-quantum setting, the term corresponding to exhaustive key search becomes 
, where 
 is the number of keys, 
 is the key length, and 
 is the number of quantum queries to the underlying block cipher 
 and its inverse, with 
 serving as a coarse measure of the amount of offline (quantum) computation performed by the adversary.
For example, when 
, the term 
 reaches constant order at 
 for 
 and at 
 for 
.

We show that, at the cost of additional loss terms, the 
-dependent quantum key-search term 
 can be replaced by a term of order 
, where 
 denotes the maximum number of keys under which the same nonce appears in encryption queries.
Thus, when 
 is much smaller than 
 and the additional loss terms remain small, our bound improves upon the trivial multi-key bound in that the security bound reaches constant order only at a substantially larger value of 
.
This can be viewed as a post-quantum counterpart of the classical result of Hoang et al. (CCS 2018).
As in their work, for randomized nonce-generation mechanisms that abstract patterns used in protocols such as TLS, the parameter 
 remains small even when 
 is large.
Although our bounds are not tight and leave room for improvement, they give non-trivial post-quantum multi-key guarantees for several concrete parameter settings.
To the best of our knowledge, this is the first non-trivial post-quantum multi-key security bound for an AEAD mode in the QICM.
Our proof combines the reprogramming-and-resampling approach of Alagic et al. (EUROCRYPT 2022) with counting arguments used in the classical multi-user analysis of GCM by Hoang et al.
2026/1429 (PDF)
Last updated:  2026-09-18
Generalized Integral Properties: Separable Structure and Structured Searc
Attacks and cryptanalysis
Dongchen Chai, Shenghu Hu, Thomas Peyrin, Zilong Wang, Trevor Yap, Hongyi Zhang, Liu Zhang, and Chunning Zhou
Hide abstract
Generalized integral properties extend classical integral distinguishers by allowing general Boolean mappings over plaintext and ciphertext variables. This additional flexibility raises two natural cryptanalytic questions: whether generalized mappings can yield higher-round distinguishers, and whether the resulting relations can be exploited to improve key-recovery
attacks. Addressing these questions requires understanding which generalized integral structures are relevant and how they can be searched systematically. In this work, we introduce a structural classification of key-independent generalized integral properties into separably generated and residual-cancellation properties. Although some previously reported generalized integral properties appear to involve cancellations among key-dependent residuals, we show through explicit decompositions that they are in fact separably generated. Motivated by this view, we search for balanced separable terms within explicit plaintext-side and ciphertext-side candidate spaces.

We develop two complementary automated search strategies: a unified Matrix--Bipartite Graph--Matrix framework for joint plaintext--ciphertext search and a specialized Gaussian-elimination-based solver for ciphertext-side mappings under a fixed plaintext structure. Across five representative block ciphers, the proposed methods extend previous integral distinguishers by one round for SIMON and for SPECK variants other than SPECK32, while the other evaluated ciphers mainly yield additional balanced mappings at the same round depth. We further show that the underlying round-extension mechanisms for the SPECK family and SIMON arise from exploitable algebraic structures in their round functions, whereas no comparable simplification is observed for the remaining ciphers under the current search setting. Using SKINNY-64-64 as a case study, we show that the additional nonlinear relations provide stronger filtering but destroy the computational separability exploited by existing partial-sum techniques, and therefore do not yield a better key-recovery attack under the current approach.
2026/2075 (PDF)
Last updated:  2026-09-17
Bounding the Excess Risk for Linear Models Trained on Marginal-Preserving, Differentially-Private, Synthetic Data
Applications
Yvonne Zhou, Mingyu Liang, Ivan Brugere, Danial Dervovic, Antigoni Polychroniadou, Min Wu, and Dana Dachman-Soled
Hide abstract
The growing use of machine learning (ML) has raised concerns that an ML model may reveal private information about an individual who has contributed to the training dataset. To prevent leakage of sensitive data, we consider using differentially-private (DP), synthetic training data instead of real training data to train an ML model. A key desirable property of synthetic data is its ability to preserve the low-order marginals of the original distribution. Our main contribution comprises novel upper and lower bounds on the excess empirical risk of linear models trained on such synthetic data, for continuous and Lipschitz loss functions. We perform extensive experimentation alongside our theoretical results
2026/2074 (PDF)
Last updated:  2026-09-17
Revisiting Malicious Private Aggregation: Formalization, Weaknesses, and Enhancements
Cryptographic protocols
Ananya Appan, Pranav Shriram Arunachalaramanan, David Heath, and Ling Ren
Hide abstract
Private aggregation schemes enable aggregation of sensitive data across clients without leaking any individual client's input. Their applications include privacy-preserving federated learning, private heavy hitters, and anonymous systems.

Many private aggregation schemes assume two non-colluding servers and aim to guarantee privacy even when one server is malicious, i.e., the malicious server learns an aggregation result that includes all honest client inputs. However, many private aggregation schemes in the literature fail to achieve privacy, which we believe is in large part due to a lack of formalism for the nuanced variations of privacy guarantees.

In this work, we present ideal functionalities for several variants of private aggregation. We also formalize a common paradigm underlying most prior private aggregation schemes. The formal treatment helps us identify security flaws or underspecifications in existing private aggregation schemes. We then present modular modifications or fill in critical details to help prior schemes in the share-aggregate paradigm securely realize the private aggregation functionalities we formally define, including in settings where clients may have unreliable networks.
2026/2073 (PDF)
Last updated:  2026-09-17
Revisiting ML Training under Fully Homomorphic Encryption: Convergence Guarantees, Differential Privacy, and Efficient Algorithms
Applications
Yvonne Zhou, Mingyu Liang, Ivan Brugere, Danial Dervovic, Yue Guo, Antigoni Polychroniadou, Min Wu, and Dana Dachman-Soled
Hide abstract
We present the first theoretical convergence analysis of machine learning training under fully homomorphic encryption (FHE), combined with a differentially private (DP) training algorithm tailored to encrypted computation. Our approach improves computational efficiency over standard differentially private gradient descent (DP-GD) while achieving comparable utility. In particular, we prove convergence of approximate gradient descent using polynomial approximations of activation and loss functions, which are required for FHE compatibility. To preserve privacy in downstream tasks, we integrate differential privacy without relying on costly per-sample gradient clipping, enabling scalable encrypted learning. We also provide data-independent hyperparameter selection and theoretically grounded strategies for polynomial approximation which can be of independent interest. Together, these contributions advance the feasibility of efficient, private, and secure machine learning on sensitive data.
2026/1907 (PDF)
Last updated:  2026-09-17
Higher-order differential attacks on the full DuX
Attacks and cryptanalysis
Guoqiang Liu and Bing Sun
Hide abstract
DuX is a family of substitution-permutation block ciphers over 
 with 
 and twelve rounds. For the two large-word instances its designers estimate that integral and higher-order differential distinguishers in the encryption direction reach at most six rounds, and fix the number of rounds accordingly. We bound the word-wise algebraic degree of DuX by exponent sets in the decryption direction, where the decryption S-box has coordinate degrees 
 against the 
 of the encryption S-box. Each row of the inverse diffusion matrix is supported on two residue classes of word indices modulo four. An active set equal to one class therefore keeps the vector of degree bounds constant within each class at every layer, and we prove that the four class-wise bounds then obey an exact recursion with base 
 instead of four. This yields higher-order differential distinguishers for eleven rounds of DuX(
) and DuX(
) with 
 chosen ciphertexts, and for seven rounds of DuX(
) with 
. Extending one round towards the plaintext gives three successive systems of equations. Every unknown there has a coefficient computed from the returned plaintext words, and the key words recovered at one stage are substituted into the next; for DuX(
) the last two systems are replaced by bivariate interpolation. Once each system attains the maximal rank that its structure permits, a condition that can be checked during the attack, solving the systems recovers all sixteen master-key words of the full twelve-round DuX(
) and DuX(
). The data and time complexity is 
 and 
 with constant memory, and eight rounds of DuX(
) are covered with 
. The two diffusion layers differ by a rotation of four words, so the analysis is the same for each of the 
 key-dependent choices of diffusion layers.
2026/2072 (PDF)
Last updated:  2026-09-17
Sunshine Systems: Enabling Privacy-Preserving Compliance with Freedom of Information Laws
Applications
Aarushi Goel, Gabriel Kaptchuk, and Yuange Li
Hide abstract
Freedom of information laws (informally known as Sunshine Laws), such as the US Federal Freedom of Information Act, provide legal pathways for members of the public to compel governments to produce internal documentation.  These laws have become a powerful tool for government accountability, allowing journalists and activists to uncover malfeasance and advocate for change. However, because compliance with these laws requires government actors---the same entities who may be accused of malfeasance---to produce documentation, there is a risk that evidence of malfeasance may be suppressed rather than produced in response to a query.

In this work, we introduce a cryptographic framework called Sunshine Systems, designed to constrain government actors' ability to circumvent freedom of information laws.  At a high level, Sunshine Systems enable agencies to prove, in zero knowledge, that they have produced the complete and correct set of documents responsive to a given request.  We instantiate a Sunshine System that supports keyword queries, building on recent advances in lookup arguments.  We implement and evaluate our approach on low-cost hardware, demonstrating its practicality even for small government agencies.
2026/2031 (PDF)
Last updated:  2026-09-17
Mind Small Integers in MDS Matrix: Collision Attacks on Round-Reduced Reinforced Concrete
Attacks and cryptanalysis
Jiamin Cui, Fukang Liu, Jianqiang Ni, and Willi Meier
Hide abstract
The rapid advancement of Zero-Knowledge Proofs (ZKP) has motivated the design of ZK-friendly hash functions. 
A relatively new design strategy for ZK-friendly hash functions is using the composition of small look-up tables (LUTs) to build a nonlinear transform (called \bars layer) over a large prime field 
. This not only improves the plain performance but also enhances its security against algebraic attacks since the \bars layer is equivalent to a complex and high-degree polynomial over 
 This design strategy was first proposed at CCS 2022 for the ZK-friendly hash function \rc. However, there has been no third-party collion attacks of \rc since then, and all existing attacks on such LUT-based ZK-friendly ciphers like \tip, \mono and \sky mainly exploit the differential or linear properties of \bars.

In this paper, we demonstrate that the \bars layer surrounded by MDS matrices (called \conc layers) with small integers, i.e., 
, might lead to weaker differential properties of the round function. This key observation leads to a novel collision attack on 4.5 out of 7 rounds of \rc, successfully bypassing the \bars layer  as well as the subsequent \conc layer and power-map-based nonlinear layer (called \brick layer). The attack is verified by providing a practical collision for 3.5-round \rc where the last 4 layers are 
.

Furthermore, we provide constructive countermeasures by designing an algorithm to generate an improved \conc layer that thwarts this specific attack without increasing the number of arithmetic constraints compared to the original design.
Finally, we also applied our framework to \tip but found that its MDS matrix can effectively prevent this attack. This is the first time to exploit the weak combination of \bars layer and \conc layer to mount efficient attacks on LUT-based ZK-friendly hash functions. We believe that it sheds new insight into the security of these hash functions.
2025/181 (PDF)
Last updated:  2026-09-17
Combined blinding Side-Channel and Fault countermeasures for Lattice-Based schemes
Public-key cryptography
Mounir Taha, Julien Cam, Max Duparc, Herve Pelletier, and Karine Villegas
Hide abstract
In this paper, we revisit and enhance two blinding countermeasures aimed at protecting implementations of lattice‑based cryptographic schemes against side‑channel and fault‑injection attacks. First, we build upon the Number Theoretic Transform (NTT) blinding technique introduced by Ravi et al., and optimize the use of blinding twiddle factors so as to maximize their number without increasing the total number of modular multiplications. Second, we improve the Chinese Remainder Theorem (CRT)–based blinding scheme proposed by Heinz and Pöppelmann by substantially reducing the randomness‑sampling overhead and streamlining the associated fault‑detection mechanism. Importantly, the two countermeasures are fully compatible and can be seamlessly combined to provide enhanced resistance to both classical and soft‑analytical side‑channel attacks, as well as fault‑injection attacks, while maintaining a performance overhead comparable to existing approaches. We demonstrate the practicality of our proposals through a proof‑of‑concept implementation integrated into Kyber/ML‑KEM.
2026/1979 (PDF)
Last updated:  2026-09-17
From Specs to Apps: Verifying and Monitoring Models of Signal and WhatsApp
Cryptographic protocols
Moustafa Said, Aurora Naska, Kevin Morio, and Robert Künnemann
Hide abstract
The Signal protocol is a prominent messaging protocol that secures communication for billions of users. It powers WhatsApp, the most widely used messaging application worldwide, and the Signal app, popular among privacy-conscious users. Extensive research in the computational and Dolev-Yao settings provides strong formal security guarantees for the protocol itself. However, a gap remains between the guarantees of the protocol specification and the implementation’s actual behavior at runtime.

In this work, we bridge this gap by applying SpecMon, a recently proposed runtime monitor, to check whether observed executions conform to formal protocol models. To this end, we instrument two applications (WhatsApp Web and Signal Desktop) to capture their interactions with the network and the cryptographic components. Using this instrumentation, we develop two multiset-rewrite models that are compatible with Tamarin, thus enabling verification. We derive the first model of WhatsApp Web’s implementation of the Signal protocol and the most detailed model to date of Signal’s original protocol. Monitoring establishes that observed executions conform to these models, relative to the trusted event extraction and the symbolic abstraction. For the core components of the Signal protocol, we verify authentication and secrecy properties. Finally, monitoring reveals previously undocumented differences between the original libsignal library and WhatsApp’s fork.

We evaluate our methodology and demonstrate its reproducibility. Developing the WhatsApp Web model, instrumenting the app, adding fuzzing, and running the experiments took three person-weeks. We also demonstrate efficient monitoring of real-world applications and detection of deliberately injected security faults, with low overhead in our measured setting.
2026/2071 (PDF)
Last updated:  2026-09-17
Soft Analytical Side-Channel Attacks on SHA-2 and HMAC
Attacks and cryptanalysis
Maxime Lecomte, Julien Maillard, Antoine Moran, Guénaël Renault, and Benjamin Smith
Hide abstract
We investigate the use of bit-level Soft Analytical Side-Channel Attacks (SASCA) to recover secret inputs of the SHA-256 hash function, and secret keys of HMAC-SHA-256, using Sentential Decision Diagrams to overcome the computational challenge posed to classical Belief Propagation by multiple-word modular addition.
Our attack on HMAC requires no control nor knowledge of the input, and exploits its double manipulation of the key to improve key-recovery performance. We make comprehensive simulations to evaluate the attacker’s recovery capacity for both SHA-256 and HMAC-SHA-256. Finally, we study the profiling capabilities of load instructions in a multiposition EM probe setup on a STM32F303RET6 microcontroller, and use these results to evaluate the capability of our attack against software implementations of HMAC-SHA-256 in a realistic scenario.
2026/1232 (PDF)
Last updated:  2026-09-17
A Heuristic Subexponential Attack on the McEliece Cryptosystem
Attacks and cryptanalysis
Pierre Briaud, Axel Lemoine, Hugues Randriambololona, and Jean-Pierre Tillich
Hide abstract
We provide a new way of performing an algebraic attack on the McEliece cryptosystem based on binary Goppa codes. It also applies in general to the case where the field over which the Goppa code is defined is of even characteristic. It is based on a new algebraic modeling for finding matrices of rank 
 in the code of quadratic relations related to the Goppa code that is attacked. Such matrices are then used to recover the secret algebraic structure of the code, from which an equivalent secret key can be efficiently derived, leading to a full key-recovery attack. A byproduct of our approach is a new distinguisher for Goppa codes in even characteristic which is as the syzygy distinguisher of \cite{R25}subexponential in the security level of the scheme. We demonstrate the effectiveness of our attack on McEliece TII challenges, some of which having been studied in \cite{BLT26}, and aimed at having 83,89,119,166, 210 and even 248 bit security respectively and CFS keys with parameters 
 and 
, corresponding to a security of 
 bits according to \cite{LS12}. This CFS key was not attacked in practice in \cite{BLT26} and took us 14 hours of computation and 24GB of RAM. We make the conjecture that this attack has a complexity which is of the same nature as the distinguisher, namely subexponential in the security level. It should be noted that this attack is at best of complexity of order 
 for NIST level 1 Classic McEliece parameters.
2025/2133 (PDF)
Last updated:  2026-09-17
Byzantine Broadcast with Unknown Participants
Cryptographic protocols
Wonseok Choi, Ran Cohen, Juan Garay, Nikos Skoumios, and Vassilis Zikas
Hide abstract
A sender wishes to broadcast a message across an open, permissionless network (e.g., on the dark web) such that whoever is around and active reaches agreement, even if the sender is malicious. Crucially, no a priori knowledge of the number of parties or their identities is given, and no assumptions are made regarding the number of honest parties or existence of resource-allocation mechanisms, like proof-of-work or stake. 

The above is an instance of Byzantine broadcast (BB) with unknown-participants (UP Broadcast). Despite extensive research on dishonest-majority BB, existing approaches (such as Dolev-Strong broadcast) fail in this setting, as they rely on knowing the number of parties or corruption bounds. A key challenge, which might appear as an inherent limitation, is that malicious parties can join dynamically, making it difficult for honest parties to terminate without violating consistency. 

In this work, we provide the first definitions of UP Broadcast, accommodating both static and dynamic participation under arbitrarily many corruptions. We show that the problem is solvable by introducing a novel primitive called active-parties set agreement (APSA), which we use to construct a polynomial-time deterministic UP Broadcast protocol (as well as UP Interactive Consistency). Our protocols guarantee consistency and validity for all honest parties active throughout the execution.

Furthermore, we settle the round complexity of UP Broadcast by proving that our deterministic protocols are round-optimal, matching a lower bound that holds even for randomized protocols. Finally, while we establish negative results for unrestricted dynamic participation where parties may join late, we formulate best-possible guarantees for late-joining parties alongside matching round-optimal constructions.
2026/848 (PDF)
Last updated:  2026-09-17
Cryptanalytic Extraction of Neural Networks for Privacy-Preserving Machine Learning
Attacks and cryptanalysis
Wen Zhang, Bingsheng Zhang, Tianpei Lu, and Kui Ren
Hide abstract
Since Carlini et al. (CRYPTO'20) introduced a black-box neural network extraction method inspired by differential cryptanalysis, cryptanalytic model extraction has been extensively studied. Existing methods, however, still face three bottlenecks.

\begin{itemize}
\item First, these methods are largely confined to contractive architectures and generally do not extend to expansive networks because of rank deficiency. We classify the regimes for single-point and multi-point recovery and mitigate this limitation through projection-based aggregation in the multi-point case.
\item Second, prior attacks fundamentally exploit the non-differentiability of activations such as ReLU. Asselineau et al. (CRYPTO'26) recently extended cryptanalytic extraction to a broad class of nonlinear activations, but their approach requires high numerical precision and may recover only a subset of the parameters. We show that privacy-preserving machine learning (PPML) unintentionally introduces a new attack surface: fixed-point arithmetic over finite rings induces silent modular wraparound, creating observable discontinuities even when the underlying activation is smooth. Exploiting these discontinuities, we recover all parameters of networks with smooth activations at much lower precision than prior work.
\item Third, Neuron Wiggle, a widely used sign-recovery technique, is suboptimal. We recast it as a unified paradigm and analyze it from a signal-detection perspective. We prove that when the downstream coefficients are Gaussian, or the network is sufficiently wide, there exists an optimal probing direction, and the resulting method outperforms the original algorithm.
\end{itemize}

We evaluate our attack on eight CIFAR-10 DNNs and eight expansive networks with different activation functions. The attack extracts all parameters of expansive networks with smooth, unbounded activations. Our sign-recovery method further attains 
 accuracy on our benchmark while reducing the number of queries by 
--
 relative to Neuron Wiggle.
2026/973 (PDF)
Last updated:  2026-09-17
Asynchronous Shamir-Based Threshold FHE: Smaller Moduli and Strong Simulation Security
Cryptographic protocols
Won Kim, Changmin Lee, JeongHwan Lee, Alain Passelègue, and Damien Stehlé
Hide abstract
We study 
-out-of-
 threshold fully homomorphic encryption (ThFHE) based on Shamir secret sharing (SSS) in the asynchronous setting. With SSS, a partial decryption share is a single ring element; this allows to avoid the large number of ring elements stored per party required by prior strongly secure constructions based on 
 reconstruction coefficients. However, this reduction introduces a fundamental trade-off: Lagrange reconstruction during distributed decryption severely amplifies noise, forcing a substantially larger ciphertext modulus. 

In this work, we give SSS-based asynchronous ThFHE constructions over standard RLWE-based FHE schemes (BFV, BGV, and CKKS) for both standard and strong simulation security. Through tight analysis of the modulus constraints, our constructions simultaneously reduce the required modulus, compared with prior asynchronous Shamir-based constructions, and the per-party storage, compared with the prior strongly secure construction. For example, for 
 under standard simulation security, our analysis reduces the modulus overhead of distributed decryption by 
 at 
 and by up to 
 for 
 close to 
, relative to the prior works.  Under strong simulation security, the per-party storage for 
 and 
 at 
-bit security drops from 
 GB in prior work to 
 KB.
2025/1643 (PDF)
Last updated:  2026-09-17
SCA-GPT: A Generation-Planning-Tool Assisted LLM Agent for Fully Automated Side-Channel Analysis on Cryptosystems
Attacks and cryptanalysis
Wenquan Zhou, An Wang, Yaoling Ding, Annyu Liu, Jingqi Zhang, Jiakun Li, and Liehuang Zhu
Hide abstract
Non-invasive security testing under standards such as ISO/IEC 17825 still relies on human experts: how to measure a given target, and what the standard requires once the results are available, differ with every device and algorithm class, so the procedure is composed anew for each assessment. We present SCA-GPT, a large language model (LLM) agent for side-channel evaluation: given only a single natural-language instruction, it completes a full standard-conformant assessment covering timing analysis, simple power analysis, and differential power analysis, reporting for every test item a verdict, the governing clause, and checkable numeric evidence. The agent integrates a domain-specific expert knowledge base curated from 155 documents with a suite of specialized side-channel analysis tools. In retrieval experiments, the expert knowledge base achieves a Hit Rate@5 of 89.7% and an MRR@5 of 74.1%. We further evaluate the full framework with three LLMs on 13 real datasets covering six cryptographic algorithms on smart-card, microcontroller, and FPGA targets. DeepSeek-V3.2, Kimi-K2.6, and Qwen3.5 achieve success rates of 90.0%, 87.7%, and 94.6%, respectively, versus 0.0% for a plain ReAct agent, at an average of 253 seconds per assessment. To our knowledge, SCA-GPT is the first automated LLM agent for standard-conformant side-channel evaluation.

Scroll to top
2026/071 (PDF)
Last updated:  2026-09-17
Codeword Masking Can Be Harmful Under Replay Attacks on HQC
Attacks and cryptanalysis
Jaeho Jeon, Yongseong Park, Jaeyeon Lee, Suseong Lee, Donghyen Kim, and Young-Sik Kim
Hide abstract
HQC, selected by NIST for standardization in 2025, was recently shown
vulnerable to a replay attack recovering 
, from which the long-term secret 
 was shown to be recoverable, and against which \emph{codeword masking} was proposed as the countermeasure. We revisit both and demonstrate that the countermeasure yields an opposite effect: on the target build, the unmasked replay attack is ineffective, whereas introducing codeword masking paradoxically enables its success. We present a new leakage venue in HQC, in the Reed--Muller encoder, and apply to it the post-decoding idea of recent work, turning HQC's own error correction into a side-channel budget that recovers the message: a single trace suffices, with only 
 profiling traces, and we confirm the same across the official implementations (the NIST reference and optimized code, and PQClean) at both \texttt{-O3} and \texttt{-Os}. We then improve the replay attack on 
, raising its per-bit recovery probability; at the optimization level we study, that is still not enough for a practical attack. Codeword masking, adopted as the defense, supplies what replay could not: the random share erases the ambiguity that identical replays leave behind, and the extrapolated probability of recovering the whole 
 rises from 
 to 
. Increasing the masking order fails to mitigate the attack: we demonstrate identical recovery against a three-share implementation, without incurring additional profiling overhead for the adversary. Masking must therefore be designed against the optimization level and the device it is deployed on.
2026/2069 (PDF)
Last updated:  2026-09-17
Default Correct: A New Fault Surface in the Comparison Booleanisation of Kyber-KEM
Public-key cryptography
Anirudh Jaiswal, Abhilash Kumar Das, and Dhiman Saha
Hide abstract
The Fujisaki–Okamoto (FO) transform protects Kyber-KEM against chosen-
ciphertext attacks and is based on a ciphertext comparison step. This comparison is usually treated as a single atomic check. However, in every mainstream implementation it is a short pipeline of independently faultable stages. It comprises a byte-wise mismatch accumulation, a two’s-complement Booleanisation, and a conditional move (cmov). In this work, we expose a previously unexamined stage of this pipeline, the Booleanisation, as a new attack surface. Our research reveals that a
single sub-instruction clock glitch forces the mismatch flag fail to 0, so decapsulation accepts any ciphertext. Every prior fault attack on this comparison targets the equality test or the conditional move. So does the only countermeasure proposed for it: the default-fail cmov ordering of Xagawa et al. Our surface sits upstream of that move, so it escapes both. We instantiate this Default Correct fault on an ARM Cortex-M4 (ChipWhisperer-Lite) and demonstrate it across the pqm4, PQClean, and
reference implementations of Kyber-512/768/1024 at all optimisation levels (-O0–O3). Crucially, every one of these implementations already carries Xagawa’s default-fail ordering, yet the fault succeeds on all of them: the surface is therefore not merely new but structurally beyond the reach of the state-of-the-art defence for this comparison. The Booleanisation is thus a single point of failure common to all three code bases. Leveraging the accept-everything behaviour, we use the standard plaintext-checking
oracle and recover the full secret key of Kyber-512, Kyber-768, and Kyber-1024 in a single on-device run each. Finally, we analyse why defences that protect only the cmov or a single fail flag cannot address a fault that targets an upstream stage of the comparison, and we propose a producer-side countermeasure: an integrity-checked Booleanisation that computes the same flag while folding in an execution counter, so that any partial instruction skip is detected, at negligible overhead.
2026/2068 (PDF)
Last updated:  2026-09-17
Distributed SNARGs Resilient to Corrupt Verifiers
Foundations
Elette Boyle and Lalita Devadas
Hide abstract
Distributed certification is a method for monitoring the correctness of a distributed system. The model consists of a centralized prover in addition to multiple verifiers lying on the nodes of a communication network, where the goal is to assert that the network satisfies a desired property. In doing so, the prover generates certificates for each verifier; the verifiers can then communicate in a small number of rounds, and accordingly accept or reject. The prover’s assertion is accepted if all verifiers accept.

A significant body of work has gone toward developing and understanding limitations of distributed certification schemes, predominantly in the setting of information-theoretic soundness, and recently with computational soundness, achieving a form of distributed (locally verifiable) Succinct Non-interactive Arguments (SNARGs) (Aldema Tshuva et al, TCC 2023). As is standard in the model, soundness holds in existing constructions assuming that all verifiers in the network are honest.

In this work, we introduce and explore the notion of robust distributed SNARGs (rdSNARGs) which retain (computational) soundness guarantees even when the cheating prover can collude with some nodes in the network. Addressing cheating verifiers presents several challenges. We construct rdSNARGs for any distributed language in P with succinct certificate size and communication from extended versions of RAM SNARGs (Kalai et al, STOC 2023), where the level of succinctness scales with the threshold of corrupt nodes. Complementarily, we demonstrate a lower bound showing that an rdSNARG with significantly smaller certificates and communication implies a SNARG for NP.
2026/605 (PDF)
Last updated:  2026-09-17
Adaptively-Secure Proxy Re-Encryption with Tight Security
Public-key cryptography
Chen Qian, Shuo Chen, and Shuai Han
Hide abstract
(Bi-Directional) Proxy Re-Encryption (
) is a public-key encryption scheme that allows a proxy, holding a re-encryption key from 
 to 
, to transform a ciphertext intended for 
 into one intended for 
. 
 has numerous applications, including secure data sharing and cloud computing. However, most existing 
 schemes experience significant security degradation when adversaries are allowed to adaptively corrupt re-encryption or secret keys. Prior to this work, only a few 
 schemes achieved quasi-polynomial security loss in the adaptive setting, and even those were limited to restricted re-encryption strategies.

In this paper, we propose four distinct 
 schemes with tight security guarantees in the adaptive setting, based on the 
 assumption:
- 
, 
: Single- and multi-challenge 
-secure 
 schemes with tight security focusing on efficient constructions.
- 
, 
: Single- and multi-challenge 
-secure 
 schemes with (almost) tight security focusing on 
-type security.

To achieve tightly 
-secure 
 schemes, we introduce a novel concept called tag-based language-malleable 
 with special simulation soundness. This primitive provides simulation-sound 
 while preserving a restricted form of malleability. We construct both one-time and unbounded versions of this primitive under the 
(Matrix Decisional Diffie-Hellman) assumption.
2025/248 (PDF)
Last updated:  2026-09-17
New Exchanged Boomerang Distinguishers for 5-Round AES
Attacks and cryptanalysis
Hanbeom Shin, Seonkyu Kim, Byoungjin Seok, Dongjae Lee, Deukjo Hong, Jaechul Sung, and Seokhie Hong
Hide abstract
In block ciphers, the attacker should not be able to distinguish a block cipher from a random permutation; therefore the existence of a distinguisher is important. Cryptanalysis of the reduced-round variants of block ciphers is also important in cryptographic design. AES is the most widely used block cipher, and currently, the best-known distinguisher for 5-round AES has a data and time complexity of 
 with a success probability of 55\%. In this paper, we propose the massive exchanged boomerang and multiple exchanged boomerang distinguishers for 5-round AES. The massive exchanged boomerang distinguisher utilizes the probability that the truncated difference for the returned plaintext pairs is such that, in each of its diagonals, the 4 bytes are either all active, or all inactive. Although this probability is very high for a random permutation, we significantly reduce it using the friend pairs technique, while keeping the boomerang probability unchanged. This enables us to distinguish a block cipher from a random permutation. The massive exchanged boomerang distinguisher for 5-round AES has a data and time complexity of 
 with a success probability of 70\%. The multiple exchanged boomerang distinguisher is constructed by clustering four trails that have the same input and output truncated differences, enabling it to distinguish a block cipher from a random permutation with lower complexity and higher success probability. The multiple exchanged boomerang distinguisher for 5-round AES has a data and time complexity of 
 and a success probability of 79.6\%, which represents a new best-known result for the secret-key distinguisher on 5-round AES.
2026/2067 (PDF)
Last updated:  2026-09-17
Maltese: Succinct Polynomial Commitment from Lattices
Cryptographic protocols
Katarina Cheng, Wilson Nguyen, and Nirvan Tyagi
Hide abstract
Succinct polynomial commitment schemes are a key building block in succinct non-interactive arguments of knowledge (SNARKs). Existing succinct lattice-based polynomial commitment schemes take a "split-and-fold" strategy making use of homomorphism to compress proof size to 
 for polynomials of size 
. Prior works either (1) incur superlogarithmic verifier work (e.g., 
), (2) incur concretely large proof sizes and verifier work (e.g., hundreds of MB), or (3) rely on non-standard lattice assumptions. We proprose a new succinct polynomial commitment scheme Maltese that operates over a Merkle tree commitment of the lattice-based Ajtai hash function. We employ a folding strategy and propose new sum-check-based reductions for managing norm growth of the tree commitment over folding rounds. Maltese is secure under the standard Module-SIS assumption and produces opening proofs of 
KB for multilinear polynomials of size 
; proofs are around 
 smaller than prior work with polylogarithmic verifier complexity but between 
-
 larger than prior work with larger verifier complexity or stronger structured assumptions.
2026/2066 (PDF)
Last updated:  2026-09-17
Optimizing HAETAE and SMAUG-T on Cortex-M4 for Resource-Constrained IoT Devices
Implementation
JunHyeok Choi, DongHyun Shin, and Seog Chung Seo
Hide abstract
The practicality of post-quantum cryptography (PQC) on IoT devices depends on both operation cycle counts and communication costs from public values (public keys, ciphertexts, and signatures). The DSA HAETAE and the KEM SMAUG-T, both selected in the KpqC competition, provide smaller public values than ML-DSA and ML-KEM; however, the lack of platform-specific optimization leaves their operation cycle counts high, which can offset this advantage. In this paper, we optimize HAETAE and SMAUG-T on the Cortex-M4, a representative 32-bit embedded MCU, and conduct a multi-faceted evaluation. We fix two bugs in the existing implementation of the HAETAE 
 evaluation and propose Autocorrelation-based Spectral-Norm Evaluation (ASNE), which replaces the per-polynomial FFTs with a single autocorrelation FFT. Together with further optimizations, we achieve improvements of up to 309.1%, 4.8%, and 6.0% in KeyGen, Sign, and Verify, respectively, over the state-of-the-art implementation. For SMAUG-T, we analyze the coefficient ranges of the polynomial multiplication operands to select an auxiliary NTT-friendly modulus 
, and count the number of operations per layer to apply the optimal NTT. As a result, we achieve improvements of up to 266.7%, 348.9%, and 341.0% in KeyGen, Encaps, and Decaps, respectively, over the reference C implementation. Furthermore, we apply static analysis and dudect measurements to the optimized kernels and observe no evidence of timing leakage. Using models anchored to prior IoT protocol measurements, we estimate the transmission-unit count, end-to-end latency, and energy, showing that HAETAE and SMAUG-T can be competitive PQC alternatives in constrained wireless environments.
2026/544 (PDF)
Last updated:  2026-09-17
HARE: Compact HQC via Distance-Informed Erasure Decoding
Public-key cryptography
Tianrui Wang, Qicheng Teng, Anyu Wang, Jun Zhang, Bo Pang, Chunhuan Zhao, Sihuang Hu, and Xiaoyun Wang
Hide abstract
We present HARE, a key encapsulation mechanism (KEM) scheme based on the Hamming Quasi-Cyclic (HQC) framework that achieves substantially reduced public key and ciphertext sizes. HARE focuses on two complementary techniques: a novel distance-informed erasure decoding method for the concatenated code and ciphertext compression based on covering codes.
The distance-informed erasure decoding method uses distance information from the inner Reed-Muller decoder to identify unreliable local blocks and treats them as erasures for the outer Reed-Solomon decoder.This improves decoding performance and enables more compact code parameters. For ciphertext compression, HARE adopts a covering-code perspective and maps the ciphertext vector to a nearby codeword with a shorter representation. The use of the Kaikkonen-Rosendahl code provides a more favorable bandwidth trade-off than previously considered Hamming and Golay code constructions.
Combining these techniques, HARE reduces the combined size of the public key and ciphertext by 15.4%, 14.8%, and 15.3% at NIST security categories 1, 3, and 5, respectively, compared with HQC.
2026/1770 (PDF)
Last updated:  2026-09-17
How Many Traces Suffice? PAC Guarantees for Profiled and Non-profiled Side-Channel Analysis
Attacks and cryptanalysis
Seyedmohammad Nouraniboosjin and Fatemeh Ganji
Hide abstract
Side-channel analysis (SCA) is commonly evaluated by reporting the number of traces required to reduce the rank of the correct key. Still, such evaluations remain empirical and do not explain how many traces suffice for reliable recovery, how profiling and attack data contribute separately, or when additional traces cannot overcome weak key distinguishability. We address these questions through a Probably Approximately Correct (PAC) formulation of profiled and non-profiled SCA. Our framework treats candidate-key scores as the common cryptanalytic object and separates finite-sample estimation from the intrinsic separation between the correct key and competing hypotheses. This distinction enables confidence guarantees for key rank and helps determine whether an attack failure is due to insufficient data or an inherently weak attack score. We instantiate the framework with representative profiled and non-profiled attacks chosen for their analytical tractability. Experiments on ASCAD-f and ASCAD-r show that this analytical tractability does not come at the cost of impractical attack performance. The profiled attack achieves exact recovery with tens of attack traces, whereas the non-profiled single-attack rank certificate guarantees exact recovery with about 1,000 traces. These results are competitive with recent ASCAD attacks and, in the non-profiled setting, substantially below the smallest trace counts identified in prior studies, while additionally providing finite-sample guarantees on key rank. More generally, the same finite-sample rank analysis can be adapted to other learners and distinguishers by deriving the corresponding score-gap guarantees. Overall, the framework turns trace complexity from an empirical attack observation into a reusable finite-sample criterion for key recovery.
2026/980 (PDF)
Last updated:  2026-09-17
Key-Independent Secret-Key Distinguisher for 7-Round AES based on the Joint Generalized Zero-Difference Property
Attacks and cryptanalysis
Hanbeom Shin, Sunyeop Kim, Byoungjin Seok, Deukjo Hong, Jaechul Sung, Seokhie Hong, Sangjin Lee, and Dongjae Lee
Hide abstract
A key-independent secret-key distinguisher identifies structural deviations from an ideal random permutation without discovering any information about the secret key. It is therefore of primary importance for understanding the inherent properties of a block cipher's round function. While numerous key-independent secret-key distinguishers have been proposed for 5- and 6-round AES, none has been proposed for 7-round AES to date. In this paper, we propose the first key-independent secret-key distinguisher for 7-round AES, which exploits solely the structural properties of the round function. We propose the Joint Generalized Zero-Difference Property, where a quartet constructed from related differences satisfies three distinct generalized zero-difference properties simultaneously. By leveraging this joint property, we construct a new 7-round differential characteristic that a right quartet follows with a probability of 
, whereas a random permutation satisfies the same conditions with a probability of 
. Based on this characteristic, we design a distinguishing attack requiring data, time, and memory complexities of 
. Our analysis confirms that the proposed distinguisher achieves a success probability of approximately 77.8%. We experimentally verify the joint property using small-scale AES, confirming that the theoretical predictions match the observed results. This work achieves the longest-round key-independent secret-key distinguisher for AES reported to date.
2026/2065 (PDF)
Last updated:  2026-09-17
Differentially Oblivious Resizing for Group-By Aggregations
Cryptographic protocols
James Bell-Clark, Albert Cheu, Adria Gascon, Jonathan Katz, and Lukas Gerlach
Hide abstract
Systems for private group-by aggregation (e.g., computing histograms, min/max values, or averages) let a confidential virtual machine (CVM) generate statistics from streaming user data. A particular challenge in such systems is ensuring that the CVM's memory-access patterns do not reveal (too much) private information to the untrusted host.
While it is possible to rely on oblivious RAM (ORAM), doing so imposes a significant performance penalty and requires allocating sufficient memory to handle a worst-case data stream.

We introduce ROGA, a scheme for Resizable Oblivious Group-by Aggregation. ROGA uses an extension of oblivious single-access machines and thus improves performance, both asymptotically and concretely, relative to using ORAM. It also incorporates a novel, differentially oblivious resizing mechanism that ensures the allocated memory is within a constant factor of the memory used by a non-oblivious solution. ROGA also parallelizes cleanly across multiple cores for improved performance. 

We implement ROGA in Rust and use Binsec/Rel to verify trace noninterference of its compiled fixed-capacity operations, resize estimator, and fixed-work noise sampler, showing that executions with equal public parameters have identical branch-target and memory-address traces for all secret inputs. The only data-dependent branch is the differentially private resize decision. Compared to state-of-the-art oblivious schemes for confidential analytics (which
do not support private resizing), ROGA is up to 50.4
 faster when sharded across 64 logical cores, and private resizing cuts memory by up to 14
 compared to domain-provisioned instances. In a case study, ROGA using 16 cores processes the standard network statistics of a 277-million-packet backbone trace with 
 end-to-end overhead over a non-oblivious pipeline while exactly matching the reference output.
2026/2063 (PDF)
Last updated:  2026-09-16
Witness Encryption for NP from SNARGs and Groups
Foundations
Zhengzhong Jin
Hide abstract
We construct the first witness encryption for NP in the generic group model from succinct non-interactive arguments (SNARGs) for NP. Our construction applies to any SNARG with subexponential soundness and polylogarithmic online verification time after input preprocessing.

Central to our result is the first Karp-Levin reduction from satisfiability for circuits of size 
 to the minimum-distance problem for linear codes (GapMDP) over a prime field of size 
, with an approximation factor of 
, where 
 is the security parameter. We obtain this reduction by adapting Hair and Sahai's recent hardness result for GapSVP. Combining our reduction with the witness encryption framework due to Barta, Ishai, Ostrovsky, and Wu [CRYPTO 2020], we obtain the first unconditional extractable witness encryption for circuits of size 
 in the generic group model. These techniques may be of independent interest.
2026/237 (PDF)
Last updated:  2026-09-16
Exploiting SNOVA’s Structure in the Wedge Product Attack
Attacks and cryptanalysis
Maxime Bros, Thai Hung Le, Jacob Lichtinger, Brice Minaud, Ray Perlner, Daniel Smith-Tone, and Cristian Valenzuela
Hide abstract
Post-quantum cryptography (PQC) aims to develop cryptographic schemes secure against quantum adversaries. One promising class of digital signature schemes is based on multivariate quadratic equations, where Unbalanced Oil and Vinegar (UOV) is a leading example. UOV has been extensively studied since its introduction by Kipnis, Patarin, and Goubin at Eurocrypt 1999, and it has remained secure. The signature sizes the scheme allows are quite small; however the key sizes are very large, making the scheme less attractive for some common protocols.  To remediate this deficiency, some schemes---such as MAYO, QRUOV, and SNOVA---add a structure to reduce the size of the public key. These multivariate schemes are candidates that made it to the third round of the National Institute of Standards and Technology's Call for Additional Digital Signature Schemes for the Post-Quantum Cryptography Standardization Process. In this work, we revisit a recently proposed algebraic attack by Ran at Eurocrypt 2026 on UOV and extend this approach to a new attack on SNOVA by exploiting its block-ring structure. In addition to improving the attack complexity, the exploitation of the block-ring structure rules out spurious solutions, which prevents the generic version of Ran's attack from applying to SNOVA. This attack breaks 6 of the 11 second-round SNOVA parameter sets and improves on the previous best result for an additional 2 sets; it is significantly more effective against larger 
 in comparison to several earlier attacks. For example, for SNOVA-V with parameters 
, the estimated security drops to 
 bits, compared to 
 bits for the previous best known attack.
2025/1625 (PDF)
Last updated:  2026-09-16
A Practical and Fully Distributed E-Voting Protocol for the Swiss Context
Cryptographic protocols
Véronique Cortier, Alexandre Debant, Henri Devillez, Olivier Esseiva, Pierrick Gaudry, Audhild Høgåsen, and Chiara Spadafora
Hide abstract
Internet voting aims at guaranteeing both vote secrecy and verifiability. Advanced deployed systems, such as in Estonia or Switzerland, put a strong emphasis on cast-as-intended: even when a voter uses a corrupt device, they should be able to check that their intended vote is included in the tally. One approach relies on return codes, written on a voting card. This enables cast-as-intended even when the voter uses a single device that may be corrupted. While convenient for voters, this requires to generate the codes in a secure manner. This approach is deployed in Switzerland but the current system requires a single trusted setup authority: an offline component that is assumed to correctly generate the voting data.

We propose a novel protocol that allows to distribute the trust during the setup. At the heart of our system lies an asymmetric protocol where several parties create the voting material, while allowing one of these parties to remain offline during the voting phase. Usability constraints are taken into account in our design, both in terms of computational complexity (linear setup and tally) and in terms of user experience (we ask the voter to type a high-entropy string only once). Our protocol is therefore a candidate for the next generation e-voting protocol in Switzerland.

We provide an implementation of our protocol, demonstrating its practicability. Moreover, we formally prove vote privacy and verifiability. Our analysis takes into account the presence of short codes that an attacker can guess with non-negligible probability and reveals that making a significant change to the result remains exponentially hard.
2026/1121 (PDF)
Last updated:  2026-09-16
Security Amplification via Robust Indistinguishability Combiners
Foundations
Benny Applebaum, Nir Bitansky, and Nathan Geier
Hide abstract
A robust combiner for a cryptographic primitive 
 takes multiple candidate constructions of 
 and produces a secure construction of 
 provided that sufficiently many of the candidates are secure. A closely related notion is that of a security amplifier, where given a weakly secure construction of 
, we aim to obtain a (strongly) secure one. Intuitively, one may expect that any robust combiner should act as an amplifier by thinking of "good randomness" as inducing secure instances, and of "bad randomness" as inducing insecure instances. Formalizing this intuition, however, has turned out to be challenging. Despite significant progress, general results remain limited and confined either to specific primitives or only to the statistical setting.

We establish a new framework of robust indistinguishability combiners, which greatly extends the class of combiners covered by prior work, and prove that they inherently act as security amplifiers. Our results extend to the computational setting, provided that the combiner makes a single query to each candidate. The new framework allows us to rederive previously known amplification results in a simplified manner, as well as prove new amplification results that have so far been out of reach.

As our main application, we present the first security amplifier for functional encryption, resolving an open question that first arose in constructions of indistinguishability obfuscation, and for which a gap was discovered in previous proofs. Our amplifier transforms a weak scheme with any constant indistinguishability error into one with full negligible security.
2026/2061 (PDF)
Last updated:  2026-09-16
Normal Alignment: Improved Cryptanalytic Sign Recovery on Hard-Label Networks
Secret-key cryptography
Shi Tang, Zirui Chen, Yongjia Su, Zhengchao Gao, Lingyue Qin, and Xiaoyang Dong
Hide abstract
At EUROCRYPT 2025, Carlini {\em et al.} proposed a breakthrough in the cryptanalytic extraction on hard‑label (S1) deep neural networks (DNNs), demonstrating polynomial-time signature and sign recovery. However,  Carlini {\em et al.}'s sign‑recovery method ({which we call \em Future Toggle}) suffers  only a marginal advantage over random guessing, producing high‑confidence wrong sign predictions in deeper layers. Such errors trigger expensive exponential‑time enumeration. 

This work presents {\em Normal Alignment}, a novel statistical sign‑recovery approach for S1 DNNs. Drawing on the expected length difference between projected normals of adjacent decision facets at dual points, our method infers neuron signs via normal‑signature alignment. It delivers higher voting accuracy and pushes erroneous predictions to low‑confidence ranks, which further enables a more efficient combined method, {\em eSOE + Alignment}, by combining {\em Normal Alignment} with the hard‑label {SOE} extension. This combined strategy removes heavy enumeration overhead and realizes exact polynomial‑time full sign recovery. 

Experiments demonstrate the effectiveness of our method,  especially for deep layers. For example, with our method, the signs for CIFAR-10 (architecture 192-64
8-10) and MNIST (architecture 64-96
3-32-10) models can be fully recovered in polynomial time; in contrast, Carlini {\em et al.}'s sign‑recovery method would require exponential‑time enumerations involving 
 or 
 guesses of the signs, respectively.
2026/2060 (PDF)
Last updated:  2026-09-16
DelegProof: A Machine-Checked Security Analysis of EIP-7702 Delegation
Applications
Rong Qian, Yu Cheng, Lingyu Gao, Yuchang Zhang, and Zengli Guo
Hide abstract
EIP-7702, live on Ethereum since the Pectra upgrade, lets externally owned accounts delegate their execution to arbitrary contract code with a single signature. The consequences are measurable: 63% of observed delegations point to malicious contracts, with $2.36M in confirmed losses, and ecosystem guidance already warns about cross-chain replay and front-run initialization. What is missing is a formal account of the problem: the 7702 delegation lifecycle has no formal treatment, and none of the recommended mitigations has been machine-verified. We present DelegProof, to our knowledge the first symbolic formal analysis of EIP-7702 authorization semantics composed with the ERC-4337 EntryPoint pipeline. Our Tamarin models are calibrated by reproducing four documented attack classes, and yield ten machine-checked attack patterns—among them the silent failure of the advertised temporary-delegation bundle, storage confusion across re-delegation, and an ERC-1271 substitution attack that reaches into the signature checks of relying contracts—together with the security invariants that do hold. Formalizing the recommended mitigations shows that banning chainId-0 authorizations eliminates cross-chain replay, that an account-bound initialization gate restores init authorization while an unbound gate is provably still bypassable, and that namespaced storage slots make the storage-confusion class structurally inexpressible. All models and proofs are available at https://anonymous.4open.science/r/delegproof-artifact-F7FE/.
2026/1098 (PDF)
Last updated:  2026-09-16
A gentle introduction to lattice-based cryptography
Public-key cryptography
Alfred Menezes
Hide abstract
We present four quantum-safe schemes: the Kyber (ML-KEM) and FrodoKEM key encapsulation mechanisms, and the Dilithium (ML-DSA) and Falcon (FN-DSA) signature schemes. We also develop the mathematical background on lattices needed to understand why Kyber, FrodoKEM, Dilithium and Falcon are regarded as lattice-based cryptosystems, and we provide insight into the computational hardness of the underlying lattice problems. The exposition is intended to be accessible to senior undergraduate students and beginning graduate students.
2026/613 (PDF)
Last updated:  2026-09-16
Haechi: Simple Commitment-based Keyless In-person Verifiable Elections
Cryptographic protocols
Jiwon Kim, Michael Naehrig, Bharath Namboothiry, Olivier Pereira, and Josh Benaloh
Hide abstract
Verifiable election systems allow voters to verify that their secret ballots were accurately recorded and counted, while enabling anyone to verify the announced tallies. However, two practical obstacles stand between this guarantee and actual verification. First, public election records can be prohibitively large: for an election with one million ballots, deployed systems may publish more than 100GB of cryp-
tographic data. Second, most existing systems protect ballot privacy through encryption, introducing key management—usually one of the most cumbersome and error-prone components of election administration.

We present Haechi, a keyless verifiable election scheme for in-person voting, based on cryptographic vector commitments rather than encryption. At the core of Haechi is a new zero-knowledge proof of ballot well-formedness based on Compressed Σ-protocol theory. Our proof scheme proves the validity of an entire ballot using only a logarithmic number of group elements in the number of the ballot options. This allows all of a voter’s selections to be encoded into a single vector commitment.

With this approach, Haechi reduces the data required for public verification by more than an order of magnitude compared to prior in-person verifiable election systems. Haechi thereby makes verifications for large-scale election considerably more practical. We also give a post-quantum instantiation of Haechi based on lattice commitments, which preserves the same keyless workflow at the cost of larger election records.
2026/2059 (PDF)
Last updated:  2026-09-16
On Aborts in Differential Privacy
Foundations
Fredrik Meisingseth
Hide abstract
There is a growing literature on differential privacy (DP) in multiparty protocols, enabling use in distributed settings and reduced trust assumptions. A largely overlooked aspect of such protocols is the extent to which an adversary can undermine the DP guarantees by making the protocol abort. Another is the DP properties of the honest parties' outputs. We initiate the study of both these issues.

        First, we propose an idealized model of aborts, where an aborter is leaked some information about a DP mechanism execution and then chooses whether or not to let an independent malicious analyst learn the mechanism output. We bound the effects of aborts according to what information (the database, randomness, output, or any combination of these) is leaked. These bounds are not only applicable to multiparty DP via general-purpose multiparty computation (MPC) but may also be used in the analysis of other methods to certify DP properties of a protocol. 
        
        Second, we apply these bounds to analyze ideal protocol executions in MPC. We show, for the first time, that the ideal execution with fairness gives strictly stronger DP guarantees than that for security-with-abort, for the outputs to the honest parties. Further, solitary-output functionalities satisfy guarantees similar to those of the execution with fairness. 

        Finally, we analyze multiparty protocols with respect to the way in which they realize the respective ideal functionalities. We show that partially fair and fully fair protocols have very similar DP guarantees. Since partial fairness, as opposed to full fairness, can be achieved in dishonest-majority settings, and in particular the two-party setting, this opens up for enhanced DP guarantees in two-party computation.
2026/867 (PDF)
Last updated:  2026-09-16
On the (Privacy) Harms of the European Digital Identity Framework
Applications
Christian Knabenhans, Shannon Veitch, Mathilde Raynal, Theresa Stadler, Sylvain Chatel, Wouter Lueks, and Carmela Troncoso
Hide abstract
As digital identity frameworks (DIFs) gain traction around the world, many see privacy-enhancing technologies (PETs) as the key to prevent their potential negative societal impacts such as discrimination or surveillance. We critically examine whether PETs can achieve this goal using the European Digital Identity Framework (EUDIF) as an example. We develop a harm analysis methodology based on harm trees that illustrates how information leakage, certain design decisions, or a DIF's deployment context lead to harms. We leverage techniques from cryptographic modeling to formally capture the leakage of the core functionality of the EUDIF and its proposed applications. Our harm modeling elucidates which pathways to harm are inherent to the EUDIF's core functionality, and which pathways can be mitigated through PETs. Our analysis shows that, while PETs can reduce information flows, they fall short in actually mitigating the harms that deploying digital identity can bring to individuals and society.
2026/040 (PDF)
Last updated:  2026-09-16
Polynomial Evaluation on Many Inputs with Bounded Hamming Weight over GF(2)
Foundations
Fukang Liu, Vaibhav Dixit, Muzhou Li, Daisuke Yamamoto, Wakaha Ogata, Santanu Sarkar, and Willi Meier
Hide abstract
We propose a new polynomial evaluation algorithm to evaluate a degree-
 Boolean polynomial 
 on all elements in some structured sets 
. This problem has been well-studied for 
 and there are efficient polynomial evaluation algorithms like standard Mobius transform, memory-efficient Mobius transform (EUROCRYPT 2021, TOMS 2024) and fast exhaustive search (CHES 2010, PQCrypt 2023) for the case 
. In addition, the standard Mobius transform can also be used to evaluate a polynomial over the set 
 of  all 
 whose Hamming weight is upper bounded by 
, and its memory complexity is the same as the size of 
. In Dinur's algorithm for polynomial method proposed at EUROCRYPT 2021, there is a critical step to efficiently evaluate 
 over a more general input set 
. To our knowledge, in the literature, no memory-efficient polynomial evaluation algorithms are designed for such a special input set. Dinur proposed a tweaked fast exhaustive search algorithm for this problem, but it has not been proved nor implemented. This leaves us with a natural question: can we design an efficient polynomial evaluation algorithm tailored for a more general input set 
 where 
 and 
? We answer this question by proposing a new polynomial evaluation algorithm named \textbf{FESG}, i.e., \textbf{F}ast \textbf{E}xhaustive \textbf{S}earch over a more \textbf{G}eneral input set, to handle the input set 
. This algorithm is based on the derivative-based polynomial evaluation framework proposed at CHES 2010. In addition to extending the application of derivative-based framework to a more general input set, we also successfully address a major issue in existing derivative-based algorithms over 
 proposed at CHES 2010 and PQCrypt 2023, reducing the time complexity of the initialization phase from 
 to 
. As a result, FESG is also efficient for large 
. Our algorithm is almost optimal, whose theoretic time and memory complexity are  
 bit operations and 
 bits, respectively. Here, 
 is the size of the set 
. An efficient implementation of the FESG algorithm for any 
 is also given in this work. In particular, the FESG algorithm also provides a proven method for a critical step in Dinur's algorithm for polynomial method without affecting its overall complexity.
2026/1999 (PDF)
Last updated:  2026-09-16
Revisiting Single-Color Initial Structures in Meet-In-The-Middle Attacks
Secret-key cryptography
Shiyao Chen, Jian Guo, Wenjie Nan, Danping Shi, and Tianyu Zhang
Hide abstract
The meet-in-the-middle (MITM) attack framework is one of the most powerful cryptanalytic techniques with broad influence to preimage, key recovery, and collision attacks. In this paper, we present two generic techniques. First, we observe that the constant space in MITM attacks is an exploitable source of degrees of freedom: its value-independence enables MITM-style partition and acceleration of attack subprocesses. With this intuition in mind, we revisit the single-color initial structure technique by Chen et al. at Asiacrypt 2025, and find an improved algorithm that relaxes the canonical optimization objective in automatic search for MITM attacks. Second, we extend the partial-target-preimage-to-collision conversion by Li, Isobe, and Shibutani at FSE 2012 used in MITM-based collision attacks  to the settings of chosen-prefix collision and diamond structure construction. We demonstrate the practical relevance of our techniques by presenting a number of improved results in (pseudo-)preimage, (chosen-prefix) collision, and herding attacks on AES-like constructions over the state-of-the-art.
2026/1861 (PDF)
Last updated:  2026-09-16
Anonymous Attribute-Based Signcryption: Definitions, Constructions, and Applications
Cryptographic protocols
Yongkang Lang, Fangguo Zhang, Zhiyuan An, Xinyi Huang, and Xiaofeng Chen
Hide abstract
We put forward a generalization of attribute-based signcryption, called anonymous attribute-based signcryption (A
BSC). Beyond message confidentiality and ciphertext unforgeability, A
BSC further requires \textit{ciphertext anonymity}: no information about the signcryptor's attributes or ciphertext-related attributes/policies is leaked, regardless of the decryption outcome. Specifically, we begin by establishing the syntax and security notions for A
BSC within a \textit{unified} framework, which encompasses various variants (key-policy, ciphertext-policy, dual-policy, and a hierarchical dual-policy variant called Special A
BSC). Then, we construct a Special A
BSC scheme for \textit{general policies} (modeled as bounded-depth Boolean circuits) from the succinct learning with errors and the basis-augmented short integer solution assumptions in the standard model, hence achieving post-quantum security. This naturally yields lattice-based instantiations of both ciphertext-policy and dual-policy A
BSC.

Beyond its independent interest, we also show the expressiveness and generality of our A
BSC by exploring its application to matchmaking encryption (ME) and arranged matchmaking encryption (AME) proposed by Ateniese et al. (Crypto '19). As a byproduct, we give generic constructions of both ME and AME for \textit{arbitrary policies} against unbounded collusions, and strengthen the CPA-privacy of (A)ME to achieve CCA security. The latter is achieved for free in our construction, as A
BSC natively provides CCA security. Overall, our new solution enhances the diversity of approaches for building the advanced primitive (A)ME.
2026/1238 (PDF)
Last updated:  2026-09-16
On the Additive Sensitivity of LZ77 Under Consecutive Edits
Foundations
Jeremiah Blocki and Seunghoon Lee
Hide abstract
We revisit the problem of mitigating information leakage in the widely used but insecure compress-then-encrypt paradigm. While encryption hides message contents, the ciphertext length is directly related to the length of the compressed message, which may, in turn, leak information about the {\em content} of the message itself. Recent work of Blocki et al. (TCC~2025) proposed an 
-differentially private approach that adds randomized padding calibrated to the global sensitivity of the compression algorithm, and showed that LZ77 has global sensitivity 
 for input length 
 and sliding window size 
. 

However, prior analysis focused only on sensitivity with respect to single-character edits, which leads to limited privacy guarantees when protecting longer substrings such as passwords, passphrases, cookies, or confidential user records.  A natural attempt to handle longer secrets is to appeal to group privacy, but for approximate differential privacy, this leads to very poor parameter degradation: in particular, the effective value of 
 can grow exponentially with the group size 
.  In this work, we introduce and study the sensitivity of compression schemes under block edits. Specifically, we define two strings to be 
-neighbors if they differ only within a contiguous interval of length 
. 

Our main technical contribution is a nearly tight characterization of the 
-consecutive sensitivity of LZ77. We show that the 
-consecutive sensitivity of LZ77, both with and without self-referencing, is at most 
. In particular, when 
, the bound simplifies to 
, matching the known bound for single-character edits. Combined with the framework of Blocki et al., our bound yields 
-differential privacy for 
-neighbors with the same asymptotic padding scale as for single-character edits. We provide matching lower bounds to demonstrate that our upper bound is tight, e.g., when 
, the 
-consecutive sensitivity of LZ77 is at least 
, matching the 
 term from our upper bound up to a logarithmic factor.
2026/975 (PDF)
Last updated:  2026-09-16
Functional Bootstrapping for a Single LWE Ciphertext with 
 Polynomial Multiplications
Public-key cryptography
Xiaopeng Zheng, Hongbo Li, and Dingkang Wang
Hide abstract
Bootstrapping is the key technique that turns leveled homomorphic encryptionc into fully homomorphic encryption, but it remains a major efficiency bottleneck. Recent work by Z. Liu and Y. Wang (ASIACRYPT 2023) showed how to bootstrap 
 LWE ciphertexts with total cost of 
 polynomial multiplications based on the BFV scheme. However, their results achieve 
 complexity only through amortization over large batches, and do not give a genuine non-amortized 
 bound for a single ciphertext.
In this paper, we present a BFV-based functional bootstrapping algorithm for arbitrary functions over large plaintext spaces with total cost of 
 polynomial multiplications for one LWE ciphertext. The same construction also supports small and moderate batches, and processes a batch of 
 ciphertexts with total cost 
 in the supported parameter range. The main technical ingredient is a sparse-packing polynomial-evaluation method for BFV ciphertexts, which exploits the duplicated-slot structure to evaluate an arbitrary polynomial on 
 encrypted inputs with total cost of 
.
We implement the scheme in Lattigo using the BFV scheme. At 128 bit security and on a single thread, bootstrapping an arbitrary function takes 3.15 seconds for one ciphertext encrypting a 9-bit plaintext and 3.77 seconds for 128 such ciphertexts in one batched invocation. For 16-bit plaintexts, it takes 10.63 seconds for one ciphertext and 18.07 seconds for 16 ciphertexts. These results show that non-amortized single-ciphertext functional bootstrapping, as well as small and moderate batch bootstrapping, can be practical for arbitrary functions over relatively large plaintext spaces.
2025/2085 (PDF)
Last updated:  2026-09-16
Strong Pseudorandom Functions in 
 in the Bounded-Query Setting
Secret-key cryptography
Marshall Ball, Clément Ducros, Saroja Erabelli, Lisa Kohl, Nicolas Resch, and Peter Scholl
Hide abstract
Understanding the minimal computational power needed to realize a pseudorandom function (PRF) is a long-standing question in cryptography. By the Razborov–Smolensky polynomial approximation method, it is known that 
 cannot support strong pseudorandom functions with subexponential security, since any such function can be distinguished from random with quasipolynomially many samples.
     
    In this work, we initiate the study of low-complexity strong PRFs under a refined framework that separates adversary query complexity from running time and distinguishing advantage, and observe that distinguishing algorithms for 
 do not apply if the number of queries is below the threshold implied by the Razborov–Smolensky approximation bound. In this setting, we prove that strong PRFs cannot exist in the smaller class 
, showing that every such function family can be efficiently distinguished from random using polynomially many queries. 
    Together with the fact that functions in 
 are conjectured to have heavy Fourier coefficients (ITCS 2014), this identifies 
 as the minimal candidate class for realizing strong PRFs in the bounded-query setting. 

We propose the first candidate strong PRF in 
, which we conjecture to offer subexponential security against adversaries limited to a fixed quasipolynomial number of queries. Furthermore, our candidate  is in the class 
, arguably the minimal class given the above barriers. 
  We show that our candidate lacks heavy Fourier coefficients, resists many settings of linear attacks, is non-sparse over 
 with high probability, has high rational degree, is inapproximable by low-degree polynomials over 
, and has low correlation with fixed function 