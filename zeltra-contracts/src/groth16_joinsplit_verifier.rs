//! Groth16 Verifier for Universal 2-in-2-out JoinSplitCircuit (BLS12-381, 19 Public Inputs)
//!
//! Implements on-chain verification using EIP-2537 precompiles per DEC-036C:
//! - 0x0c (BLS12_G1_MSM): multi-scalar multiplication for 20 IC points (3,200 bytes)
//! - 0x0f (BLS12_PAIRING_CHECK): 4-pairing equation check (1,536 bytes)

use alloc::vec::Vec;
use stylus_sdk::abi::Bytes;
use stylus_sdk::call::RawCall;

use crate::constants::{BLS12_G1_MSM, BLS12_PAIRING_CHECK};
use crate::storage::Zeltra;

// ─── JoinSplitCircuit Trusted Setup VK Constants ────────────────────────────
// Circuit: Universal JoinSplitCircuit (19 public inputs, DEC-036A / DEC-036C) over BLS12-381
// Seed: JOINSPLIT_CIRCUIT_SETUP_SEED in zeltra-core

pub const JOINSPLIT_VK_ALPHA_G1: [u8; 128] = alloy_primitives::hex!(
    "0000000000000000000000000000000009658e0abc25967aa646473c13b1a76777acaddc4e6c100077a568e5aa0660911ba11cd7c825c6ecfc3730156af584bd000000000000000000000000000000000a334c6f9cfbd069169abb2113f6281d10bf75f4adf47b80e7f070c2c84fb0175f2e559d6e71234b006ec3984b631660"
);

pub const JOINSPLIT_VK_BETA_G2: [u8; 256] = alloy_primitives::hex!(
    "000000000000000000000000000000000684de7aeeb28576d189d1d482ed1a10a9c3cc838ee4b589889432a2c3095542cb0f81a3e68bf24761391410e1917dc20000000000000000000000000000000014e09c81ccc2e2b9cb08747d5c31fc0dacba19cad2b2122ca12c5a488fa2f13d5919a8c3527d1ba354a71562ac2cafa500000000000000000000000000000000151efaae2dff60ef9b7b08cf2c536c4635765764330cbed47d3c964a9350a0edc901c453547830e494a4c20c8d58ca41000000000000000000000000000000000d24cd527c066a920012f5841ca762d3e7e56198557614eab85d2019c47848e7068b71456910bfe5c2d89e7fdd9e6d4e"
);

pub const JOINSPLIT_VK_GAMMA_G2: [u8; 256] = alloy_primitives::hex!(
    "00000000000000000000000000000000016dad079fb5c17ad1250c488d7df574b09b1dc39ab4c8675045ef9dec23af61a54240e1c1820e21f031daa67b67e23e0000000000000000000000000000000001218c7777b636f17e9155a59e816e67c5334f10f1b2e84659107541ad42ccaf9f5a9dac53b1ba6b91350066e29dbd32000000000000000000000000000000000a2545a2758fffd9e046f8481b55b196937b5f61cce3c3d3ddccb8b993d32b6872a320936a1aa8c75e54e49a6ace603600000000000000000000000000000000168bf9a293a4bd0147e282484688ea6d4a90596251c4e9606dd3139783b8c74c5ffb385523afcf8c5c0327c170816dfa"
);

pub const JOINSPLIT_VK_DELTA_G2: [u8; 256] = alloy_primitives::hex!(
    "00000000000000000000000000000000160cfce2005d81b6b95eff3732944bebab4d31e800a080be4ffe9fbc74971c7b32f680d0f15b7f4c54f9f50ac22da50100000000000000000000000000000000149a5686231a1ccb1bc8bbfc84ebf8198500403a8d7b643b9a90f0e1f294ac8c3642451862293ca8ebe106305d8a950700000000000000000000000000000000010188cc78d64ca0b9b563e9bb03f716e0900bd1def668f07617ca74d3c083a19ca11158d16f30966d06e38b62381ea3000000000000000000000000000000000bddb1c381b83cdbebc715166320452b4bb10cece2efa7b0a50957fd981be65d8cbb31654f1482cf268e480649130ae6"
);

