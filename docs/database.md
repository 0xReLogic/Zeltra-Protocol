# Database Integration

**Status**: Persistent queue implemented; crash/nonce reconciliation pending
**Date**: June 7, 2026

## Overview

SQLite menyimpan session, idempotency, persistent spend queue, dan nullifier
yang sudah confirmed. Desain settlement queue mengikuti
[`DEC-005`](../research/decisions/DEC-005-persistent-spend-settlement-queue.md).

### Security Improvements (June 6, 2026)

Based on security audit findings, the following improvements were implemented:

1. **Database Encryption**: Added SQLCipher with AES-256 encryption
2. **File Permissions**: Set chmod 600 on database, WAL, and SHM files
3. **Secure Delete**: Enabled PRAGMA secure_delete = ON
4. **Log Redaction**: Redacted sensitive data (nullifiers, keys) to 8 characters
5. **Key Management**: Added NIMBUS_DB_KEY environment variable for encryption key

See `/security-node/SQLITE_DATABASE_SECURITY_AUDIT.md` for full audit details.

## Implementation

### 1. Database Module (`nimbus-node/src/database.rs`)

**Features**:
- SQLite with optimized PRAGMAs
- Four tables: `sessions`, `nullifiers`, `spend_queue`, `idempotency_cache`
- Transactional queue claim dengan lease
- Persistent lifecycle `queued`, `retryable`, `broadcasting`, `submitted`,
  `confirmed`, dan `failed`
- Nullifier confirmed ditulis hanya setelah receipt sukses
- Request idempotency caching with TTL-based expiration (24 hours)
- Async/blocking hybrid using `tokio::spawn_blocking`

**Key PRAGMAs**:
```sql
PRAGMA journal_mode = WAL;        -- Concurrent reads
PRAGMA synchronous = NORMAL;      -- Balance durability/performance
PRAGMA cache_size = -64000;       -- 64MB cache
PRAGMA foreign_keys = ON;         
PRAGMA temp_store = MEMORY;       
PRAGMA busy_timeout = 5000;
```

### 2. Database Methods

| Method | Purpose |
|--------|---------|
| `insert_session()` | Track deposits, reject duplicates |
| `resolve_session()` | Reveal masking keys, prevent double-reveal |
| `check_and_insert_nullifier()` | Atomic double-spend prevention (INSERT OR IGNORE) |
| `is_nullifier_spent()` | Fast nullifier lookup |
| `enqueue_spend()` | Persist request dan reserve nullifier aktif |
| `claim_spends()` | Transactional worker claim dengan lease |
| `retry_spend()` | Exponential backoff untuk failure sebelum confirmation |
| `mark_spend_submitted()` | Simpan source tx hash |
| `mark_spend_confirmed()` | Atomic queue confirmation + confirmed nullifier |
| `fail_spend()` | Simpan terminal failure tanpa menghapus evidence |
| `check_idempotency()` | Check client-supplied key for duplicate request |
| `store_idempotency()` | Store client-supplied key with serializable response |
| `cleanup_idempotency_cache()` | Evict expired keys (older than 24 hours) |
| `get_stats()` | Monitoring |

### 3. State Changes

**Before**:
```rust
pub struct AppState {
    pub sessions: Arc<Mutex<Vec<Session>>>,
    pub nullifiers: Arc<Mutex<Vec<String>>>,
}
```

**After**:
```rust
pub struct AppState {
    pub db: Database,  // Persistent SQLite
    pub key_manager: KeyManager,
    pub guardian_circuit_breaker: CircuitBreaker,
    pub vault_circuit_breaker: CircuitBreaker,
    pub rpc_circuit_breaker: CircuitBreaker,
}
```

### 4. Handler Updates

- `handlers/deposit.rs` - Uses `db.insert_session()` and `db.resolve_session()`
- `handlers/spend.rs` - Enqueue dan process persistent settlement lifecycle
- `handlers/x402.rs` - Enqueue saja; tidak direct broadcast
- `handlers/health.rs` - Membaca queue depth dari database

### 5. Dependencies

```toml
rusqlite = { version = "0.32", features = ["bundled-sqlcipher"] }
anyhow = "1.0"
tempfile = "3.0"  # for tests
```

**Security Note**: `bundled-sqlcipher` feature enables AES-256 encryption for the database with bundled SQLCipher library.

