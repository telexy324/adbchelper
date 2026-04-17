use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct SshToolLocator<'a> {
    pub resource_dir: Option<&'a Path>,
    pub executable_dir: Option<&'a Path>,
}

pub struct SshToolResolution {
    program: PathBuf,
    pub source_label: String,
}

impl SshToolResolution {
    pub fn command(&self) -> Command {
        Command::new(&self.program)
    }

    pub fn display_path(&self) -> String {
        self.program.display().to_string()
    }
}

pub fn prepare_private_key_for_ssh(private_key_path: &str, app_data_dir: &Path) -> Result<PathBuf, String> {
    let source = PathBuf::from(private_key_path);
    if !source.exists() {
        return Err(format!("Configured privateKeyPath does not exist: {private_key_path}"));
    }

    let runtime_dir = app_data_dir.join("runtime").join("ssh_keys");
    fs::create_dir_all(&runtime_dir)
        .map_err(|error| format!("Failed to prepare SSH runtime key directory: {error}"))?;

    let mut hasher = DefaultHasher::new();
    private_key_path.hash(&mut hasher);
    let ext = source.extension().and_then(|value| value.to_str()).unwrap_or("key");
    let staged = runtime_dir.join(format!("imported_{:x}.{ext}", hasher.finish()));

    fs::copy(&source, &staged)
        .map_err(|error| format!("Failed to copy SSH private key into app runtime: {error}"))?;

    tighten_private_key_permissions(&staged)?;
    Ok(staged)
}

pub fn resolve_ssh(locator: SshToolLocator<'_>, custom_ssh_path: Option<&str>) -> Result<SshToolResolution, String> {
    resolve_named_tool(locator, custom_ssh_path, "ssh", bundled_ssh_names(), system_ssh_name())
}

pub fn resolve_ssh_keygen(
    locator: SshToolLocator<'_>,
    custom_ssh_path: Option<&str>,
) -> Result<SshToolResolution, String> {
    if let Some(sibling) = resolve_sibling_tool(custom_ssh_path, keygen_name())? {
        return Ok(sibling);
    }
    resolve_named_tool(
        locator,
        None,
        "ssh-keygen",
        bundled_keygen_names(),
        system_keygen_name(),
    )
}

pub fn resolve_ssh_keyscan(
    locator: SshToolLocator<'_>,
    custom_ssh_path: Option<&str>,
) -> Result<SshToolResolution, String> {
    if let Some(sibling) = resolve_sibling_tool(custom_ssh_path, keyscan_name())? {
        return Ok(sibling);
    }
    resolve_named_tool(
        locator,
        None,
        "ssh-keyscan",
        bundled_keyscan_names(),
        system_keyscan_name(),
    )
}

fn resolve_named_tool(
    locator: SshToolLocator<'_>,
    custom_path: Option<&str>,
    tool_label: &str,
    bundled_names: &'static [&'static str],
    system_name: &'static str,
) -> Result<SshToolResolution, String> {
    if let Some(path) = custom_path.map(str::trim).filter(|value| !value.is_empty()) {
        let custom = PathBuf::from(path);
        if !custom.exists() {
            return Err(format!("Configured {tool_label} path does not exist: {path}"));
        }
        return Ok(SshToolResolution {
            program: custom,
            source_label: format!("custom {tool_label} path ({path})"),
        });
    }

    for base_dir in [locator.resource_dir, locator.executable_dir].into_iter().flatten() {
        for name in bundled_names {
            let candidate = base_dir.join("tools").join(name);
            if candidate.exists() {
                return Ok(SshToolResolution {
                    program: candidate,
                    source_label: "bundled tools directory".to_string(),
                });
            }
        }
    }

    for candidate in known_windows_locations(system_name) {
        if candidate.exists() {
            return Ok(SshToolResolution {
                program: candidate,
                source_label: "standard Windows OpenSSH location".to_string(),
            });
        }
    }

    Ok(SshToolResolution {
        program: PathBuf::from(system_name),
        source_label: "system PATH".to_string(),
    })
}

