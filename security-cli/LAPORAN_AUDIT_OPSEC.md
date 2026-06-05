# Laporan Audit Keamanan Operasional (OpSec): Nimbus CLI

**Tanggal Audit:** 5 Juni 2026  
**Auditor:** Cascade (Senior Security Auditor)  
**Scope:** Operational & Access Control Security  
**Fokus Area:** Penanganan input rahasia, konfirmasi operasi berbahaya, validasi argumen perintah

---

## Ringkasan Eksekutif

Berdasarkan analisis kode `nimbus-cli` dari sudut pandang Operational Security (OpSec), saya telah mengidentifikasi **3 kerentanan dengan tingkat keparahan Critical** dan **2 kerentanan High** yang berkaitan dengan kebocoran rahasia melalui shell history, kurangnya konfirmasi operasi berbahaya, dan validasi input yang tidak memadai.

---

## 1. KERENTANAN INPUT RAHASIA (KRITIS)

### 1.1 Secret Keys Dilewatkan sebagai Command-Line Arguments
**Lokasi:** `cli.rs:22, 59`, `keys.rs:21`, `blind.rs:21`

**Masalah:** Secret keys (sk) dilewatkan sebagai command-line arguments menggunakan flag `--sk`.

**Skenario Serangan:**
- Admin menjalankan perintah: `nimbus-cli sign --sk 0xabc123... --blinded 0xdef456...`
- Perintah ini tersimpan di shell history (`~/.bash_history`, `~/.zsh_history`)
- Attacker dengan akses ke sistem membaca history file
- Secret key diekstrak dan digunakan untuk sign transaksi tanpa otorisasi
- Secret key juga terlihat di process list (`ps aux`) dan system logs

**Perbaikan Kode:**
```rust
// cli.rs - Gunakan environment variable atau stdin untuk secrets
use std::io::{self, Write};
use rpassword::read_password;

#[derive(Subcommand)]
pub enum Commands {
    /// Sign a blinded message using the issuer secret key
    Sign {
        /// Blinded message in hex
        #[arg(short, long)]
        blinded: String,
        
        /// Issuer secret key (read from stdin or env var)
        #[arg(short, long, env = "NIMBUS_ISSUER_SK")]
        sk: Option<String>,
    },
}

// Di blind.rs - Prioritaskan stdin untuk secrets
pub fn sign<R: Rng>(blinded: String, sk: Option<String>, rng: &mut R) {
    let sk_hex = match sk {
        Some(key) => key,
        None => {
            print!("Enter Issuer Secret Key: ");
            io::stdout().flush().unwrap();
            read_password().expect("Failed to read secret key")
        }
    };
    
    // ... lanjutkan dengan validasi dan proses
}
```

### 1.2 Secret Keys Dicetak ke Stdout
**Lokasi:** `keys.rs:15`, `blind.rs:37-38`, `keys.rs:29-30`

**Masalah:** Secret keys, masking keys, dan secret shares dicetak langsung ke stdout.

**Skenario Serangan:**
- Admin menjalankan `nimbus-cli generate-keys`
- Output dengan secret key dicetak ke terminal
- Terminal session dicatat oleh screen recording software
- Output di-redirect ke file log yang tidak aman
- Attacker dengan akses ke log file mengekstrak secret key

**Perbaikan Kode:**
```rust
// keys.rs - Gunakan stderr untuk output non-rahasia, tulis rahasia ke file aman
use std::fs;
use std::os::unix::fs::PermissionsExt;

pub fn generate_keys<R: Rng>(rng: &mut R) {
    let sk = IssuerSecretKey::generate(rng);
    let pk = sk.public_key();
    
    let sk_hex = hex::encode(serialize_to_bytes(&sk));
    let pk_hex = hex::encode(serialize_to_bytes(&pk));
    
    // Tulis secret key ke file dengan permission 600
    let sk_file = "issuer_secret_key.hex";
    fs::write(sk_file, &sk_hex).expect("Failed to write secret key");
    fs::set_permissions(sk_file, fs::Permissions::from_mode(0o600))
        .expect("Failed to set file permissions");
    
    // Cetak public key ke stdout (aman)
    println!("NEW KEYPAIR GENERATED");
    println!("------------------------------------------------------------");
    println!("Secret Key saved to: {}", sk_file);
    println!("Public Key (pk_iss) :\n{}", pk_hex);
    println!("------------------------------------------------------------");
    
    // Hapus secret key dari memori setelah digunakan
    unsafe {
        std::ptr::write_bytes(sk_hex.as_ptr() as *mut u8, 0, sk_hex.len());
    }
}
```

---

## 2. KERENTANAN KONFIRMASI OPERASI (KRITIS)

### 2.1 Tidak Ada Prompt Konfirmasi untuk Operasi Berbahaya
**Lokasi:** Semua command handlers di `commands/mod.rs`

**Masalah:** Tidak ada prompt konfirmasi interaktif sebelum operasi berbahaya dieksekusi.

