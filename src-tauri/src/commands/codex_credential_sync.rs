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
