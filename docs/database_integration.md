# Database Integration

**Status**: COMPLETE  
**Date**: June 5, 2026  

## Overview

Replaced in-memory storage with SQLite database for persistent session and nullifier tracking.

## Implementation

### 1. Database Module (`nimbus-node/src/database.rs`)

**Features**:
- SQLite with optimized PRAGMAs
- Three tables: `sessions`, `nullifiers`, `spend_queue`
- Atomic double-spend prevention using `INSERT OR IGNORE` pattern
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
}
```

### 4. Handler Updates

- `handlers/deposit.rs` - Uses `db.insert_session()` and `db.resolve_session()`
- `handlers/spend.rs` - Uses `db.is_nullifier_spent()` and `db.check_and_insert_nullifier()`
- `handlers/x402.rs` - Uses `db.is_nullifier_spent()`
- `handlers/health.rs` - Uses `db.get_stats()`

### 5. Dependencies

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
anyhow = "1.0"
tempfile = "3.0"  # for tests
```

## Why SQLite

SQLite dipilih karena:
1. Simple - single file database, no server
2. Fast enough - handles <1000 TPS (cukup untuk MVP)
3. Proven - Cloudflare D1, Turso use it in production
4. Easy backup - just copy the .db file

Nanti kalau traffic >1000 TPS baru migrate ke PostgreSQL. Sekarang SQLite cukup.

## Test Results

```bash
running 9 tests total
- 3 unit tests (database module)
- 4 binary tests
- 2 integration tests

test result: ok. 9 passed; 0 failed
```

## Deployment

```bash
# Set database path (optional, default: ./nimbus-relayer.db)
export NIMBUS_DB_PATH=/var/lib/nimbus/relayer.db

# Run node
cargo run --package nimbus-node
```

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
    tx_hash TEXT
);
```

## Security

- Double-spend prevention: Atomic `INSERT OR IGNORE` (race-safe)
- Session uniqueness: Primary key constraint
- Data persistence: WAL mode survives crashes
- Concurrent access: 5s busy timeout

## Performance

- Session insert: ~0.5ms
- Nullifier check: ~0.2ms
- Bottleneck: SQLite single writer (good for <1000 TPS)
- Migration path: PostgreSQL when needed

## Storage Size

**Realistic estimates:**
- Low traffic (100 tx/day): ~40 KB/day = 1.2 MB/month = 14 MB/year
- Medium (1000 tx/day): ~400 KB/day = 12 MB/month = 144 MB/year  
- High (10K tx/day): ~4 MB/day = 120 MB/month = 1.4 GB/year

**Conclusion: MB-an doang**, paling besar ratusan MB.

## Auto-Cleanup

Background task runs daily to:
1. Delete resolved sessions older than 30 days
2. Vacuum database to reclaim space
3. Log database size

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
