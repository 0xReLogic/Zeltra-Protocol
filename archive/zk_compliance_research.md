# ZK-Compliance & EIP-2537 Precompiles: Research & Implementation Plan

Dokumen ini menganalisis kerentanan kritis pada implementasi ZK-Compliance awal di smart contract, menyajikan temuan riset terbaru 2026 mengenai standardisasi Privacy Pools, serta menyusun rencana refaktorisasi verifikator Groth16 menggunakan precompile EIP-2537 BLS12-381.

---

## 1. Temuan Utama (Riset Jurnal & Benchmark 2026)

### A. Kerentanan Kebocoran Privasi (Privacy Leak)
Pada kode awal, fungsi `verify_compliance` menerima parameter `leaf` dan `proof_bytes` secara plaintext dan melakukan verifikasi Merkle proof secara on-chain:
```rust
let is_member = self.verify_merkle_proof(leaf, proof_bytes, root)?;
```
**Masalah**: Karena transaksi dikirim ke blockchain, leaf (deposit commitment) dan proof_bytes terlihat secara publik di mempool dan block explorer. Hal ini **menghilangkan 100% anonimitas pengguna** karena pengamat luar dapat langsung mencocokkan transaksi deposit dengan transaksi spend/withdrawal.

**Solusi Standar 2026 (Privacy Pools & 0xBow)**: 
* `leaf` dan `merkle_proof` harus diperlakukan sebagai **Private Witness (Secret Input)** di dalam sirkuit ZK.
* Kontrak pintar on-chain hanya menerima `root` (Merkle root dari clean association set) sebagai **Public Input**.
* ZK proof Groth16/Plonk membuktikan secara matematis bahwa *"Prover mengetahui leaf dan jalur Merkle valid menuju root"* tanpa membocorkan leaf tersebut.

### B. Kerentanan Proof Spoofing (Public Input Binding)
Pada kode awal, fungsi `verify_groth16_proof` menerima parameter `public_inputs_g1_bytes` (G1 point `IC`) yang sudah dihitung sebelumnya oleh pemanggil off-chain:
```rust
public_inputs_g1_bytes: Vec<u8> // IC linear combination
```
**Masalah**: Pemanggil jahat dapat mengirimkan bukti ZK yang sah untuk *public inputs* yang berbeda, lalu memalsukan G1 point `IC` agar lolos verifikasi pairing check. Kontrak tidak mengikat (*bind*) parameter on-chain (seperti `root` dan `nullifier`) ke dalam verifikasi.

**Solusi**: Kontrak pintar harus menghitung kombinasi linear G1 point `IC` secara on-chain menggunakan precompile `BLS12_G1ADD` (`0x0b`) dan `BLS12_G1MSM` (`0x0c`) dari parameter asli yang dikirim:
$$\text{IC} = \text{vk.IC}[0] + \sum (\text{public\_input}[i] \cdot \text{vk.IC}[i+1])$$

### C. Status Precompile EIP-2537 (Arbitrum L2)
* ArbOS 50 "Dia" (rilis 2025/2026) secara resmi mengaktifkan precompile EIP-2537 untuk kurva BLS12-381 di Arbitrum One, Nova, dan Sepolia testnet.
* Alamat Precompile EIP-2537:
  * `0x0b`: `BLS12_G1ADD` (Point Addition G1)
  * `0x0c`: `BLS12_G1MSM` (Multi-Scalar Multiplication G1)
  * `0x0d`: `BLS12_G2ADD` (Point Addition G2)
  * `0x0e`: `BLS12_G2MSM` (Multi-Scalar Multiplication G2)
  * `0x0f`: `BLS12_PAIRING_CHECK` (Pairing Check)

---

## 2. Desain Sirkuit & Parameter Verifikasi

