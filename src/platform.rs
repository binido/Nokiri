use crate::model::State;
use anyhow::{Context, Result, bail};
use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit,
    aead::{Aead, AeadCore, OsRng},
};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

pub trait SecretStore: Send {
    fn key(&self, create: bool) -> Result<Vec<u8>>;
}
pub struct SystemSecrets;
impl SecretStore for SystemSecrets {
    fn key(&self, create: bool) -> Result<Vec<u8>> {
        let entry = keyring::Entry::new("io.github.binido.nokiri", "vault-key-v1")?;
        match entry.get_secret() {
            Ok(key) => Ok(key),
            Err(keyring::Error::NoEntry) if create => {
                let key = ChaCha20Poly1305::generate_key(&mut OsRng).to_vec();
                entry
                    .set_secret(&key)
                    .context("Could not store encryption key in the system credential store")?;
                Ok(key)
            }
            Err(_) => bail!(
                "Could not unlock the system credential store. Unlock Keychain / Secret Service and reopen Nokiri. Existing data has not been changed."
            ),
        }
    }
}

pub struct Vault {
    path: PathBuf,
    secrets: Box<dyn SecretStore>,
}
impl Vault {
    pub fn new(path: PathBuf, secrets: Box<dyn SecretStore>) -> Self {
        Self { path, secrets }
    }
    pub fn load(&self) -> Result<State> {
        if !self.path.exists() {
            return Ok(State::default());
        }
        let bytes = fs::read(&self.path)?;
        if bytes.len() < 12 {
            bail!(
                "The encrypted catalog is truncated. Restore a backup; the file was not changed."
            );
        }
        let key = self.secrets.key(false)?;
        let cipher = ChaCha20Poly1305::new_from_slice(&key)
            .map_err(|_| anyhow::anyhow!("Invalid credential-store key"))?;
        let plaintext = cipher.decrypt(bytes[..12].into(), &bytes[12..]).map_err(|_| anyhow::anyhow!("Could not decrypt catalog. Its key may be missing or the file damaged. The file was not changed."))?;
        serde_json::from_slice(&plaintext).context("Unsupported or damaged catalog")
    }
    pub fn save(&self, state: &State) -> Result<()> {
        let key = self.secrets.key(!self.path.exists())?;
        let cipher = ChaCha20Poly1305::new_from_slice(&key)
            .map_err(|_| anyhow::anyhow!("Invalid credential-store key"))?;
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let plaintext = serde_json::to_vec(state)?;
        let encrypted = cipher
            .encrypt(&nonce, plaintext.as_slice())
            .map_err(|_| anyhow::anyhow!("Could not encrypt catalog"))?;
        let mut bytes = nonce.to_vec();
        bytes.extend(encrypted);
        atomic_write(&self.path, &bytes)
    }
}

pub fn data_dir() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("io.github", "binido", "Nokiri")
        .context("Could not locate application data directory")?;
    let path = dirs.data_local_dir().to_path_buf();
    fs::create_dir_all(&path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(path)
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("Missing parent directory")?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}
pub fn instance_lock(path: &Path) -> Result<File> {
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.join("instance.lock"))?;
    fs2::FileExt::try_lock_exclusive(&lock)
        .context("Nokiri is already running. Open its existing window.")?;
    Ok(lock)
}
