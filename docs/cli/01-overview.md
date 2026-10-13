# Zeltra CLI — 01: Overview & Arsitektur

Dokumen ini menjelaskan struktur internal, dependensi, dan cara build dari crate `zeltra-cli`.

---

## 1. Peran Zeltra CLI

`zeltra-cli` adalah utilitas command-line lokal untuk:
1. Menguji dan menyimulasikan seluruh siklus kriptografi **Blind Diffie-Hellman Key Exchange (BDHKE)** secara mandiri tanpa memerlukan koneksi jaringan RPC.
2. Melakukan manajemen kunci issuer (pembuatan keypair dan pembagian secret key via Shamir Secret Sharing).
3. Melakukan agregasi parsial threshold signature sebelum diintegrasikan ke cluster guardian relayer.

---

## 2. Struktur Modul Rust (`zeltra-cli/src/`)

Arsitektur kode `zeltra-cli` memisahkan secara ketat antara parsing argumen CLI dan eksekusi logika kriptografi:

```text
zeltra-cli/
├── Cargo.toml               # Konfigurasi dependensi crate
└── src/
    ├── main.rs              # Entry point minimal: inisialisasi RNG & dispatch CLI
    ├── cli.rs               # Definisi Clap derive (enum Commands & argumen)
    └── commands/            # Logika eksekusi perintah
        ├── mod.rs           # Dispatcher: handle_command(command, rng)
        ├── keys.rs          # Handler pembuatan kunci & Shamir secret splitting
        ├── blind.rs         # Handler protokol BDHKE (blind, sign, verify, unmask)
        └── threshold.rs     # Handler agregasi parsial signature threshold
```

### Karakteristik Kode:
* **`main.rs` (Entrypoint):** Hanya berisi 14 baris. Menggunakan `Cli::parse()` dari Clap dan mengoper command ke dispatcher dengan random number generator thread lokal (`thread_rng`).
* **`cli.rs` (Clap Definitions):** Murni pendefinisian struct & enum tanpa logika bisnis. Semua subcommands didefinisikan dengan atribut derive `#[command(subcommand)]`.
* **`commands/` (Separation of Concerns):** Logika kriptografi diimpor langsung dari crate `zeltra-core` (`IssuerSecretKey`, `client_blind`, `issuer_sign_blinded`, dll).

---

## 3. Dependensi (`Cargo.toml`)

`zeltra-cli` dibangun dengan dependensi minimal yang efisien:

```toml
[dependencies]
zeltra-core = { path = "../zeltra-core" }     # Primitif kriptografi BLS12-381 & Arkworks
clap = { version = "4.6.1", features = ["derive"] } # CLI argument parser
rand = "0.8.6"                               # Secure random number generator
tokio = { version = "1.52.3", features = ["full"] } # Async runtime
hex = "0.4.3"                                 # Encoding/decoding data kurva ke format Hex
```

---

## 4. Cara Build Binary

Pastikan berada di root direktori project, lalu jalankan:

```bash
cargo build --bin zeltra-cli
```

Binary hasil kompilasi akan berada di:
* Debug build: `./target/debug/zeltra-cli`
* Release build (optimasi penuh): `cargo build --release --bin zeltra-cli` $\rightarrow$ `./target/release/zeltra-cli`