pub const JOINSPLIT_VK_IC: [[u8; 128]; 20] = [
    // IC[0]
    alloy_primitives::hex!("00000000000000000000000000000000160b06ec3bc6cbeb112c34249d85e885ad31d39705e820abedee4a02a56e4c2d67e1800699cb57875610d157e40c2ede0000000000000000000000000000000006ff90bb52338181fd408c96e06f4ff45d46409931cfdc72bb72594f27e6d562622d2fd2e7f506516a0f20c141a202b5"),
    // IC[1] - note_root
    alloy_primitives::hex!("00000000000000000000000000000000021499dc6d0c040f135f4264f97675c79ce4fe5ae72b1f42c350dcb64367ecd31128a64560ffb761f0b7175386fd39370000000000000000000000000000000001f41f41020174824cf3f7840be3aaeec57ec6c394c4dfa7331be6d63343acf3266424760a915e21d04f1853eed89b54"),
    // IC[2] - leaf_count
    alloy_primitives::hex!("000000000000000000000000000000000075df22f815d3a5a3c237d4a6e08d0adcec7a1fa5fa974c80655ab5311b3549d456b9c47cb5bf08e531e13ba681519d0000000000000000000000000000000010e58c7cba459f9186eaf47091910beb4a777c3a0c10152d0fa35466b992b282faab972c9b3d5e05585dbf021da6371c"),
    // IC[3] - input_nullifier_1
    alloy_primitives::hex!("00000000000000000000000000000000084e81de6854a8db2f46efd95a7d6adda8fb863b4d289a11a1435bfa23191afa4fe44f58e6682a79ef16ede8345ff748000000000000000000000000000000000b9a4ced3a4262b860683e986c64986fca5930f7d4fa12bb477822eab76257e128c8509a13cf8759881701b0e0383bb9"),
    // IC[4] - input_nullifier_2
    alloy_primitives::hex!("0000000000000000000000000000000014c8b8d21dad9a5d402f4d7821eafda38409193866c771189214b4a859481425323c2b48a684d5d892f0bf9be819a17c0000000000000000000000000000000016413ec4519645f4ea1cba9b33321059633de1c05dcaed9ab08bfaced96bce8d3d63a7d56f365157df6508c6d567b588"),
    // IC[5] - epoch_id_1
    alloy_primitives::hex!("0000000000000000000000000000000016910cee7dc9022565d22ef4bccbd431cabca558507ee07cb9e6bb5833b85da0e89efe4784e4c0c707ddc5e5becd6d5d0000000000000000000000000000000006233718f64b27c6bc86b71fce98e41cf172c759263979ebb1d03b2953e5746ce2d7b91b00c942a91746b1f7d17ebc3d"),
    // IC[6] - epoch_id_2
    alloy_primitives::hex!("0000000000000000000000000000000006524ef999a9e18cf9ca2c65ff3d78faaa6750e6d3615f5d950bf737af2cc90f73ca57b84d34e38d354883786b057a7f000000000000000000000000000000000ff2231b2ec56bab12b17c8491f864da73ff2eb117b688318b99085b8062f76840e7694204cf758b0c7d4c451ae02a4f"),
    // IC[7] - output_commitment_1
    alloy_primitives::hex!("0000000000000000000000000000000016593c3d5bba5b080ee4a3cd0d5e375255174f70c1f2fead3e53f5d6418030ac0809c0c139f9f344ce590ff30ac3816300000000000000000000000000000000023088ed6cd2e19dc3dba77600e2dadec738e4f748c38a8ae87b0b53a64fdd01996f115cbddaa13e8e679ccca3d6f9c2"),
    // IC[8] - output_commitment_2
    alloy_primitives::hex!("000000000000000000000000000000000f0daf2b24bd4cb87a2610d87b5ae52c8c00cb8a478d2d31fc90da3cbcc7934d43e308c9bbd37b53ebb414c644c530730000000000000000000000000000000001297a9a382d785444a6f70c5376429fdaae9bd1ea6d12ac89b10ef94aa70a08570d2ea2973127a122b91f55ab61c849"),
    // IC[9] - recipient
    alloy_primitives::hex!("000000000000000000000000000000000eb1653fe2a55ee5e3ea8f859976027b4bff8fa48eb86ada05e359a11d39aa24ea6b51294d937a929968436af3d8495600000000000000000000000000000000134930e0509c6a29fc526eb9498dfb9695b3d77c890b10b4b689fb60bcd7b1446e1ae1a12bfb6f8dc1057c8409c7a50a"),
    // IC[10] - merchant_amount
    alloy_primitives::hex!("00000000000000000000000000000000144ac7619e19bc84de9254fe55474aaf6c2721edd0b580326ee6b8ed39b8d34d75cf2aa1ee1914c643b22689c23f8cf10000000000000000000000000000000016345f59dc5578dd950f72622885dcdf8c07948dc0be85308b609bd3fbcfd07487c3bf44df246c3eb42de5e6b31571f0"),
    // IC[11] - protocol_fee
    alloy_primitives::hex!("00000000000000000000000000000000174470b5dfa425f1fe9844bfd98872dade2dce735c621902e8e0b8b4fd962d2a92c78e2d201441b286394abbb87551aa0000000000000000000000000000000003c99ff19cdf1ec6f08a3efe3746a3d2bd21ed1830a69d9d018c689c5e5115cf195c86618ab1f955177c4d0aeae322d3"),
    // IC[12] - execution_fee
    alloy_primitives::hex!("0000000000000000000000000000000004f0a03021994eb805671759700c2b04ad6672d69bd2a8724914f104fb4dfcfeb60447d7cffcd1e2704d9db11646fa9c00000000000000000000000000000000049134dcba86e1c6f9561d3056275ba7cd19293cdc2f0d17b3c496707426eb9094ca84dc7608a86ec90f70c9b46a50cc"),
    // IC[13] - quote_hash_hi
    alloy_primitives::hex!("000000000000000000000000000000000422fc0777f3cf8188f10e1341feecd2dc78b68e7691e2d6b726cbcdb3d3314dca30cc7508606066e1e0620ecc6dba0f0000000000000000000000000000000012d69d990ff0b01ccdfcfceddf35169ae1a669d92710cf0a381b7bae62e932fd7650e44d3d5402182a99484d35c2b98b"),
    // IC[14] - quote_hash_lo
    alloy_primitives::hex!("000000000000000000000000000000000f914217f401f03041f2993e4a4eb9e44af531b40e091585d13fe69ccd78e49c532b9f937fa8c2359cf1c9b86960ae8e000000000000000000000000000000000e6a56f418cdf1cdb07cc2c30c4986de51fae6f33da0ea608c4949333a90ff4d04b07328352bd5864458ceeab0f8b796"),
    // IC[15] - chain_id
    alloy_primitives::hex!("0000000000000000000000000000000005a899bfe2b1dff3a167883e36d33503bb34897ca3cf270e2ee23093455d1982af2c4d656cc9b20cc70fe78940e4136500000000000000000000000000000000140327e6d737c7fa4fca460105a043a4be33897b038b29ed5c808726d1e37f5253e7da287357c8b2b20500b313f98d28"),
    // IC[16] - contract_address
    alloy_primitives::hex!("0000000000000000000000000000000000f6665ffc5bcae23beffe476119ffa315d0a9588a5e352d67dd77c99debf1450cc76e676023cef3bdc5ef5da9f749f9000000000000000000000000000000000888946ee0cc0ed7d292dacd6f8e51cb15e4231fd61ae65676e90a4029d474d2015cca0647929ea2c9e7f2833e6e2de2"),
    // IC[17] - expiry
    alloy_primitives::hex!("00000000000000000000000000000000133fab15f96723051ead9ee26e30f25624383d27a301bef850058422d96abfea571648776aa69bd6b6b0dd486fbcd06e0000000000000000000000000000000008d7600d3cd0028ff4093c7d9a398343deda3096ea92bb01fc8e6bbee845dcf659f4ae08e288e0d7f1b97770d0f7dfc1"),
    // IC[18] - flags_packed
    alloy_primitives::hex!("00000000000000000000000000000000039750d133b8c2a7c8d3554aaaafd1e58eefe8fa1e5a3c037223d8a3399bcd36cca397d89b8b9a3747d8c2081d4982fb000000000000000000000000000000000b0904a70a299e5ac455805a42bd99fe8d0321d2098e707f961033d6c7d53d84c5e6be704388a21dc04a7890e870ec68"),
    // IC[19] - is_rollover
    alloy_primitives::hex!("0000000000000000000000000000000019cf52962138d41936ec58382f34fa5077f09d727d67acbd13e3a1e090a9cc22f05059328cc5457b3e0d2a079646a4a5000000000000000000000000000000000b4c8169e7bbbf0eb84e71463d94b9bc5f9c5293e5b7311d396b7030e027fc2d20db7594e0261e056c8f9fe2730f8455"),
];

