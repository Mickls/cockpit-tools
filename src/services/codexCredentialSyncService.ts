import { invoke } from '@tauri-apps/api/core';

export interface CodexCredentialSyncSettings {
  enabled: boolean;
  serverUrl: string;
  deviceId?: string | null;
  vaultId?: string | null;
  lastSyncAt?: number | null;
  lastError?: string | null;
}
export const loadCodexCredentialSyncSettings = () => invoke<CodexCredentialSyncSettings>('codex_credential_sync_load');
export const saveCodexCredentialSyncSettings = (settings: CodexCredentialSyncSettings) => invoke<CodexCredentialSyncSettings>('codex_credential_sync_save', { settings });
export function normalizeCodexCredentialSyncUrl(value: string): string { return value.trim().replace(/\/+$/, ''); }
