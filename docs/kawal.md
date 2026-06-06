# KawalHarta -- Transparansi Harta Pejabat Indonesia On-Chain

> **Status**: IDE / Draft -- dikerjakan setelah Nimbus Protocol mainnet launch.
> **Tanggal Ide**: 6 Juni 2026
> **Founder**: Solo (sama seperti Nimbus)
> **Infrastruktur**: Reuse 80% kode Nimbus (Stylus contracts, Relayer, Alloy, SQLite)

---

## 1. Masalah

- APBN Indonesia Rp 3.000+ triliun/tahun, dibiayai uang rakyat.
- 400.000+ pejabat wajib lapor LHKPN (harta kekayaan) ke KPK.
- Data LHKPN saat ini:
  - **Self-reported** (pejabat isi sendiri, bisa bohong).
  - **Format PDF** (susah dianalisis massal).
  - **Akses makin ditutup** (per Maret 2026 butuh autentikasi).
  - **Ga ada alert otomatis** jika harta naik abnormal.
  - **Ga ada cross-check** dengan gaji resmi.
- Rakyat tidak punya tools untuk mengawasi secara efektif.

### Data Sources yang Sudah Ada (Publik)

| Sumber | URL | Isi |
|--------|-----|-----|
| e-LHKPN KPK | elhkpn.kpk.go.id | Laporan harta 404.303 pejabat |
| KawalHarta.org | kawalharta.org | Agregator LHKPN (24.988 aset terpantau) |
| Data Rakyat API | datarakyat.id/docs/lhkpn | REST API scraper LHKPN (MIT license) |
| KPK Statistik | kpk.go.id/publikasi-data/statistik/lhkpn-id | Statistik kepatuhan pelaporan |

---

## 2. Solusi: KawalHarta.id

Platform web publik (GRATIS) yang:
1. **Scrape + snapshot** data LHKPN on-chain (Arbitrum L2, gas murah) -- data jadi **permanen dan tidak bisa dihapus**.
2. **Scoring Engine** (AI + rules) menilai "Honesty Score" setiap pejabat berdasarkan perbandingan harta vs gaji resmi.
3. **Community Auditor** -- warga bisa flag/challenge data pejabat dengan bukti.
4. **Token $KAWAL** -- memberi insentif ekonomi bagi ekosistem auditor dan pendanaan platform.

---

## 3. Arsitektur

```
┌────────────────────────────────────────────────────┐
│          FRONTEND (Web Biasa, Publik, GRATIS)       │
│  - Dashboard harta pejabat                          │
│  - Honesty Score ranking                            │
│  - Anomaly alerts                                   │
│  - Community challenge feed                         │
└────────────────────┬───────────────────────────────┘
                     │
                     ▼
┌────────────────────────────────────────────────────┐
│              BACKEND / RELAYER NODE                  │
│  (Reuse nimbus-node)                                │
│  - Scraper: LHKPN data ingestion                   │
│  - Scoring Engine: hitung Honesty Score             │
│  - API publik: /api/official/{name}                 │
│  - API publik: /api/ranking                         │
│  - API publik: /api/anomalies                       │
│  - Indexer: baca on-chain data                      │
│  - Database: SQLite cache (reuse database.rs)       │
└────────────────────┬───────────────────────────────┘
                     │
                     ▼
┌────────────────────────────────────────────────────┐
│          SMART CONTRACT (Arbitrum Stylus)            │
│  (Reuse nimbus-contracts)                           │
│                                                     │
│  DataRegistry:                                      │
│  - snapshot_lhkpn(official_id, hash, year)          │
│  - record_score(official_id, score, timestamp)      │
│  - submit_challenge(official_id, evidence_hash)     │
│  - vote_challenge(challenge_id, approve/reject)     │
│                                                     │
│  TokenContract ($KAWAL):                            │
│  - stake(amount) -- jadi Community Auditor          │
│  - claim_reward() -- claim reward dari audit valid  │
│  - slash(auditor) -- hukuman untuk spam/flag palsu  │
│  - burn(amount) -- deflasi dari revenue             │
│                                                     │
│  Treasury:                                          │
│  - receive_revenue()                                │
│  - distribute_rewards() -- 40% auditor              │
│  - distribute_staker() -- 20% passive staker        │
│  - fund_development() -- 30% dev fund              │
│  - auto_burn() -- 10% burn                         │
└────────────────────────────────────────────────────┘
```