/// Computes the public inputs linear combination point L = IC_0 + sum_{i=0..18} x_i * IC_{i+1} in G1.
///
/// Implements C6.1: Encodes 20 points (IC_0 through IC_19) with 20 scalars into 3,200 bytes.
/// - Element 0: (IC_0, 1) where scalar is 1 left-padded to 32 bytes.
/// - Elements 1..19: (IC_{i+1}, public_inputs[i]).
///
/// Executes staticcall to EIP-2537 precompile 0x0c (BLS12_G1_MSM) directly.
pub fn compute_joinsplit_public_inputs_g1(
    public_inputs: &[[u8; 32]; 19],
) -> Result<[u8; 128], Vec<u8>> {
    // 20 points * 160 bytes (128 point + 32 scalar) = 3,200 bytes
    let mut msm_input = Vec::with_capacity(20 * 160);

    // Point 0: IC[0] with scalar 1
    msm_input.extend_from_slice(&JOINSPLIT_VK_IC[0]);
    let mut scalar_one = [0u8; 32];
    scalar_one[31] = 1;
    msm_input.extend_from_slice(&scalar_one);

    // Points 1..=19: IC[i + 1] with public_inputs[i]
    for i in 0..19 {
        msm_input.extend_from_slice(&JOINSPLIT_VK_IC[i + 1]);
        msm_input.extend_from_slice(&public_inputs[i]);
    }

    let host = Zeltra::runtime_host();
    let msm_result = unsafe { RawCall::new_static(&host).call(BLS12_G1_MSM, &msm_input) }
        .map_err(|_| b"G1_MSM_PRECOMPILE_FAILED".to_vec())?;

    if msm_result.len() != 128 {
        return Err(b"INVALID_MSM_RESULT_LENGTH".to_vec());
    }

    let mut out = [0u8; 128];
    out.copy_from_slice(&msm_result);
    Ok(out)
}

