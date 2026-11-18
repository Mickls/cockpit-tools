use ed25519_dalek::SigningKey;
use rand::{rngs::OsRng, RngCore};
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
fn device_key_path() -> Result<PathBuf, String> { Ok(crate::modules::account::get_data_dir()?.join("codex-credential-sync-device.json")) }
#[derive(Debug, Clone, Serialize, Deserialize)] struct DeviceKey { bytes: Vec<u8> }
pub fn load_or_create_device_signing_key() -> Result<SigningKey, String> {
    let p=device_key_path()?;
    if let Ok(content)=fs::read_to_string(&p) {
        let (value,_)=crate::modules::secure_account_storage::deserialize_account_file::<DeviceKey>(&p,&content)?;
        let bytes:[u8;32]=value.bytes.try_into().map_err(|_|"同步设备私钥长度无效".to_string())?;
        return Ok(SigningKey::from_bytes(&bytes));
    }
    let mut bytes=[0u8;32];OsRng.fill_bytes(&mut bytes);
    let value=DeviceKey{bytes:bytes.to_vec()};
    let encrypted=crate::modules::secure_account_storage::serialize_account_file("codex-credential-sync-device",&value)?;
    crate::modules::atomic_write::write_string_atomic(&p,&encrypted).map_err(|e|e.to_string())?;
    Ok(SigningKey::from_bytes(&bytes))
}
pub fn load() -> Result<CodexCredentialSyncSettings, String> { let p=path()?; match fs::read_to_string(p) { Ok(v)=>serde_json::from_str(&v).map_err(|e|format!("解析凭据同步设置失败: {e}")), Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(Default::default()), Err(e)=>Err(format!("读取凭据同步设置失败: {e}")) } }
pub fn save(mut settings: CodexCredentialSyncSettings) -> Result<CodexCredentialSyncSettings, String> { settings.server_url=settings.server_url.trim().trim_end_matches('/').to_string(); if settings.server_url.len()>2048{return Err("同步服务器地址过长".into())}; let p=path()?; crate::modules::atomic_write::write_string_atomic(&p,&serde_json::to_string_pretty(&settings).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?; Ok(settings) }
#[cfg(test)] mod tests { use super::*; #[test] fn normalizes_server_url(){let mut s=CodexCredentialSyncSettings{server_url:" https://example.test/// ".into(),..Default::default()};s.server_url=s.server_url.trim().trim_end_matches('/').into();assert_eq!(s.server_url,"https://example.test");} }
