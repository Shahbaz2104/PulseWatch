//! Generate an Argon2id hash for `ADMIN_PASSWORD_HASH`.
//!
//! The password is read from stdin so it never appears in shell history or the
//! process list:
//!
//! ```text
//! printf '%s\n' "$PASSWORD" | cargo run --bin hash-password
//! ```
//!
//! In `.env`, wrap the output in single quotes: dotenvy interpolates unquoted
//! `$` sequences, which would corrupt the PHC string.

use std::io::{self, BufRead, Write};

use argon2::password_hash::{PasswordHasher, SaltString};
use argon2::Argon2;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprint!("Admin password (read from stdin): ");
    io::stderr().flush()?;

    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let password = line.trim_end_matches(['\r', '\n']);
    if password.is_empty() {
        return Err("password must not be empty".into());
    }
    if password.chars().count() < 12 {
        eprintln!("warning: use at least 12 characters");
    }

    let salt = SaltString::encode_b64(&rand::random::<[u8; 16]>())
        .map_err(|err| std::io::Error::other(format!("generate salt: {err}")))?;
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| std::io::Error::other(format!("hash password: {err}")))?
        .to_string();
    println!("{hash}");
    Ok(())
}
