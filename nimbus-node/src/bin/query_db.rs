use rusqlite::{Connection, Result};
use std::env;
use serde_json::{Map, Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: query_db <db_path> <db_key> <query>");
        std::process::exit(1);
    }
    let db_path = &args[1];
    let db_key = &args[2];
    let query = &args[3];

    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "key", db_key)?;
    conn.pragma_update(None, "kdf_iter", "1000")?;

    let mut stmt = conn.prepare(query)?;
    let col_names: Vec<String> = stmt.column_names().into_iter().map(|s| s.to_string()).collect();
    let col_count = stmt.column_count();

    let mut rows = stmt.query([])?;
    let mut results = Vec::new();

    while let Some(row) = rows.next()? {
        let mut row_map = Map::new();
        for i in 0..col_count {
            let col_name = &col_names[i];
            let value_ref = row.get_ref(i)?;
            let value = match value_ref {
                rusqlite::types::ValueRef::Null => Value::Null,
                rusqlite::types::ValueRef::Integer(n) => Value::Number(serde_json::Number::from(n)),
                rusqlite::types::ValueRef::Real(f) => {
                    if let Some(n) = serde_json::Number::from_f64(f) {
                        Value::Number(n)
                    } else {
                        Value::Null
                    }
                }
                rusqlite::types::ValueRef::Text(s) => Value::String(String::from_utf8_lossy(s).into_owned()),
                rusqlite::types::ValueRef::Blob(b) => Value::String(hex::encode(b)),
            };
            row_map.insert(col_name.clone(), value);
        }
        results.push(Value::Object(row_map));
    }

    println!("{}", serde_json::to_string(&results)?);
    Ok(())
}
