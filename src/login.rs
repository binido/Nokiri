use anyhow::{Context, Result, bail};
use std::{fs, path::PathBuf};

pub struct LoginItem {
    pub path: PathBuf,
    pub executable: PathBuf,
}

impl LoginItem {
    #[cfg(target_os = "macos")]
    pub fn current() -> Result<Self> {
        let home = directories::BaseDirs::new().context("Could not locate home directory")?;
        Ok(Self {
            path: home
                .home_dir()
                .join("Library/LaunchAgents/io.github.binido.nokiri.plist"),
            executable: std::env::current_exe()?,
        })
    }

    fn plist(&self) -> Result<String> {
        let bundle = self.executable.ancestors().nth(3);
        if !bundle.is_some_and(|p| p.extension().is_some_and(|e| e == "app")) {
            bail!(
                "Launch at login requires the installed Nokiri.app. Move it to Applications and enable this setting there."
            );
        }
        let executable = self
            .executable
            .to_str()
            .context("Application path is not valid UTF-8")?;
        if executable.chars().any(|c| c.is_control()) {
            bail!("Application path contains unsupported control characters.");
        }
        let executable = executable
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;");
        Ok(format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>io.github.binido.nokiri</string>
<key>ProgramArguments</key><array><string>{executable}</string><string>--background</string></array>
<key>RunAtLoad</key><true/>
<key>LimitLoadToSessionType</key><string>Aqua</string>
<key>AssociatedBundleIdentifiers</key><array><string>io.github.binido.nokiri</string></array>
</dict></plist>
"#
        ))
    }

    pub fn enabled(&self) -> Result<bool> {
        match fs::read_to_string(&self.path) {
            Ok(contents) if contents == self.plist()? => Ok(true),
            Ok(_) => bail!(
                "Launch at login points to another app copy. Turn it off and on again from the installed Nokiri.app."
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e).context("Could not read launch-at-login setting"),
        }
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<()> {
        if enabled {
            let plist = self.plist()?;
            fs::create_dir_all(
                self.path
                    .parent()
                    .context("Missing LaunchAgents directory")?,
            )?;
            crate::platform::atomic_write(&self.path, plist.as_bytes())
                .context("Could not enable launch at login")
        } else {
            match fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e).context("Could not disable launch at login"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_registration_round_trips_and_detects_another_app_copy() {
        let dir = tempfile::tempdir().unwrap();
        let mut item = LoginItem {
            path: dir
                .path()
                .join("LaunchAgents/io.github.binido.nokiri.plist"),
            executable: PathBuf::from("/Applications/Nokiri & <test>.app/Contents/MacOS/Nokiri"),
        };
        assert!(!item.enabled().unwrap());
        item.set_enabled(true).unwrap();
        assert!(item.enabled().unwrap());
        let plist = fs::read_to_string(&item.path).unwrap();
        assert!(plist.contains("Nokiri &amp; &lt;test&gt;.app"));
        assert!(plist.contains("<string>--background</string>"));
        assert!(!plist.contains("KeepAlive"));
        #[cfg(target_os = "macos")]
        assert!(
            std::process::Command::new("plutil")
                .arg("-lint")
                .arg(&item.path)
                .status()
                .unwrap()
                .success()
        );
        item.executable = PathBuf::from("/Applications/Nokiri.app/Contents/MacOS/Nokiri");
        assert!(item.enabled().is_err());
        item.set_enabled(true).unwrap();
        assert!(item.enabled().unwrap());
        item.set_enabled(false).unwrap();
        item.set_enabled(false).unwrap();
        assert!(!item.enabled().unwrap());
    }

    #[test]
    fn unpackaged_executable_cannot_replace_registration() {
        let dir = tempfile::tempdir().unwrap();
        let item = LoginItem {
            path: dir.path().join("login.plist"),
            executable: PathBuf::from("/tmp/target/debug/nokiri"),
        };
        fs::write(&item.path, "existing registration").unwrap();
        assert!(item.set_enabled(true).is_err());
        assert_eq!(
            fs::read_to_string(&item.path).unwrap(),
            "existing registration"
        );
    }
}
