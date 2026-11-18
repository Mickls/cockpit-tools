// Adapter inside codex_account scope: use existing authority and mutation guards.
pub(crate) fn credential_sync_identity(account:&CodexAccount)->Result<Vec<u8>,String>{
 if account.is_api_key_auth()||account.is_agent_identity_auth()||account.is_web_session_auth(){return Err("此账号类型不参与凭据同步".into());}
 let subject=account.user_id.clone().filter(|s|!s.trim().is_empty()).or_else(||decode_jwt_payload_value(&account.tokens.id_token).and_then(|p|p.get("sub").and_then(|s|s.as_str()).map(str::to_string))).ok_or("账号缺少稳定用户标识，不能自动同步")?;
 let workspace=account.account_id.clone().filter(|s|!s.trim().is_empty()).ok_or("账号缺少稳定工作空间标识，不能自动同步")?;
 serde_json::to_vec(&("openai","chatgpt",subject,workspace,&account.organization_id)).map_err(|_|"账号身份编码失败".into())
}
pub(crate) fn credential_sync_snapshot(id:&str,read_runtime:bool)->Result<CodexAccount,String>{
 let mut account=load_account_checked(id)?.ok_or("账号不存在或已删除")?;let _=credential_sync_identity(&account)?;
 if read_runtime {if sync_account_from_authority_sources(&mut account)?{account=save_account_preserving_egress_proxy(&account)?;}}
 Ok(account)
}
pub(crate) async fn credential_sync_probe(account:&CodexAccount)->Result<(),String>{
 if codex_oauth::is_token_expired(&account.tokens.access_token){return Err("候选 access_token 已过期".into());}
 let response=request_remote_account_check(account).await.map_err(|e|e.message)?;
 validate_account_check_payload(&response,account).map_err(|e|e.message)
}
pub(crate) fn credential_sync_candidate(original:&CodexAccount,tokens:CodexTokens)->Result<CodexAccount,String>{
 let identity=credential_sync_identity(original)?;let mut candidate=original.clone();candidate.tokens=tokens;sync_identity_from_tokens(&mut candidate);
 if credential_sync_identity(&candidate)?!=identity{return Err("候选凭据用户或工作空间不匹配".into());}
 if let Some(id)=extract_chatgpt_account_id_from_access_token(&candidate.tokens.access_token){if Some(id.as_str())!=candidate.account_id.as_deref(){return Err("候选 access_token 身份不匹配".into());}}
 Ok(candidate)
}
pub(crate) async fn credential_sync_commit(original:&CodexAccount,candidate:&CodexAccount)->Result<CodexAccount,String>{
 let account_lock=codex_token_lock_for(&original.id);let _token=account_lock.lock().await;
 let _file=acquire_codex_token_refresh_file_lock(&original.id,"remote credential recovery").await?;
 let _guard=CODEX_ACCOUNT_MUTATION_LOCK.lock().map_err(|_|"账号写入锁不可用")?;
 let mut current=load_account_checked(&original.id)?.ok_or("账号已删除，取消远端恢复")?;
 if current.token_generation!=original.token_generation||account_credential_hash(&current)!=account_credential_hash(original)||credential_sync_identity(&current)?!=credential_sync_identity(candidate)?{return Err("本地凭据已变化，取消过期恢复结果".into());}
 let journal_dir=account::get_data_dir()?.join("credential-sync-recovery");std::fs::create_dir_all(&journal_dir).map_err(|_|"恢复记录目录不可用")?;
 let journal=crate::modules::secure_account_storage::serialize_account_file("codex-sync-precommit",&current)?;
 crate::modules::atomic_write::write_string_atomic(&journal_dir.join(format!("{}.json",current.id)),&journal).map_err(|_|"恢复记录保存失败")?;
 current.tokens=candidate.tokens.clone();mark_token_chain_updated(&mut current);current.quota_error=None;
 save_account_with_tombstone_guard(&current)?;Ok(current)
}