## Why SQLite

SQLite dipilih untuk relayer single-node karena sudah menjadi dependency repo,
memberikan transaksi atomic dan WAL crash recovery, serta menjaga scope
operasional MVP. Batas throughput belum diklaim sebelum load test, WAL
checkpoint test, disk-full test, backup, dan restore selesai.

## Test Results

`cargo test -p nimbus-node -- --test-threads=1` lulus pada 7 Juni 2026:

- 11 library tests
- 12 binary tests
- 2 integration tests

## Deployment

```bash
# Set database path (optional, default: ./nimbus-relayer.db)
export NIMBUS_DB_PATH=/var/lib/nimbus/relayer.db

# Set database encryption key (REQUIRED for production)
export NIMBUS_DB_KEY="your-secure-encryption-key-here"

# Run node
cargo run --package nimbus-node
```

### Security Deployment Checklist

- [ ] Set strong `NIMBUS_DB_KEY` environment variable
- [ ] Store encryption key in KMS (OpenBao/Vault) for production
- [ ] Run application as dedicated non-root user
- [ ] Verify database files have chmod 600 permissions
- [ ] Enable log rotation for secure log management
- [ ] Set up automated encrypted backups
- [ ] Test database recovery procedure

## Database Schema

### sessions
```sql
CREATE TABLE sessions (
    session_id TEXT PRIMARY KEY,
    com_k_hex TEXT NOT NULL,
    amount INTEGER NOT NULL,
    resolved BOOLEAN NOT NULL DEFAULT 0,
    masking_key_hex TEXT,
    client_address TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    resolved_at INTEGER
);
```

### nullifiers
```sql
CREATE TABLE nullifiers (
    nullifier TEXT PRIMARY KEY,
    spent_at INTEGER NOT NULL,
    tx_hash TEXT
);
```

### spend_queue
```sql
CREATE TABLE spend_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    nullifier TEXT NOT NULL,
    sig_hex TEXT NOT NULL,
    recipient TEXT NOT NULL,
    amount INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    processed BOOLEAN NOT NULL DEFAULT 0,
    tx_hash TEXT,
    request_json TEXT,
    status TEXT NOT NULL DEFAULT 'queued',
    retry_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    available_at INTEGER NOT NULL DEFAULT 0,
    lease_until INTEGER,
    updated_at INTEGER NOT NULL DEFAULT 0,
    block_number INTEGER
);
```

### idempotency_cache
```sql
CREATE TABLE idempotency_cache (
    idempotency_key TEXT PRIMARY KEY,
    endpoint TEXT NOT NULL,
    response_json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_idempotency_created ON idempotency_cache(created_at);
```

## Security

### Encryption at Rest
- **SQLCipher**: Database encrypted with AES-256 using SQLCipher
- **Key Management**: Encryption key from `NIMBUS_DB_KEY` environment variable
- **Default Key**: Falls back to "default-change-in-production" (MUST be changed in production)
- **Recommendation**: Use KMS (OpenBao/Vault) for production key management

### File Permissions
- **Database Files**: chmod 600 (owner-only read/write)
- **WAL Files**: chmod 600 on .db-wal files
- **SHM Files**: chmod 600 on .db-shm files
- **Recommendation**: Run application as dedicated non-root user

### Data Protection
- **Secure Delete**: `PRAGMA secure_delete = ON` overwrites deleted data
- **Log Redaction**: Sensitive data (nullifiers, keys) redacted to 8 characters in logs
- **Reservation**: Satu nullifier hanya memiliki satu queue item aktif
- **Confirmation**: `nullifiers` ditulis bersama perubahan queue ke `confirmed`
- **Session uniqueness**: Primary key constraint
- **Data persistence**: WAL mode survives crashes
- **Concurrent access**: 5s busy timeout

### Operational Security
- Contract tetap source of truth settlement finansial
- Database menjadi source of truth workflow relayer lokal
- If database is lost/corrupted:
  - confirmed state harus direkonsiliasi dari chain;
  - pending queue membutuhkan backup/restore;
  - jangan menganggap user resubmit selalu aman karena transaksi sebelumnya
    mungkin sudah diterima RPC.

## Remaining Reliability Work