fn resolve_sibling_tool(custom_ssh_path: Option<&str>, sibling_name: &str) -> Result<Option<SshToolResolution>, String> {
    let Some(path) = custom_ssh_path.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let custom = PathBuf::from(path);
    if !custom.exists() {
        return Err(format!("Configured ssh path does not exist: {path}"));
    }
    let Some(parent) = custom.parent() else {
        return Ok(None);
    };
    let sibling = parent.join(sibling_name);
    if sibling.exists() {
        return Ok(Some(SshToolResolution {
            program: sibling,
            source_label: format!("custom ssh sibling tool ({})", parent.display()),
        }));
    }
    Ok(None)
}

fn bundled_ssh_names() -> &'static [&'static str] {
    if cfg!(target_os = "windows") {
        &["ssh.exe", "ssh"]
    } else {
        &["ssh", "ssh.exe"]
    }
}

fn bundled_keygen_names() -> &'static [&'static str] {
    if cfg!(target_os = "windows") {
        &["ssh-keygen.exe", "ssh-keygen"]
    } else {
        &["ssh-keygen", "ssh-keygen.exe"]
    }
}

fn bundled_keyscan_names() -> &'static [&'static str] {
    if cfg!(target_os = "windows") {
        &["ssh-keyscan.exe", "ssh-keyscan"]
    } else {
        &["ssh-keyscan", "ssh-keyscan.exe"]
    }
}

fn system_ssh_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "ssh.exe"
    } else {
        "ssh"
    }
}

fn system_keygen_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "ssh-keygen.exe"
    } else {
        "ssh-keygen"
    }
}

fn system_keyscan_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "ssh-keyscan.exe"
    } else {
        "ssh-keyscan"
    }
}

fn keygen_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "ssh-keygen.exe"
    } else {
        "ssh-keygen"
    }
}

fn keyscan_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "ssh-keyscan.exe"
    } else {
        "ssh-keyscan"
    }
}

fn tighten_private_key_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|error| format!("Failed to inspect SSH private key copy: {error}"))?
            .permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(path, permissions)
            .map_err(|error| format!("Failed to set SSH private key permissions: {error}"))?;
        return Ok(());
    }

    #[cfg(windows)]
    {
        let icacls = resolve_icacls_path();
        let mut command = Command::new(&icacls);
        command.arg(path);
        command.arg("/inheritance:r");
        if let Some(user) = std::env::var_os("USERNAME") {
            command.arg("/grant:r");
            command.arg(format!("{}:R", PathBuf::from(user).display()));
        }
        let output = command
            .output()
            .map_err(|error| format!("Failed to run icacls for SSH private key: {error}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if stderr.is_empty() {
                "Failed to tighten SSH private key permissions on Windows.".to_string()
            } else {
                format!("Failed to tighten SSH private key permissions: {stderr}")
            });
        }
        return Ok(());
    }

    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(windows)]
fn resolve_icacls_path() -> PathBuf {
    for env_key in ["WINDIR", "SYSTEMROOT"] {
        if let Some(base) = std::env::var_os(env_key) {
            let candidate = PathBuf::from(base).join("System32").join("icacls.exe");
            if candidate.exists() {
                return candidate;
            }
        }
    }
    PathBuf::from("icacls.exe")
}

fn known_windows_locations(system_name: &str) -> Vec<PathBuf> {
    if !cfg!(target_os = "windows") {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    let file_name = PathBuf::from(system_name)
        .file_name()
        .map(|value| value.to_owned());

    let Some(file_name) = file_name else {
        return candidates;
    };

    for env_key in ["WINDIR", "SYSTEMROOT"] {
        if let Some(base) = std::env::var_os(env_key) {
            let base = PathBuf::from(base);
            candidates.push(base.join("System32").join("OpenSSH").join(&file_name));
            candidates.push(base.join("Sysnative").join("OpenSSH").join(&file_name));
        }
    }

    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        let base = PathBuf::from(program_files);
        candidates.push(base.join("OpenSSH").join(&file_name));
    }

    candidates
}