---

## 4. Honesty Score Engine

```
Input:
  - Harta dilaporkan (dari LHKPN)
  - Gaji + tunjangan resmi (data publik)
  - Riwayat harta tahun-tahun sebelumnya
  - Community flags/challenges

Kalkulasi:
  wealth_gap = kenaikan_harta_per_tahun / gaji_tahunan_resmi

  Skor:
  - wealth_gap <= 1.0   → Score 80-100 (wajar)
  - wealth_gap 1.0-2.0  → Score 60-80  (perlu penjelasan)
  - wealth_gap 2.0-5.0  → Score 30-60  (mencurigakan)
  - wealth_gap > 5.0    → Score 0-30   (sangat mencurigakan)

  Modifier:
  - Telat lapor LHKPN    → -10 poin
  - Ada challenge valid   → -15 poin
  - Konsisten tiap tahun  → +5 poin
  - Kehadiran rapat tinggi → +5 poin

Output:
  Honesty Score 0-100 (tercatat on-chain, permanen)
```

---

## 5. Token $KAWAL Economics

### Supply & Distribution

| Alokasi | Persentase | Keterangan |
|---------|-----------|------------|
| Community Rewards | 40% | Reward untuk auditor yang flag valid |
| Development | 20% | Biaya pengembangan platform |
| Public Sale | 15% | Listing awal |
| Treasury | 15% | Dana operasional jangka panjang |
| Team (vested 2 tahun) | 10% | Founder + kontributor |

### 4 Utilitas Token

1. **Stake to Audit**: Stake 100+ $KAWAL untuk jadi Community Auditor, bisa submit challenge. Flag valid = reward, flag spam = slash.
2. **Governance**: 1 $KAWAL = 1 vote. Vote investigasi pejabat, validasi challenge, parameter platform.
3. **Premium Access**: Export data, custom analytics, API developer, early alert notification.
4. **Revenue Sharing**: 20% revenue platform didistribusikan ke staker sebagai passive income.

### Revenue Streams (Uang Masuk)

| Sumber | Model | Target |
|--------|-------|--------|
| API Premium (jurnalis, NGO, hukum) | $5-50/bulan subscription | $10K/bulan |
| Enterprise License (ICW, TI, firma hukum) | $500-5000/bulan | $20K/bulan |
| Bounty Pool (donor/lembaga bayar investigasi) | Per kasus | Variable |
| Dashboard Ads (traffic tinggi) | CPM-based | $5K/bulan |
| Analytics Reports (per daerah/kementerian) | $10-100/report | $5K/bulan |

### Revenue Distribution (Smart Contract)

```
Revenue masuk Treasury → auto-distribute:
├── 40% → Community Auditor rewards
├── 30% → Development fund
├── 20% → Staker rewards (passive income)
└── 10% → Token burn (deflasi)
```

---

## 6. Deteksi Kecurangan (Ga Cuma Percaya Orang Jujur)

### Layer 1: Data dari Sumber Digital (Bukan Manual)
- Integrasikan langsung ke sistem bank/keuangan negara (SPAN/SAKTI) kalau memungkinkan.
- Scrape data LHKPN otomatis, bukan input manual.

### Layer 2: Multi-Party Verification
- Setiap data butuh konfirmasi dari beberapa sumber sebelum skor final.
- Cross-check LHKPN vs data properti BPN vs data pajak.

### Layer 3: AI Cross-Check
- Bandingkan harga aset yang dilaporkan vs harga pasar (oracle).
- Deteksi pola anomali (sudden wealth increase, hidden assets).
- Satellite imagery untuk verifikasi properti (opsional, fase lanjut).