**Skenario Serangan:**
- Admin menjalankan perintah `nimbus-cli split-key --sk 0xabc123... --threshold 3 --total 5`
- Perintah langsung dieksekusi tanpa konfirmasi
- Jika admin salah ketik threshold atau total, secret key terbagi secara tidak sengaja
- Tidak ada kesempatan untuk membatalkan operasi
- Potensi kesalahan operasional yang tidak dapat di-undo

**Perbaikan Kode:**
```rust
// commands/mod.rs - Tambahkan konfirmasi interaktif
use std::io::{self, Write};

fn confirm_dangerous_operation(prompt: &str) -> bool {
    print!("{} [y/N]: ", prompt);
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Failed to read input");
    
    let input = input.trim().to_lowercase();
    input == "y" || input == "yes"
}

/// Dispatch command to appropriate handler
pub fn handle_command<R: Rng>(command: Commands, rng: &mut R) {
    match command {
        Commands::GenerateKeys => {
            if !confirm_dangerous_operation(
                "Generate new keypair? This will create a new secret key."
            ) {
                println!("Operation cancelled.");
                return;
            }
            keys::generate_keys(rng);
        }
        
        Commands::SplitKey { sk, threshold, total } => {
            let warning = format!(
                "Split secret key into {} shares with threshold {}? This operation cannot be undone.",
                total, threshold
            );
            if !confirm_dangerous_operation(&warning) {
                println!("Operation cancelled.");
                return;
            }
            keys::split_key(sk, threshold, total, rng);
        }
        
        Commands::Sign { blinded, sk } => {
            if !confirm_dangerous_operation(
                "Sign blinded message? This will use your secret key to create a signature."
            ) {
                println!("Operation cancelled.");
                return;
            }
            blind::sign(blinded, sk, rng);
        }
        
        // ... tambahkan konfirmasi untuk operasi berbahaya lainnya
    }
}
```

---

## 3. KERENTANAN VALIDASI ARGUMEN (TINGGI)

### 3.1 Panic pada Input yang Tidak Valid
**Lokasi:** `threshold.rs:6-8, 13-15`, `keys.rs:21-23`, `blind.rs:20-26`

**Masalah:** Input divalidasi menggunakan `.expect()` yang menyebabkan panic pada error.

**Skenario Serangan:**
- Attaker menyuntikkan input yang tidak valid: `--indices "abc,def,ghi"`
- Program panic dengan error message yang verbose
- Crash sistem lokal
- Potensi denial-of-service pada sistem yang menjalankan CLI
- Error message bisa mengungkapkan informasi internal

**Perbaikan Kode:**
```rust
// threshold.rs - Gunakan Result type untuk error handling yang graceful
use std::error::Error;

pub fn aggregate(indices: String, signatures: String) -> Result<(), Box<dyn Error>> {
    // Validasi dan parse indices dengan error handling yang baik
    let idx_list: Result<Vec<usize>, _> = indices.split(',')
        .map(|s| s.trim().parse::<usize>())
        .collect();
    
    let idx_list = idx_list.map_err(|e| {
        format!("Invalid index format: {}. Expected comma-separated numbers (e.g., '1,2,3')", e)
    })?;
    
    // Validasi panjang indices dan signatures
    if idx_list.is_empty() {
        return Err("At least one index is required".into());
    }
    
    if idx_list.len() > 100 {
        return Err("Too many indices. Maximum allowed is 100".into());
    }
    
    let sig_list: Vec<String> = signatures.split(',')
        .map(|s| s.trim().to_string())
        .collect();
    
    if idx_list.len() != sig_list.len() {
        return Err(format!(
            "Indices and signatures count mismatch: {} indices vs {} signatures",
            idx_list.len(),
            sig_list.len()
        ).into());
    }
    
    // Validasi hex format signatures
    for sig in &sig_list {
        if !is_valid_hex(sig) {
            return Err(format!("Invalid hex format for signature: {}", sig).into());
        }
    }
    
    // ... lanjutkan dengan proses aggregation
    
    Ok(())
}

fn is_valid_hex(s: &str) -> bool {
    s.len() % 2 == 0 && s.chars().all(|c| c.is_ascii_hexdigit())
}
```

### 3.2 Tidak Ada Validasi Panjang Input
**Lokasi:** Semua command arguments

**Masalah:** Tidak ada batasan panjang maksimum untuk string input.

**Skenario Serangan:**
- Attacker menyuntikkan input yang sangat panjang: `--sk "A" * 1000000`
- Memory exhaustion melalui alokasi string yang besar
- Crash sistem lokal
- Potential denial-of-service

