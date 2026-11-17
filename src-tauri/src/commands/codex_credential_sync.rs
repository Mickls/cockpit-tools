use crate::modules::codex_credential_sync::{self,CodexCredentialSyncSettings};
#[tauri::command]
pub fn codex_credential_sync_load() -> Result<CodexCredentialSyncSettings,String>{codex_credential_sync::load()}
#[tauri::command]
pub fn codex_credential_sync_save(settings: CodexCredentialSyncSettings)->Result<CodexCredentialSyncSettings,String>{codex_credential_sync::save(settings)}
