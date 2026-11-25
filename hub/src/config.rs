//! The command line and the secrets it points to.

use std::{
    fs,
    io::Write,
    net::SocketAddr,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context};
use clap::{Args, Parser, Subcommand};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

#[derive(Parser)]
#[command(name = "rustcraft-hub", about = "The Rustcraft hub", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Runs the hub.
    Serve(ServeArgs),
    /// Writes a new signing key (a hex seed) to a file readable by its owner only.
    GenerateKey {
        /// The file to create; it must not exist.
        path: PathBuf,
    },
}

#[derive(Args)]
pub struct ServeArgs {
    /// The address to listen on.
    #[arg(long, env = "HUB_LISTEN", default_value = "127.0.0.1:8080")]
    pub listen: SocketAddr,
    /// The Postgres connection URL.
    #[arg(long, env = "HUB_DATABASE_URL", conflicts_with = "database_url_file")]
    pub database_url: Option<String>,
    /// A file holding the Postgres connection URL.
    #[arg(long, env = "HUB_DATABASE_URL_FILE")]
    pub database_url_file: Option<PathBuf>,
    /// The public URL of the hub, written in certificates.
    #[arg(long, env = "HUB_PUBLIC_URL")]
    pub public_url: String,
    /// The file holding the hex seed of the signing key (see `generate-key`).
    #[arg(long, env = "HUB_SIGNING_KEY_FILE")]
    pub signing_key_file: PathBuf,
    /// A file holding the secret that keys the sign-up address hashes: at least 32 characters.
    #[arg(long, env = "HUB_IP_SECRET_FILE")]
    pub ip_secret_file: PathBuf,
    /// The name of the hub, shown to players.
    #[arg(long, env = "HUB_NAME", default_value = "Rustcraft")]
    pub name: String,
}

impl ServeArgs {
    /// The database URL, from the flag or from its file.
    pub fn database_url(&self) -> anyhow::Result<String> {
        match (&self.database_url, &self.database_url_file) {
            (Some(url), _) => Ok(url.clone()),
            (None, Some(path)) => read_secret(path),
            (None, None) => bail!("give --database-url or --database-url-file"),
        }
    }
}

/// Reads a one-line secret file, without the trailing whitespace.
pub fn read_secret(path: &Path) -> anyhow::Result<String> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(text.trim().to_owned())
}

/// Reads the hex seed of a signing key.
pub fn read_signing_key(path: &Path) -> anyhow::Result<SigningKey> {
    let seed = rustcraft_protocol::identity::key_from_hex(&read_secret(path)?)
        .with_context(|| format!("{} must hold 64 hex characters", path.display()))?;
    Ok(SigningKey::from_bytes(&seed))
}

/// Reads the secret of the sign-up hashes.
pub fn read_ip_secret(path: &Path) -> anyhow::Result<Vec<u8>> {
    let secret = read_secret(path)?;
    if secret.len() < 32 {
        bail!("{} must hold at least 32 characters", path.display());
    }
    Ok(secret.into_bytes())
}

/// Creates `path` with a new random key, mode 0600. It fails when the file exists.
pub fn generate_key(path: &Path) -> anyhow::Result<()> {
    let key = SigningKey::generate(&mut OsRng);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("creating {}", path.display()))?;
    writeln!(file, "{}", hex::encode(key.to_bytes()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hub-config-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        dir
    }

    #[test]
    fn a_generated_key_is_private_and_reads_back() {
        let dir = dir();
        let path = dir.join("key");
        generate_key(&path).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let key = read_signing_key(&path).unwrap();
        assert_eq!(hex::encode(key.to_bytes()), read_secret(&path).unwrap());
        assert!(
            generate_key(&path).is_err(),
            "an existing key is never overwritten"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_short_ip_secret_is_refused() {
        let dir = dir();
        let path = dir.join("secret");
        fs::write(&path, "short\n").unwrap();
        assert!(read_ip_secret(&path).is_err());
        fs::write(&path, format!("{}\n", "a".repeat(32))).unwrap();
        assert_eq!(read_ip_secret(&path).unwrap().len(), 32);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_database_url_comes_from_the_flag_or_its_file() {
        let dir = dir();
        let path = dir.join("url");
        fs::write(&path, "postgres://db/hub\n").unwrap();
        let args = |url: Option<&str>, file: Option<&Path>| ServeArgs {
            listen: "127.0.0.1:8080".parse().unwrap(),
            database_url: url.map(str::to_owned),
            database_url_file: file.map(Path::to_owned),
            public_url: String::new(),
            signing_key_file: PathBuf::new(),
            ip_secret_file: PathBuf::new(),
            name: String::new(),
        };
        assert_eq!(
            args(None, Some(&path)).database_url().unwrap(),
            "postgres://db/hub"
        );
        assert_eq!(
            args(Some("postgres://x"), None).database_url().unwrap(),
            "postgres://x"
        );
        assert!(args(None, None).database_url().is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
