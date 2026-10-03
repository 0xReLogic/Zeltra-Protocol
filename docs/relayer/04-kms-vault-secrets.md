# Nimbus Relayer — 04: Key Management System & Vault OpenBao

Dokumen ini menjelaskan integrasi Key Management System (KMS) untuk pengambilan kunci rahasia share BLS dan manajemen rahasia pada [`nimbus-node/src/kms.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/kms.rs).

---

## 1. Masalah Penyimpanan Kunci di Environment Variable

Menyimpan kunci rahasia (*secret share key*) langsung di file `.env` atau environment variable OS memiliki risiko fatal:
* Kunci dapat bocor melalui pembacaan `/proc/$PID/environ`.
* Kunci rahasia dapat ter-commit tanpa sengaja ke Git repository.
* Tidak memiliki jejak audit (*audit trail*) siapa dan kapan kunci diakses.

Oleh karena itu, `nimbus-node` mengintegrasikan layanan **HashiCorp Vault / OpenBao KMS** sebagai penyedia kunci resmi tingkat produksi.

---

## 2. Alur Pengambilan Kunci dari OpenBao/Vault

Saat node dinyalakan (*startup sequence*):

```text
[ Node Startup ]
       |
       v
Cek variabel NIMBUS_VAULT_TOKEN?
       ├── YA (Mode Produksi KMS):
       │    1. Kirim HTTP GET ke $NIMBUS_VAULT_ADDR/$NIMBUS_VAULT_PATH
       │       dengan header: X-Vault-Token: $NIMBUS_VAULT_TOKEN
       │    2. Parse JSON response: data.data.share_key
       │    3. Deserialisasi 40-byte buffer: (index: usize, share_sk: Fr)
       │    4. Simpan kunci di memori RAM terisolasi
       │
       └── TIDAK:
            ├── Mode Strict: FAIL-CLOSED (Node seketika exit(1))
            └── Mode Dev: Fallback membaca variabel NIMBUS_SHARE_KEY (Muncul Warning Keras)
```

---

## 3. Konfigurasi Environment KMS

Untuk mengaktifkan pengambilan kunci via Vault/OpenBao:

```bash
# Token autentikasi akses Vault
export NIMBUS_VAULT_TOKEN="hvs.CAESIJ..."

# Alamat server Vault / OpenBao
export NIMBUS_VAULT_ADDR="https://vault.internal.net:8200"

# Path penyimpanan secret data
export NIMBUS_VAULT_PATH="v1/secret/data/nimbus/guardian-1"
```

*Ketika `NIMBUS_VAULT_TOKEN` aktif, node secara otomatis mem-bypass pembacaan `NIMBUS_SHARE_KEY` mentah di file konfigurasi lokal.*

---

## 4. Keamanan Memori: Zeroization (`zeroize`)

Untuk mencegah data kunci tersisa di RAM setelah proses selesai:
* Node menggunakan crate `zeroize` (`zeroize = "1.8.2"`).
* Kunci rahasia dan masking factor di-overwrite dengan nilai nol seketika setelah operasi pembuatan signature share atau kalkulasi inverse selesai, mencegah kebocoran memori via memory dump atau swap disk.