**Perbaikan Kode:**
```rust
// cli.rs - Tambahkan validasi panjang input
const MAX_HEX_LENGTH: usize = 4096; // 2048 bytes dalam hex

#[derive(Subcommand)]
pub enum Commands {
    /// Split a secret key into n shares with threshold t
    SplitKey {
        /// Secret key in hex (max 4096 characters)
        #[arg(short, long)]
        #[arg(value_parser = validate_hex_length)]
        sk: String,
        
        /// Threshold t (1-100)
        #[arg(short, long, default_value_t = 3)]
        #[arg(value_parser = validate_threshold)]
        threshold: usize,
        
        /// Total shares n (1-100)
        #[arg(short = 'n', long, default_value_t = 5)]
        #[arg(value_parser = validate_total)]
        total: usize,
    },
}

fn validate_hex_length(s: &str) -> Result<String, String> {
    if s.len() > MAX_HEX_LENGTH {
        Err(format!(
            "Hex string too long. Maximum allowed is {} characters",
            MAX_HEX_LENGTH
        ))
    } else if !s.chars().all(|c| c.is_ascii_hexdigit()) {
        Err("Invalid hex format. Only 0-9 and a-f are allowed".to_string())
    } else {
        Ok(s.to_string())
    }
}

fn validate_threshold(t: &str) -> Result<usize, String> {
    let val = t.parse::<usize>()
        .map_err(|_| "Threshold must be a number".to_string())?;
    
    if val < 1 || val > 100 {
        Err("Threshold must be between 1 and 100".to_string())
    } else {
        Ok(val)
    }
}

fn validate_total(n: &str) -> Result<usize, String> {
    let val = n.parse::<usize>()
        .map_err(|_| "Total shares must be a number".to_string())?;
    
    if val < 1 || val > 100 {
        Err("Total shares must be between 1 and 100".to_string())
    } else {
        Ok(val)
    }
}
```

---

## 4. KEKHAWATIRAN OPSEC TAMBAHAN (DI LUAR SCOPE)

### 4.1 Tidak Ada Audit Logging
**Lokasi:** Seluruh CLI

**Masalah:** Tidak ada logging operasi untuk audit trail.

**Risiko:** Tidak ada cara untuk melacak siapa yang menjalankan operasi apa dan kapan.

### 4.2 Tidak Ada Rate Limiting
**Lokasi:** Semua command handlers

**Masalah:** Tidak ada batasan jumlah operasi per satuan waktu.

**Risiko:** Attacker bisa melakukan brute force atau flooding operations.

### 4.3 Tidak Ada User Authentication
**Lokasi:** Seluruh CLI

**Masalah:** Tidak ada verifikasi identitas pengguna sebelum operasi.

**Risiko:** Siapa pun dengan akses ke sistem bisa menjalankan operasi berbahaya.

### 4.4 Tidak Ada Secure Memory Handling
**Lokasi:** Semua operasi dengan secret data

**Masalah:** Secret data tidak dibersihkan dari memori setelah digunakan.

**Risiko:** Memory dump attack bisa mengekstrak secret data.

### 4.5 Tidak Ada File Permission Checks
**Lokasi:** Operasi file I/O

**Masalah:** Tidak ada verifikasi permission file sebelum membaca/menulis.

**Risiko:** Secret data bisa ditulis ke file dengan permission yang tidak aman.

### 4.6 Tidak Ada Context Verification
**Lokasi:** Seluruh CLI

**Masalah:** Tidak ada verifikasi apakah CLI berjalan sebagai root atau di environment yang tidak aman.

**Risiko:** Operasi berbahaya bisa berjalan di environment yang tidak diinginkan.

---

## 5. RINGKASAN REKOMENDASI

**Tindakan Segera (Kritis):**
1. Hapus secret key dari command-line arguments, gunakan stdin atau environment variable
2. Jangan cetak secret data ke stdout, simpan ke file dengan permission 600
3. Tambahkan prompt konfirmasi interaktif untuk semua operasi berbahaya
4. Ganti `.expect()` dengan error handling yang graceful menggunakan Result type

**Tindakan Jangka Pendek (Tinggi):**
1. Tambahkan validasi panjang input untuk semua string arguments
2. Tambahkan validasi format hex untuk semua hex inputs
3. Tambahkan validasi range untuk numeric arguments (threshold, total)
4. Implementasikan audit logging untuk semua operasi

**Tindakan Jangka Panjang (Sedang):**
1. Implementasikan secure memory handling (zeroing setelah penggunaan)
2. Tambahkan file permission checks untuk semua operasi file I/O
3. Implementasikan rate limiting untuk mencegah abuse
4. Tambahkan user authentication/authorization untuk operasi berbahaya
5. Implementasikan context verification (environment checks)
6. Tambahkan integrasi dengan hardware security module (HSM) untuk key storage

---

## Sumber Referensi

### Best Practices CLI Security

- **OWASP Command Injection Cheat Sheet:** Input validation, sanitization
- **CIS Benchmark for Linux:** Secure shell configuration, file permissions
- **NIST SP 800-53:** Access control, audit logging
- **Rust Security Guidelines:** Error handling, memory safety

### Operational Security Standards

- **OpSec Best Practices:** Secret handling, confirmation prompts
- **Defense in Depth:** Multiple layers of security controls
- **Principle of Least Privilege:** Minimal access required
- **Secure Coding Practices:** Input validation, error handling
