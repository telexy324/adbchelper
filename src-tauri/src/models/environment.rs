use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentProfile {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub kubernetes_enabled: bool,
    pub elk_enabled: bool,
    pub ssh_enabled: bool,
    pub nacos_enabled: bool,
    pub redis_enabled: bool,
}
