# DEC-010: Tailscale Security untuk Guardian Endpoint

Date: 2026-06-07

## Masalah

Endpoint signing guardian saat ini dapat diakses dari network interface apapun
termasuk interface publik. Tidak ada pembatasan akses berbasis network, hanya
validasi application-layer (allowlist identity leader). Ini meningkatkan attack
surface untuk:

- Scanning port oleh attacker eksternal
- Brute force atau exploitation vulnerability application-layer
- Exposure jika ACL/allowlist application-layer salah konfigurasi
- Side-channel attacks melalui timing error responses

## Invariant bisnis/security

- Endpoint guardian hanya dapat diakses dari Tailscale network
- Hanya node leader yang dapat mengakses endpoint guardian
- Port guardian tidak listen pada interface publik
- Tailscale ACL memberikan defense-in-layer di atas application-layer auth
- Device yang keluar dari tailnet langsung kehilangan akses

## Pilihan yang dipertimbangkan

1. Hanya application-layer auth (allowlist IP/TLS) tanpa Tailscale binding
2. Bind ke Tailscale interface tanpa ACL (semua node tailnet bisa akses)
3. Bind ke Tailscale interface dengan ACL strict (hanya leader)
4. Tambahkan mTLS atau signed request di atas Tailscale

## Keputusan

Gunakan pilihan 3 untuk testnet: bind endpoint guardian hanya ke interface Tailscale
dan konfigurasi ACL strict yang hanya mengizinkan akses dari node leader.

Untuk production, tambahkan pilihan 4 sebagai defense-in-depth.

## Alasan

- Pilihan 1 tidak memberikan network-level defense dan bergantung sepenuhnya
  pada application-layer yang bisa salah konfigurasi
- Pilihan 2 masih memungkinkan akses dari node tailnet lain yang compromised
- Pilihan 3 memberikan zero-trust network segmentation dengan overhead minimal
- Pilihan 4 memberikan defense-in-depth tetapi dapat ditambahkan bertahap

Tailscale ACL adalah deny-by-default, locally enforced, dan otomatis terdistribusi
ke semua device dalam tailnet, memberikan network-layer security tanpa perlu
manajemen firewall manual per server.

## Sumber primer

- Tailscale ACL Documentation: https://tailscale.com/docs/features/access-control/acls
- Tailscale ACL API: https://github.com/tailscale/tailscale/blob/main/api.md
- Grants vs ACLs: https://tailscale.com/docs/reference/grants-vs-acls
- Tailscale Access Control Principles: https://tailscale.com/docs/features/access-control

## Sumber pembanding

- THORChain TSS network security practices:
  https://dev.thorchain.org/bifrost/tss.html
- Lux Network threshold signing node security:
  https://lps.lux.network/docs/lp-5013/
- ICP Threshold Signatures subnet isolation:
  https://docs.internetcomputer.org/docs/references/t-sigs-how-it-works

Accessed: 2026-06-07.

## Known risks

- Jika ACL salah konfigurasi, leader tidak dapat mengakses guardian
- Jika device dihapus dari tailnet, akses langsung terputus tanpa grace period
- ACL tidak melindungi dari kompromi pada node leader itu sendiri
- Application-layer auth masih diperlukan sebagai defense-in-depth
- Tailscale coordination server outage dapat mempengaruhi konektivitas

## Versi/library/network

- Tailscale client: latest stable
- Tailscale ACL policy: JSON/HuJSON format
- Network binding: OS-level socket binding (Linux SO_BINDTODEVICE)
- Rust HTTP server: Axum with socket option configuration

## Test

Positive:

- Leader dapat mengakses endpoint guardian melalui Tailscale IP
- ACL rule yang benar mengizinkan akses leader-to-guardian
- Device yang keluar dari tailnet tidak dapat mengakses endpoint

Negative:

- Node bukan leader dalam tailnet tidak dapat akses
- Akses dari IP publik ditolak (port tidak listen pada interface publik)
- ACL rule salah ditolak dengan error jelas

## Rollback/recovery

Jika Tailscale binding menyebabkan issue:
1. Dapat dinonaktifkan dengan bind ke 0.0.0.0 sembari mempertahankan application-layer auth
2. ACL dapat di-revert ke allow-all sementara untuk debugging
3. Device dapat di-rejoin tailnet jika dihapus tidak sengaja

## Implementation

1. Konfigurasi Axum/HTTP server untuk bind hanya ke Tailscale interface:
   ```rust
   // Bind to tailscale0 interface only
   let addr = SocketAddr::from_str("100.x.y.z:8080")?; // Tailscale IP
   let listener = TcpListener::bind(&addr)?;
   ```

2. Konfigurasi Tailscale ACL di tailnet policy file:
   ```json
   {
     "acls": [
       {
         "action": "accept",
         "src": ["tag:leader"],
         "dst": ["tag:guardian:8080"]
       }
     ],
     "tagOwners": {
       "tag:leader": ["admin@tailnet"],
       "tag:guardian": ["admin@tailnet"]
     }
   }
   ```

3. Validasi bahwa port tidak listen pada interface publik:
   ```bash
   sudo ss -tulpn | grep 8080
   # Harus hanya menunjukkan tailscale0, tidak eth0
   ```

4. Add startup validation: fail jika interface Tailscale tidak tersedia

## Dependency pada DEC

- DEC-007 (secure guardian authentication) - application-layer auth yang sudah ada
- DEC-010 memperkuat DEC-007 dengan network-layer security
