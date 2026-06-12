# Panduan Deployment Key Management System (KMS) & Node Relayer

Dokumen ini berisi panduan praktis cara melakukan deployment node Leader dan Guardian di server VPS menggunakan enkripsi database SQLCipher dan jaringan aman privat Tailscale.

---

## 1. Panduan Jaringan & Keamanan (Wajib Menggunakan Tailscale VPN)

Untuk keamanan jaringan relayer:
*   **Jangan buka port HTTP node (8080-8084) ke publik/internet** (`0.0.0.0`) karena bisa diserang secara langsung.
*   **Gunakan Tailscale** (mesh VPN privat):
    1.  Install Tailscale pada semua mesin VM/VPS.
    2.  Setting environment variable `NIMBUS_BIND_ADDR` agar hanya me-listen ke IP Tailscale VPS tersebut (biasanya berawalan `100.x.x.x` atau `fd7a:...`).
    3.  Set environment variable `NIMBUS_REQUIRE_TAILSCALE=true` agar node menolak startup jika tidak terikat ke IP privat VPN Tailscale.

---

## 2. Contoh Konfigurasi Environment VPS

### 2.1 Node Leader (VPS Utama - IP Tailscale: `100.64.0.1`)
```bash
# Konfigurasi Node & Database
NIMBUS_ENV=production
PORT=8080
NIMBUS_BIND_ADDR=100.64.0.1
NIMBUS_REQUIRE_TAILSCALE=true
NIMBUS_DB_PATH=/var/lib/nimbus/leader.db
NIMBUS_DB_KEY="ganti-dengan-key-database-aman"

# Kunci Kriptografi
NIMBUS_SHARE_INDEX=1
NIMBUS_SHARE_KEY=<secret_share_1_hex>
NIMBUS_THRESHOLD=3
NIMBUS_ISSUER_PUBLIC_KEY=<issuer_public_key_hex>

# Guardian Registry (Menyimpan URL Tailscale Guardian lainnya)
NIMBUS_GUARDIAN_PUBLIC_KEYS='{
  "2": "public_share_2_hex",
  "3": "public_share_3_hex",
  "4": "public_share_4_hex",
  "5": "public_share_5_hex"
}'

# Integrasi EVM (Diperlukan untuk Production/Mainnet)
NIMBUS_RPC_URL="https://your-arbitrum-rpc-endpoint"
NIMBUS_RELAYER_PRIVATE_KEY="<relayer_private_key>"
NIMBUS_CONTRACT_ADDRESS="0xe6430973795bb1cc3083787e554ef2a559dad7f1"
```

### 2.2 Node Guardian (VPS Guardian 1 - IP Tailscale: `100.64.0.2`)
```bash
# Konfigurasi Node & Database
NIMBUS_ENV=production
PORT=8080
NIMBUS_BIND_ADDR=100.64.0.2
NIMBUS_REQUIRE_TAILSCALE=true
NIMBUS_DB_PATH=/var/lib/nimbus/guardian_2.db
NIMBUS_DB_KEY="ganti-dengan-key-database-aman"

# Kunci Kriptografi
NIMBUS_SHARE_INDEX=2
NIMBUS_SHARE_KEY=<secret_share_2_hex>
NIMBUS_THRESHOLD=3
NIMBUS_ISSUER_PUBLIC_KEY=<issuer_public_key_hex>

# List Leader yang dipercaya (Address EIP-712 signer Leader)
NIMBUS_TRUSTED_LEADERS="0xLeaderSignerAddress"

# Guardian Registry (Daftar public share guardian/leader lainnya)
NIMBUS_GUARDIAN_PUBLIC_KEYS='{
  "1": "public_share_1_hex",
  "3": "public_share_3_hex",
  "4": "public_share_4_hex",
  "5": "public_share_5_hex"
}'
```

---

## 3. Administrasi Database & Integrasi KMS

### 3.1 Penyimpanan Database Lokal
*   Setiap VPS **wajib** menggunakan file database SQLite terpisah lokal pada disk VPS tersebut (`NIMBUS_DB_PATH`).
*   Jangan pernah menggunakan network storage (seperti NFS atau shared mount) untuk membagi satu file database di antara beberapa node, karena akan memicu lock collision dan merusak data SQLite.

### 3.2 Enkripsi SQLCipher
*   Database dienkripsi secara default menggunakan SQLCipher.
*   Pastikan mengonfigurasi variable `NIMBUS_DB_KEY` di level server OS secara aman (hindari hardcode key default).

### 3.3 Integrasi Vault / OpenBao KMS
Untuk tingkat keamanan tertinggi (production/mainnet), kunci rahasia relayer tidak boleh disimpan dalam environment variable `NIMBUS_SHARE_KEY`. 
Gunakan integrasi Vault/OpenBao KMS dengan menetapkan variabel berikut pada environment startup node:

```bash
# Mengaktifkan pengambilan share key langsung dari Vault KMS
export NIMBUS_VAULT_TOKEN="hvs.token-akses-vault-anda"
export NIMBUS_VAULT_ADDR="http://127.0.0.1:8200" # URL server Vault anda
export NIMBUS_VAULT_PATH="v1/secret/data/nimbus"  # Path tempat menyimpan secret key share
```
*Ketika `NIMBUS_VAULT_TOKEN` terdeteksi, node relayer akan secara otomatis mengambil private key share langsung dari KMS pada saat startup dan mem-bypass variable `NIMBUS_SHARE_KEY`.*