/// Verifies a Groth16 proof for JoinSplitCircuit on-chain using EIP-2537 pairing check (0x0f).
///
/// Implements C6.2 & C6.3:
/// Equation: e(-A, B) * e(alpha, beta) * e(L, gamma) * e(C, delta) == 1
/// Buffer: 4 pairs * 384 bytes = 1,536 bytes.
/// Asserts: success == true, returndatasize == 32, and result equals 0x00..01.
pub fn verify_joinsplit_groth16(
    proof_a_neg: &Bytes,
    proof_b: &Bytes,
    proof_c: &Bytes,
    public_inputs_g1: &[u8; 128],
) -> Result<bool, Vec<u8>> {
    if proof_a_neg.len() != 128 || proof_b.len() != 256 || proof_c.len() != 128 {
        return Err(b"INVALID_PROOF_LENGTHS".to_vec());
    }

    // DEC-052: Strict pre-pairing curve point sanitization gates
    crate::types::sanitize_g1_point(proof_a_neg.as_ref())?;
    crate::types::sanitize_g2_point(proof_b.as_ref())?;
    crate::types::sanitize_g1_point(proof_c.as_ref())?;
    crate::types::sanitize_g1_point(public_inputs_g1.as_ref())?;

    // Construct 1536-byte input for BLS12_PAIRING_CHECK (4 pairs)
    let mut input = Vec::with_capacity(1536);
    // Pair 1: -A (G1, 128B) and B (G2, 256B)
    input.extend_from_slice(proof_a_neg);
    input.extend_from_slice(proof_b);
    // Pair 2: alpha (G1, 128B) and beta (G2, 256B)
    input.extend_from_slice(&JOINSPLIT_VK_ALPHA_G1);
    input.extend_from_slice(&JOINSPLIT_VK_BETA_G2);
    // Pair 3: L (G1, 128B) and gamma (G2, 256B)
    input.extend_from_slice(public_inputs_g1);
    input.extend_from_slice(&JOINSPLIT_VK_GAMMA_G2);
    // Pair 4: C (G1, 128B) and delta (G2, 256B)
    input.extend_from_slice(proof_c);
    input.extend_from_slice(&JOINSPLIT_VK_DELTA_G2);

    let host = Zeltra::runtime_host();
    let output = unsafe {
        RawCall::new_static(&host)
            .limit_return_data(0, 32)
            .call(BLS12_PAIRING_CHECK, &input)
    }
    .map_err(|_| b"ZK_PAIRING_PRECOMPILE_CALL_FAILED".to_vec())?;

    // C6.3: Assert returndatasize == 32 and output equals 0x00..01
    if output.len() != 32 || output[..31].iter().any(|b| *b != 0) {
        return Err(b"INVALID_PAIRING_OUTPUT".to_vec());
    }

    Ok(output[31] == 1)
}
