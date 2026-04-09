use std::path::{Path, PathBuf};
use std::process::Command;

pub struct KubectlLocator<'a> {
    pub resource_dir: Option<&'a Path>,
    pub executable_dir: Option<&'a Path>,
}

pub struct KubectlResolution {
    program: PathBuf,
    pub source_label: String,
}

impl KubectlResolution {
    pub fn command(&self) -> Command {
        Command::new(&self.program)
    }

    pub fn display_path(&self) -> String {
        self.program.display().to_string()
    }
}

pub fn resolve_kubectl(
    locator: KubectlLocator<'_>,
    custom_kubectl_path: Option<&str>,
) -> Result<KubectlResolution, String> {
    if let Some(path) = custom_kubectl_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let custom = PathBuf::from(path);
        if !custom.exists() {
            return Err(format!("Configured kubectlPath does not exist: {path}"));
        }
        return Ok(KubectlResolution {
            program: custom,
            source_label: format!("custom kubectlPath ({path})"),
        });
    }

    for base_dir in [locator.resource_dir, locator.executable_dir].into_iter().flatten() {
        for name in bundled_candidate_names() {
            let candidate = base_dir.join("tools").join(name);
            if candidate.exists() {
                return Ok(KubectlResolution {
                    program: candidate,
                    source_label: "bundled tools directory".to_string(),
                });
            }
        }
    }

    Ok(KubectlResolution {
        program: PathBuf::from(system_kubectl_name()),
        source_label: "system PATH".to_string(),
    })
}

fn bundled_candidate_names() -> &'static [&'static str] {
    if cfg!(target_os = "windows") {
        &["kubectl.exe", "kubectl"]
    } else {
        &["kubectl", "kubectl.exe"]
    }
}

fn system_kubectl_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "kubectl.exe"
    } else {
        "kubectl"
    }
}
