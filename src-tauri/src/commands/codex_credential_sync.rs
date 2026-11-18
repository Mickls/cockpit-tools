use crate::modules::codex_credential_sync::{self,CodexCredentialSyncSettings};
#[tauri::command]
pub fn codex_credential_sync_load() -> Result<CodexCredentialSyncSettings,String>{codex_credential_sync::load()}
#[tauri::command]
pub fn codex_credential_sync_save(settings: CodexCredentialSyncSettings)->Result<CodexCredentialSyncSettings,String>{codex_credential_sync::save(settings)}
#[tauri::command]
pub fn codex_credential_sync_device_public_key() -> Result<String,String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    Ok(STANDARD.encode(codex_credential_sync::load_or_create_device_signing_key()?.verifying_key().as_bytes()))
}
#[tauri::command]
pub async fn codex_credential_sync_enroll(code: String) -> Result<CodexCredentialSyncSettings,String> { crate::modules::codex_credential_sync::enroll(code).await }
#[tauri::command]
pub async fn codex_credential_sync_manual_upload(account_id:String)->Result<(),String>{crate::modules::codex_credential_sync::manual_upload(account_id).await}
#[tauri::command]
pub async fn codex_credential_sync_manual_sync(account_id:String)->Result<bool,String>{crate::modules::codex_credential_sync::manual_sync(account_id).await}
#[tauri::command]
pub async fn codex_credential_sync_create_invite()->Result<String,String>{crate::modules::codex_credential_sync::invite().await}
#[tauri::command]
pub async fn codex_credential_sync_list_pairings()->Result<serde_json::Value,String>{crate::modules::codex_credential_sync::pairings().await}
#[tauri::command]
pub async fn codex_credential_sync_approve_pairing(pairing_id:String)->Result<(),String>{crate::modules::codex_credential_sync::approve(pairing_id).await}