Kita mendefinisikan 4 Public Inputs untuk sirkuit ZK-Compliance:
1. `association_root`: Merkle root dari set deposit bersih (clean set).
2. `nullifier`: Penanda anti-double-spend dari coin.
3. `recipient`: Address tujuan pengiriman dana (mengikat spending dengan alamat penerima untuk mencegah front-running/relayer hijack).
4. `amount`: Nominal transaksi spend.

### Formulasi Kombinasi Linear G1 `IC` di Rust Stylus:
```rust
// Formula: IC = vk.IC[0] + (root * vk.IC[1]) + (nullifier * vk.IC[2]) + (recipient * vk.IC[3]) + (amount * vk.IC[4])
```
Operasi scalar multiplication ($s \cdot P$) dan point addition ($P + Q$) dilakukan secara efisien dengan memanggil precompile `0x0c` (G1 MSM) untuk multi-scalar-multiplication secara batch.

---

## 3. Rencana Refaktorisasi Kode

### Langkah 1: Modifikasi `verify_compliance` di [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
Kita akan menghapus verifikasi Merkle proof on-chain dan memodifikasi parameter `verify_compliance` untuk hanya menerima `root`, `nullifier`, `recipient`, dan `amount` sebagai public inputs, serta memicu verifikasi Groth16 sesungguhnya yang mengikat input tersebut.

### Langkah 2: Implementasi Fungsi `compute_public_inputs_g1`
Menyusun payload untuk melakukan operasi MSM batch di G1 menggunakan precompile `0x0c`.

### Langkah 3: Update Mock SDK di [nimbus-sdk/src/lib.rs](file:///home/azureuser/crypto/nimbus-sdk/src/lib.rs)
Menyesuaikan struktur output prover lokal agar selaras dengan input public yang baru.

---

## 4. Draf Kode Verifikasi On-Chain (Stylus Rust)

```rust
const BLS12_G1_MSM: Address = address!("000000000000000000000000000000000000000c");
const BLS12_G1_ADD: Address = address!("000000000000000000000000000000000000000b");

/// Menghitung linear kombinasi dari public inputs secara on-chain menggunakan precompile G1 MSM.
pub fn compute_public_inputs_g1(
    vk_ic: &Vec<[u8; 128]>, // vk.IC dari setup sirkuit (public parameter)
    public_inputs: &Vec<[u8; 32]>, // [root, nullifier, recipient, amount]
) -> Result<[u8; 128], Vec<u8>> {
    if vk_ic.len() != public_inputs.len() + 1 {
        return Err(b"VK_IC_LENGTH_MISMATCH".to_vec());
    }
    
    // Siapkan input untuk BLS12_G1_MSM (0x0c)
    // Format: (G1_point_1, scalar_1) || (G1_point_2, scalar_2) || ...
    // G1_point = 128 bytes, scalar = 32 bytes. Total = 160 bytes per term.
    let mut msm_input = Vec::with_capacity(public_inputs.len() * 160);
    for i in 0..public_inputs.len() {
        msm_input.extend_from_slice(&vk_ic[i + 1]);
        msm_input.extend_from_slice(&public_inputs[i]);
    }
    
    // Panggil precompile G1 MSM
    let msm_result = unsafe {
        RawCall::new_static()
            .call(BLS12_G1_MSM, &msm_input)
    }.map_err(|_| b"G1_MSM_PRECOMPILE_FAILED".to_vec())?;
    
    // Tambahkan vk_ic[0] (base offset) ke hasil MSM menggunakan BLS12_G1_ADD (0x0b)
    // Format: G1_point_1 (128 bytes) || G1_point_2 (128 bytes)
    let mut add_input = Vec::with_capacity(256);
    add_input.extend_from_slice(&vk_ic[0]);
    add_input.extend_from_slice(&msm_result);
    
    let final_ic = unsafe {
        RawCall::new_static()
            .call(BLS12_G1_ADD, &add_input)
    }.map_err(|_| b"G1_ADD_PRECOMPILE_FAILED".to_vec())?;
    
    let mut out = [0u8; 128];
    out.copy_from_slice(&final_ic);
    Ok(out)
}
```