### Layer 4: Crowdsourced Audit (270 Juta Mata)
- Warga bisa challenge: "Pak X bilang rumahnya 1, tapi saya tau dia punya 3."
- Challenge + bukti di-hash on-chain (permanen).
- Community vote validasi challenge.

### Layer 5: Immutable Record
- Semua data on-chain = tidak bisa dihapus.
- Bahkan kalau KPK tutup portal, data di blockchain tetap ada.
- Kebohongan menjadi **bukti permanen** yang bisa dipakai kapan saja.

---

## 7. Reuse Komponen Nimbus

| Komponen Nimbus | Reuse untuk KawalHarta |
|----------------|----------------------|
| `nimbus-contracts` (Stylus) | DataRegistry + TokenContract |
| `nimbus-node` (Relayer) | Scraper + API server + Indexer |
| `database.rs` (SQLite) | Cache data LHKPN lokal |
| `evm_client.rs` (Alloy) | Broadcast data snapshots on-chain |
| `circuit_breaker.rs` | RPC resilience untuk scraper |
| `kms.rs` | Secure key management untuk admin |
| Arbitrum L2 | Sama -- gas murah, EVM compatible |

Estimasi kode baru yang perlu ditulis: **~20%** (scraper LHKPN, scoring engine, frontend).

---

## 8. Timeline Setelah Nimbus Mainnet

| Fase | Apa | Durasi |
|------|-----|--------|
| **MVP** | Web dashboard + LHKPN scraper + scoring engine (no token) | 2-3 minggu |
| **On-Chain** | Snapshot data on-chain + immutable scores | 1 minggu |
| **Community** | Auditor system + challenge + voting | 2 minggu |
| **Token** | Launch $KAWAL + staking + governance (SETELAH ada user) | 2-4 minggu |

**ATURAN PENTING**: Jangan launch token sebelum ada user base. Bangun platform → dapatkan traffic → baru launch token.

---

## 9. Forbes Angle

- **BayaniChain** (Forbes Asia 2026) masuk cuma karena 1 kolaborasi pemerintah Filipina.
- Indonesia 5x lebih besar. 270 juta penduduk = potential user base terbesar di Asia Tenggara.
- Narrative: "Solo developer Indonesia bangun anti-corruption infra menggunakan cryptography yang sama dengan privacy protocol."
- Dual product story: Nimbus (privacy for crypto) + KawalHarta (transparency for government) dari 1 infrastruktur = versatilitas teknologi yang langka.

### Target untuk Nominasi Forbes Asia 30 Under 30

1. Mainnet Nimbus launch
2. KawalHarta MVP live dengan 10K+ monthly users
3. 1 partnership/MoU (ICW, Transparency International, atau instansi pemerintah)
4. 1 grant (Arbitrum Foundation atau Chainlink BUILD)
5. 2-3 media coverage (CoinDesk, The Block, atau media Indonesia besar)

---

## 10. Referensi & Inspirasi

- **The Graph (GRT)**: Stake to index blockchain data → query fees. Model auditor KawalHarta mirip.
- **Chainlink (LINK)**: Stake to provide oracle data → data feed fees.
- **BayaniChain**: Forbes Asia 2026 -- blockchain untuk transparansi pemerintah Filipina.
- **XION (Anthony Anzalone)**: Forbes 30 Under 30 2026 -- ZK verification infra, 150+ brand, $35M ARR.
- **KawalPemilu**: Crowdsourced election monitoring Indonesia -- proof bahwa model crowdsourced audit works di Indonesia.
- **Data Rakyat (datarakyat.id)**: Open source LHKPN scraper -- bisa jadi starting point teknis.

---

> **Catatan**: Dokumen ini adalah draft ide awal. Detail teknis, legal compliance, dan token economics akan diperhalus setelah Nimbus Protocol mencapai mainnet readiness. Prioritas utama tetap: **selesaikan Nimbus dulu**.
