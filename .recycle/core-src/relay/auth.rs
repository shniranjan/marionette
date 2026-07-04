use rusqlite::params;
use sha2::{Sha256, Digest};
use uuid::Uuid;

/// Result of generating a new registration token
#[derive(Debug, Clone)]
pub struct TokenResult {
    /// The plaintext token — show this ONCE to the admin
    pub plaintext: String,
    /// SHA-256 hash stored in the database
    pub hash: String,
    /// When this token expires
    pub expires_at: String,
}

/// Generate a new registration token for an endpoint.
/// The plaintext is 256 bits of randomness, hex-encoded (64 chars).
/// Only the SHA-256 hash is stored in the database.
pub fn generate_registration_token() -> TokenResult {
    use rand::RngCore;

    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let plaintext = hex::encode(bytes);

    let mut hasher = Sha256::new();
    hasher.update(plaintext.as_bytes());
    let hash = hex::encode(hasher.finalize());

    // 1-hour expiry (SQLite datetime format)
    let expires_at = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(1))
        .unwrap()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    TokenResult {
        plaintext,
        hash,
        expires_at,
    }
}

/// Validate a registration token against the database.
/// Returns Ok(endpoint_id) if valid, Err(reason) if invalid.
pub fn validate_registration_token(
    db: &crate::db::Database,
    token: &str,
) -> Result<String, String> {
    // Hash the presented token
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let token_hash = hex::encode(hasher.finalize());

    let conn = db.conn.lock().map_err(|e| format!("db lock: {}", e))?;

    // Look up the token
    let result: Result<(String, String, bool), _> = conn.query_row(
        "SELECT endpoint_id, expires_at, consumed FROM registration_tokens WHERE token_hash = ?1",
        params![token_hash],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    );

    match result {
        Ok((endpoint_id, expires_at, consumed)) => {
            if consumed {
                return Err("Token already consumed".into());
            }

            // Check expiry (SQLite datetime format: "YYYY-MM-DD HH:MM:SS")
            let expires: chrono::DateTime<chrono::Utc> =
                chrono::NaiveDateTime::parse_from_str(&expires_at, "%Y-%m-%d %H:%M:%S")
                    .map_err(|e| format!("date parse: {}", e))?
                    .and_utc();

            if chrono::Utc::now() > expires {
                return Err("Token expired".into());
            }

            Ok(endpoint_id)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            Err("Invalid token".into())
        }
        Err(e) => Err(format!("db error: {}", e)),
    }
}

/// Store a registration token in the database.
pub fn store_registration_token(
    db: &crate::db::Database,
    endpoint_id: &str,
    token: &TokenResult,
) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| format!("db lock: {}", e))?;

    conn.execute(
        "INSERT INTO registration_tokens (id, token_hash, endpoint_id, expires_at, consumed)
         VALUES (?1, ?2, ?3, ?4, 0)",
        params![
            Uuid::new_v4().to_string(),
            token.hash,
            endpoint_id,
            token.expires_at,
        ],
    )
    .map_err(|e| format!("db insert: {}", e))?;

    Ok(())
}

/// Ensure the registration_tokens table exists
pub fn ensure_schema(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS registration_tokens (
            id          TEXT PRIMARY KEY,
            token_hash  TEXT NOT NULL UNIQUE,
            endpoint_id TEXT NOT NULL,
            expires_at  TEXT NOT NULL,
            consumed    INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_reg_tokens_hash ON registration_tokens(token_hash);
        CREATE INDEX IF NOT EXISTS idx_reg_tokens_endpoint ON registration_tokens(endpoint_id);"
    )?;
    Ok(())
}