- Persistent nonce allocation dan signed raw transaction storage.
- Reconciliation sender+nonce dan nullifier contract setelah restart.
- Confirmation threshold serta reorg handling.
- Disk-full, corrupt database, backup/restore, dan process-kill hard test.

## Storage Size

**Realistic estimates:**
- Low traffic (100 tx/day): ~40 KB/day = 1.2 MB/month = 14 MB/year
- Medium (1000 tx/day): ~400 KB/day = 12 MB/month = 144 MB/year  
- High (10K tx/day): ~4 MB/day = 120 MB/month = 1.4 GB/year

**Conclusion: MB-an doang**, paling besar ratusan MB.

## Auto-Cleanup

Background task runs daily to:
1. Delete resolved sessions older than 30 days
2. Clean up expired idempotency cache records (older than 24 hours)
3. Vacuum database to reclaim space
4. Log database size

Manual cleanup:
```rust
// Cleanup sessions older than 30 days
let deleted = db.cleanup_old_sessions(30).await?;

// Reclaim space
db.vacuum().await?;

// Check size
let size_bytes = db.get_db_size().await?;
```

## Recovery

If database is lost/corrupted:
1. **User funds**: SAFE (on blockchain)
2. **Nullifiers**: Rebuild from blockchain events
3. **Pending batches**: Lost (users resubmit)

Database is operational cache, not source of truth.

## Migration to PostgreSQL

**When to migrate**: Traffic >1000 TPS or need multi-region.

### Step 1: Install PostgreSQL and pgloader

```bash
# Install PostgreSQL
sudo apt install postgresql postgresql-contrib

# Install pgloader (SQLite to PostgreSQL migration tool)
sudo apt install pgloader
```

### Step 2: Migrate Data

```bash
# Stop relayer node
systemctl stop nimbus-relayer

# Migrate SQLite to PostgreSQL
pgloader nimbus-relayer.db postgresql://user:pass@localhost/nimbus

# Verify migration
psql -U user -d nimbus -c "SELECT COUNT(*) FROM sessions;"
psql -U user -d nimbus -c "SELECT COUNT(*) FROM nullifiers;"
```

### Step 3: Update Code

Replace `rusqlite` with `sqlx` in `Cargo.toml`:

```toml
# Remove
# rusqlite = { version = "0.32", features = ["bundled"] }

# Add
sqlx = { version = "0.7", features = ["runtime-tokio-native-tls", "postgres"] }
```

Update `database.rs`:

```rust
// Change Connection to sqlx Pool
use sqlx::{PgPool, postgres::PgPoolOptions};

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(50)
            .connect(database_url)
            .await?;
        
        // Run migrations
        sqlx::migrate!("./migrations").run(&pool).await?;
        
        Ok(Self { pool })
    }
}
```

### Step 4: Update Queries

SQLite to PostgreSQL differences:

```rust
// SQLite
"INSERT OR IGNORE INTO nullifiers ..."

// PostgreSQL  
"INSERT INTO nullifiers ... ON CONFLICT (nullifier) DO NOTHING"
```

### Step 5: Deploy

```bash
# Set DATABASE_URL
export DATABASE_URL=postgresql://user:pass@localhost/nimbus

# Restart relayer
systemctl start nimbus-relayer
```

### Migration Checklist

- [ ] Backup SQLite database
- [ ] Install PostgreSQL
- [ ] Run pgloader migration
- [ ] Verify row counts match
- [ ] Update Cargo.toml dependencies
- [ ] Update database.rs to use sqlx
- [ ] Change INSERT OR IGNORE to ON CONFLICT
- [ ] Test locally
- [ ] Deploy to production
- [ ] Monitor for errors
- [ ] Keep SQLite backup for 7 days

### Rollback Plan

If migration fails:

```bash
# Stop new version
systemctl stop nimbus-relayer

# Restore SQLite version
git checkout <previous-commit>
cargo build --release

# Use old SQLite database
export NIMBUS_DB_PATH=/backup/nimbus-relayer.db

# Restart
systemctl start nimbus-relayer
```

## Files Changed

New:
- `nimbus-node/src/database.rs`
- `nimbus-node/src/lib.rs`
- `nimbus-node/tests/integration_test.rs`

Modified:
- `nimbus-node/src/state.rs`
- `nimbus-node/src/main.rs`
- `nimbus-node/src/handlers/*.rs`
- `nimbus-node/Cargo.toml`
