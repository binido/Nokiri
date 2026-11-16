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

pub async fn network_warnings() -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        let (proxies, routes) = tokio::join!(
            network_output("/usr/sbin/scutil", &["--proxy"]),
            network_output("/usr/sbin/netstat", &["-rn", "-f", "inet"]),
        );
        let mut warnings = Vec::new();
        match proxies {
            Some(output) if system_proxy_enabled(&output) => warnings.push(
                "System proxy or PAC is enabled. Other apps may use it instead of Nokiri; configure them for Nokiri's local port.".into(),
            ),
            None => warnings.push("Could not inspect system proxy settings.".into()),
            _ => {}
        }
        match routes {
            Some(output) if tunnel_routes_present(&output) => warnings.push(
                "VPN/tunnel routes are active. Connections to your server or DNS may use another VPN. Nokiri does not bypass VPN routing.".into(),
            ),
            None => warnings.push("Could not inspect IPv4 routes for VPN conflicts.".into()),
            _ => {}
        }
        warnings
    }
    #[cfg(not(target_os = "macos"))]
    Vec::new()
}

#[cfg(target_os = "macos")]
async fn network_output(program: &str, args: &[&str]) -> Option<String> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        tokio::process::Command::new(program)
            .args(args)
            .env("LC_ALL", "C")
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(target_os = "macos")]
fn system_proxy_enabled(output: &str) -> bool {
    output.lines().any(|line| {
        let Some((key, value)) = line.split_once(':') else {
            return false;
        };
        matches!(
            key.trim(),
            "HTTPEnable"
                | "HTTPSEnable"
                | "SOCKSEnable"
                | "ProxyAutoConfigEnable"
                | "ProxyAutoDiscoveryEnable"
        ) && value.trim() == "1"
    })
}

#[cfg(target_os = "macos")]
fn tunnel_routes_present(output: &str) -> bool {
    output.lines().any(|line| {
        line.split_whitespace().nth(3).is_some_and(|interface| {
            ["utun", "tun", "tap", "ppp", "ipsec"].iter().any(|prefix| {
                interface.strip_prefix(prefix).is_some_and(|suffix| {
                    !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
                })
            })
        })
    })
}

#[cfg(all(test, target_os = "macos"))]
mod network_tests {
    use super::*;

    #[test]
    fn enabled_system_proxies_and_pac_are_reported() {
        for key in [
            "HTTPEnable",
            "HTTPSEnable",
            "SOCKSEnable",
            "ProxyAutoConfigEnable",
            "ProxyAutoDiscoveryEnable",
        ] {
            assert!(system_proxy_enabled(&format!(
                "<dictionary> {{\n  {key} : 1\n}}"
            )));
        }
    }

    #[test]
    fn disabled_proxies_and_ftp_passive_are_not_conflicts() {
        assert!(!system_proxy_enabled(
            "FTPPassive : 1\nHTTPEnable : 0\nHTTPSEnable : 0\nSOCKSEnable : 0\nProxyAutoConfigEnable : 0\nProxyAutoDiscoveryEnable : 0"
        ));
    }

    #[test]
    fn split_and_full_tunnel_routes_are_reported() {
        for destination in ["10/8", "default", "0/1", "128.0/1"] {
            for interface in ["utun4", "tun0", "tap1", "ppp0", "ipsec2"] {
                assert!(tunnel_routes_present(&format!(
                    "Destination Gateway Flags Netif Expire\n{destination} 192.0.2.1 UGSc {interface}"
                )));
            }
        }
    }

    #[test]
    fn physical_and_virtual_lan_routes_are_not_vpn_conflicts() {
        assert!(!tunnel_routes_present(
            "Routing tables\nInternet:\nDestination Gateway Flags Netif Expire\ndefault 192.0.2.1 UGScg en0\n127 127.0.0.1 UCS lo0\n192.0.2 link#25 UCS bridge100 !\n192.0.2.1 utun4 UHLWI en0 100"
        ));
    }
}
