import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { loadCodexCredentialSyncSettings, saveCodexCredentialSyncSettings, normalizeCodexCredentialSyncUrl, type CodexCredentialSyncSettings as SyncSettings } from '../../services/codexCredentialSyncService';

export function CodexCredentialSyncSettings() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<SyncSettings>({ enabled:false, serverUrl:'', deviceId:null, vaultId:null, lastSyncAt:null, lastError:null });
  const [busy, setBusy] = useState(false);
  useEffect(() => { void loadCodexCredentialSyncSettings().then(setSettings).catch(() => undefined); }, []);
  const save = async (next: SyncSettings) => { setSettings(next); setBusy(true); try { setSettings(await saveCodexCredentialSyncSettings(next)); } finally { setBusy(false); } };
  return <div className="settings-row">
    <div className="row-label"><div className="row-title">{t('codex.credentialSync.title','Codex 凭据同步')}</div><div className="row-desc">{t('codex.credentialSync.desc','个人多设备授权恢复；服务器只保存加密数据。普通 Token 刷新不会上传。')}</div></div>
    <div className="row-control" style={{ minWidth: 280 }}>
      <input className="input input-bordered input-sm" value={settings.serverUrl} placeholder="https://100.100.1.1:18444" disabled={busy} onChange={e=>setSettings({...settings,serverUrl:normalizeCodexCredentialSyncUrl(e.target.value)})} onBlur={()=>void save({...settings,serverUrl:normalizeCodexCredentialSyncUrl(settings.serverUrl)})} />
      <label className="switch" style={{ marginLeft: 8 }}><input type="checkbox" checked={settings.enabled} disabled={busy || !settings.serverUrl} onChange={e=>void save({...settings,enabled:e.target.checked})}/><span className="slider" /></label>
    </div>
  </div>;
}
