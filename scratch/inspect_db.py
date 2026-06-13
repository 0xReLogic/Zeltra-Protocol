import sqlite3

def inspect_db(db_path):
    print(f"\n=== Inspecting {db_path} ===")
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    cursor.execute("SELECT name FROM sqlite_master WHERE type='table';")
    tables = cursor.fetchall()
    print("Tables:", [t[0] for t in tables])
    for table_name in [t[0] for t in tables]:
        cursor.execute(f"PRAGMA table_info({table_name});")
        columns = cursor.fetchall()
        print(f"\nTable '{table_name}' columns:")
        for col in columns:
            print(f"  {col[1]} ({col[2]})")
        
        # Check if there is data
        cursor.execute(f"SELECT COUNT(*) FROM {table_name};")
        count = cursor.fetchone()[0]
        print(f"  Row count: {count}")
        if count > 0:
            cursor.execute(f"SELECT * FROM {table_name} LIMIT 3;")
            rows = cursor.fetchall()
            print("  Sample rows:")
            for row in rows:
                print("   ", row)
    conn.close()

inspect_db("test_leader.db")
inspect_db("nimbus-relayer.db")
