# Nimbus Relayer — 01: Arsitektur Cluster & Jaringan

Dokumen ini menjelaskan arsitektur distributed node, pembagian peran Leader vs Guardian, dan topologi jaringan aman pada [`nimbus-node`](file:///workspaces/Zeltra-Protocol/nimbus-node).

---

## 1. Peran Node: Leader vs Guardian

`nimbus-node` dapat dijalankan dalam salah satu dari dua peran utama tergantung konfigurasi environment:

```text
[ Client / SDK ]
       |
       | HTTPS (Public API)
       v
[ LEADER NODE ]  (Orkestrator & Relayer Transaksi)
       |
       | Tailscale Private Mesh VPN (Enkripsi WireGuard & ACL)
       +-----------------------+-----------------------+
       |                       |                       |
       v                       v                       v
[ GUARDIAN 1 ]          [ GUARDIAN 2 ]          [ GUARDIAN 3..5 ]
(Memegang Share 1)      (Memegang Share 2)      (Memegang Share 3..5)
```

| Fitur | Leader Node | Guardian Node |
|---|---|---|
| **Fungsi Utama** | Orkestrator signing, gateway API client, antrian spend, & broadcaster transaksi on-chain. | Menandatangani blind message secara independen menggunakan secret share lokal. |
| **Aksesibilitas** | Port publik terbuka untuk client API (`/api/deposit`, `/api/spend`, dll). | **TIDAK PERNAH terekspos ke internet publik**; hanya dapat dihubungi oleh Leader via Tailscale. |
| **Kunci Privat** | Memegang EVM relayer key (untuk bayar gas) + 1 share BLS lokal. | Hanya memegang 1 share BLS lokal (melalui Vault KMS atau env). |
| **State Storage** | SQLite SQLCipher (database antrian, nullifier, session). | Stateless atau database minimal lokal. |

---

## 2. Topologi Jaringan & Keamanan Tailscale (DEC-010)

Untuk mencegah kebocoran secret share dan serangan DDoS pada Guardian:

1. **Private Mesh Binding:** 
   Seluruh endpoint guardian (`/api/guardian/sign`) hanya di-bind ke interface IP privat **Tailscale** (CGNAT range `100.x.y.z`), bukan `0.0.0.0`.
2. **Tailscale Access Control Lists (ACL):** 
   Dikonfigurasi secara ketat sehingga port 3000 guardian hanya mengizinkan *inbound traffic* yang berasal dari tag/identitas node Leader (`tag:leader -> tag:guardian:3000`).
3. **Tanpa Ingress Publik:** 
   VPS guardian tidak memiliki port publik yang terbuka di firewall cloud (AWS Security Group / Azure NSG / DigitalOcean firewall menolak semua port kecuali SSH via VPN).
4. **Otentikasi Identitas:** 
   Leader mengenali guardian melalui registry kunci publik guardian (`NIMBUS_GUARDIAN_PUBLIC_KEYS`), dan guardian memvalidasi integritas blinded point yang diminta.

---

## 3. Ringkasan Endpoint API HTTP (`nimbus-node/src/handlers/`)

Semua endpoint dibangun menggunakan framework async **Axum 0.8**:

| Endpoint | Method | Fungsi |
|---|---|---|
| `/api/leader/sign` | `POST` | Client meminta koordinasi signing threshold 3/5 untuk blinded message $X$. |
| `/api/guardian/sign` | `POST` | Leader meminta partial signature share kepada masing-masing guardian. |
| `/api/deposit` | `POST` | Merekam sesi deposit baru sebelum transaksi on-chain dikirim client. |
| `/api/reveal` | `POST` | Merilis kunci masking $k$ ke client setelah deposit terkonfirmasi di on-chain. |
| `/api/quote/private-spend` | `GET` | Memberikan signed quote EIP-712 (estimasi gas, markup 15%, & diskon holding time). |
| `/api/spend` | `POST` | Menerima spend payload dari client dan memasukkannya ke antrian persisten. |
| `/health` | `GET` | Health check liveness relayer, saldo ETH gas, dan kedalaman antrian database. |
