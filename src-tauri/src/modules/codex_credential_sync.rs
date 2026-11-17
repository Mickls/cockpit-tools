use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CodexCredentialSyncSettings {
    pub enabled: bool,
    pub server_url: String,
    pub device_id: Option<String>,
    pub vault_id: Option<String>,
    pub last_sync_at: Option<i64>,
    pub last_error: Option<String>,
}
fn path() -> Result<PathBuf, String> { Ok(crate::modules::account::get_data_dir()?.join("codex-credential-sync.json")) }
pub fn load() -> Result<CodexCredentialSyncSettings, String> { let p=path()?; match fs::read_to_string(p) { Ok(v)=>serde_json::from_str(&v).map_err(|e|format!("解析凭据同步设置失败: {e}")), Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(Default::default()), Err(e)=>Err(format!("读取凭据同步设置失败: {e}")) } }
pub fn save(mut settings: CodexCredentialSyncSettings) -> Result<CodexCredentialSyncSettings, String> { settings.server_url=settings.server_url.trim().trim_end_matches('/').to_string(); if settings.server_url.len()>2048{return Err("同步服务器地址过长".into())}; let p=path()?; crate::modules::atomic_write::write_string_atomic(&p,&serde_json::to_string_pretty(&settings).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?; Ok(settings) }
#[cfg(test)] mod tests { use super::*; #[test] fn normalizes_server_url(){let mut s=CodexCredentialSyncSettings{server_url:" https://example.test/// ".into(),..Default::default()};s.server_url=s.server_url.trim().trim_end_matches('/').into();assert_eq!(s.server_url,"https://example.test");} }
