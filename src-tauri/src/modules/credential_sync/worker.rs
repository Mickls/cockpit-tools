use super::{crypto,sdk,load,update_settings,enqueue_upload};
use crate::modules::codex_account as accounts;
use crate::models::codex::{CodexAccount,CodexTokens};
use base64::{engine::general_purpose::STANDARD,Engine};
use reqwest::Method;
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{collections::{HashMap,HashSet},sync::{atomic::{AtomicBool,Ordering},Mutex,LazyLock},time::{Duration,Instant}};
static STARTED:AtomicBool=AtomicBool::new(false);
static ATTEMPTS:LazyLock<Mutex<HashSet<String>>>=LazyLock::new(||Mutex::new(HashSet::new()));
static ACTIVE:LazyLock<Mutex<HashSet<String>>>=LazyLock::new(||Mutex::new(HashSet::new()));
static FLUSH:tokio::sync::Mutex<()>=tokio::sync::Mutex::const_new(());
#[derive(Serialize,Deserialize)]struct Credential { identity:Vec<u8>,tokens:CodexTokens }
fn fingerprint(a:&CodexAccount)->String{format!("{:x}",Sha256::digest(serde_json::to_vec(&a.tokens).unwrap_or_default()))}
fn account_key(a:&CodexAccount)->Result<String,String>{Ok(crypto::account_key(&sdk::keys(&sdk::secret()?)?,&accounts::credential_sync_identity(a)?))}
fn status(id:&str,message:&str){let _=update_settings(|s|{if s.recovery_status.len()>128{s.recovery_status.clear();}s.recovery_status.insert(id.into(),message.into());});}
pub(super) fn clear_recovery_epoch(id:&str){if let Ok(mut ids)=ATTEMPTS.lock(){ids.remove(id);}}
pub fn start_worker(){
 #[cfg(test)] { return; }
 if STARTED.swap(true,Ordering::SeqCst){return;}tauri::async_runtime::spawn(async{
 loop{
  if let Ok(settings)=load(){if settings.enabled&&!settings.server_url.is_empty(){
   if settings.device_id.is_some(){
    let result=cycle().await;
    if let Err(error)=result{let _=update_settings(|s|s.last_error=Some(error));}
   }
  }}
  tokio::time::sleep(Duration::from_secs(5)).await;
 }
});}
async fn cycle()->Result<(),String>{
 if !sdk::finish_pairing().await?{return Ok(());}
 sdk::call(Method::POST,"/v1/devices/heartbeat",None).await?;
 let _=sdk::devices().await?;
 sdk::sync_rotation().await?;
 sdk::refresh_recovery().await?;
 assist().await?;
 if let Some(pending)=load()?.pending_uploads.first().cloned(){
  let current=accounts::credential_sync_snapshot(&pending.account_id,false);
  match current {
   Ok(account) if account.token_generation==pending.token_generation=>{upload(&account).await?;remove_queued(&pending.account_id,pending.token_generation)?;},
   // A delayed upload may not resurrect a deletion or upload a later automatic refresh.
   _=>{remove_queued(&pending.account_id,pending.token_generation)?;}
  }
 }
 update_settings(|s|{s.last_sync_at=Some(chrono::Utc::now().timestamp());s.last_error=None;})?;Ok(())
}
fn remove_queued(id:&str,generation:u64)->Result<(),String>{update_settings(|s|s.pending_uploads.retain(|p|p.account_id!=id||p.token_generation!=generation))?;Ok(())}
pub(super) async fn upload(account:&CodexAccount)->Result<(),String>{
 let key=account_key(account)?;let path=format!("/v1/accounts/{key}/backup");
 let revision=match sdk::call(Method::GET,&format!("/v1/accounts/{key}/backup-meta"),None).await{Ok(v)=>v["revision"].as_i64().ok_or("备份版本缺失")?,Err(e)if e.contains("HTTP 404")=>0,Err(e)=>return Err(e)};
 let snapshot=accounts::credential_sync_snapshot(&account.id,false)?;
 if snapshot.token_generation!=account.token_generation||fingerprint(&snapshot)!=fingerprint(account){return Err("上传前本地凭据已变化".into());}
 let plaintext=serde_json::to_vec(&Credential{identity:accounts::credential_sync_identity(account)?,tokens:account.tokens.clone()}).map_err(|_|"凭据编码失败")?;
 let state=sdk::secret()?;let signing=super::load_or_create_device_signing_key()?;
 let header=crypto::Header{protocol:1,vault_id:load()?.vault_id.ok_or("尚未登记设备")?,account_key:key,key_epoch:state.epoch,sender:super::device_id(&signing),parent_revision:revision,recipient:None,recovery_id:None};
 let envelope=crypto::seal(&sdk::keys(&state)?,&signing,header,&plaintext)?;
 sdk::call(Method::PUT,&path,Some(json!({"expected_revision":revision,"envelope":envelope}))).await?;Ok(())
}
pub async fn manual_upload(id:String)->Result<(),String>{if !sdk::ready(){return Err("请先完成设备配对并启用同步".into());}let snapshot=accounts::credential_sync_snapshot(&id,true)?;enqueue_upload(&id,snapshot.token_generation)?;let _g=FLUSH.lock().await;upload(&snapshot).await?;remove_queued(&id,snapshot.token_generation)}
pub async fn manual_sync(id:String)->Result<bool,String>{ATTEMPTS.lock().map_err(|_|"恢复状态锁不可用")?.remove(&id);recover(&id).await}
pub async fn recover_if_authorized_failure(id:&str,error:&str)->bool{
 if !explicit_auth_error(error){return false;}
 recover(id).await.unwrap_or(false)
}
fn explicit_auth_error(error:&str)->bool{
 let e=error.to_ascii_lowercase();["invalid_grant","refresh_token_expired","refresh_token_invalidated","token_revoked","token_invalidated","missing_refresh_token"].iter().any(|code|e.contains(code))
}
struct Active(String);impl Drop for Active{fn drop(&mut self){if let Ok(mut ids)=ACTIVE.lock(){ids.remove(&self.0);}}}
async fn recover(id:&str)->Result<bool,String>{
 match tokio::time::timeout(Duration::from_secs(60),recover_inner(id)).await {
  Ok(Ok(result))=>Ok(result),
  Ok(Err(error))=>{status(id,"本轮恢复失败，等待重启或手动同步");Err(error)},
  Err(_)=>{status(id,"本轮恢复超时，等待重启或手动同步");Err("凭据恢复超过总时限".into())}
 }
}
async fn recover_inner(id:&str)->Result<bool,String>{
 if !sdk::ready(){return Ok(false);}
 {let mut ids=ACTIVE.lock().map_err(|_|"恢复锁不可用")?;if !ids.insert(id.into()){return Ok(false);}}
 let _active=Active(id.into());
 {let mut ids=ATTEMPTS.lock().map_err(|_|"恢复状态锁不可用")?;if !ids.insert(id.into()){return Ok(false);}}
 status(id,"正在检查本机凭据");
 let original=accounts::credential_sync_snapshot(id,true)?;
 if accounts::credential_sync_probe(&original).await.is_ok(){status(id,"本机已有有效凭据");return Ok(true);}
 let key=account_key(&original)?;let started=Instant::now();let mut failed=HashSet::new();failed.insert(fingerprint(&original));
 status(id,"正在请求在线设备协助");
 let request=sdk::call(Method::POST,"/v1/recoveries",Some(json!({"account_key":key}))).await?;
 let recovery=request["recovery_id"].as_str().ok_or("恢复请求标识缺失")?.to_string();
 while started.elapsed()<Duration::from_secs(40){
  let response=sdk::call(Method::GET,&format!("/v1/recoveries/{recovery}"),None).await?;
  if response["state"].as_str()!=Some("pending"){break;}
  if let Some(offers)=response["offers"].as_array(){for offer in offers{
   let Some(task)=offer["task_id"].as_str()else{continue};
   let accepted=try_candidate(&original,&key,&offer["envelope"],Some(&recovery),&mut failed).await;
   if accepted.is_ok(){
    let _=sdk::call(Method::POST,&format!("/v1/recoveries/{recovery}/result"),Some(json!({"task_id":task,"outcome":"recovered"}))).await;
    // Explicit recovery confirmation is the one on-demand backup promotion path.
    if let Ok(current)=accounts::credential_sync_snapshot(id,false){let _=upload(&current).await;}
    status(id,"已从在线设备恢复；运行中实例可能需要重启");return Ok(true);
   }
   let _=sdk::call(Method::POST,&format!("/v1/recoveries/{recovery}/result"),Some(json!({"task_id":task,"outcome":"candidate_rejected"}))).await;
  }}
  if response["peers_pending"].as_i64()==Some(0){break;}
  tokio::time::sleep(Duration::from_secs(1)).await;
 }
 status(id,"正在检查服务器密文备份");
 let backup=sdk::call(Method::GET,&format!("/v1/accounts/{key}/backup"),None).await?;
 let result=try_candidate(&original,&key,&backup["envelope"],None,&mut failed).await;
 let _=sdk::call(Method::POST,&format!("/v1/recoveries/{recovery}/result"),Some(json!({"outcome":if result.is_ok(){"recovered"}else{"exhausted"}}))).await;
 if result.is_ok(){status(id,"已从备份恢复；运行中实例可能需要重启");Ok(true)}else{status(id,"本轮恢复失败，等待重启或手动同步");Ok(false)}
}
async fn try_candidate(original:&CodexAccount,key:&str,value:&Value,recovery:Option<&str>,failed:&mut HashSet<String>)->Result<(),String>{
 let envelope:crypto::EncryptedEnvelope=serde_json::from_value(value.clone()).map_err(|_|"候选密文无效")?;
 let header=&envelope.header;let settings=load()?;
 if header.vault_id!=settings.vault_id.as_deref().unwrap_or("")||header.account_key!=key{return Err("候选账号身份不一致".into());}
 let own=super::device_id(&super::load_or_create_device_signing_key()?);
 if let Some(request)=recovery{if header.recipient.as_deref()!=Some(&own)||header.recovery_id.as_deref()!=Some(request){return Err("候选恢复请求或接收设备不一致".into());}}
 else if header.recipient.is_some()||header.recovery_id.is_some(){return Err("备份包含其他恢复请求专属凭据".into());}
 let state=sdk::secret()?;if header.key_epoch!=state.epoch{return Err("候选密钥代次不一致".into());}
 let signer=sdk::trusted_key(&header.sender)?;let data=crypto::open(&sdk::keys(&state)?,&signer,header,&envelope)?;
 let credential:Credential=serde_json::from_slice(&data).map_err(|_|"候选凭据格式无效")?;
 if credential.identity!=accounts::credential_sync_identity(original)?{return Err("候选稳定账号身份不一致".into());}
 let candidate=accounts::credential_sync_candidate(original,credential.tokens)?;
 if !failed.insert(fingerprint(&candidate)){return Err("已尝试或重复的候选凭据".into());}
 accounts::credential_sync_probe(&candidate).await?;
 let saved=accounts::credential_sync_commit(original,&candidate).await?;
 // This path updates only the original account. It does not replay inference or tools.
 tauri::async_runtime::spawn(async move {let _=accounts::sync_bound_oauth_consumers_after_reauth(&saved.id).await;});Ok(())
}
async fn assist()->Result<(),String>{
 let response=sdk::call(Method::GET,"/v1/devices/self/tasks",None).await?;
 let Some(tasks)=response["tasks"].as_array()else{return Ok(());};
 for task in tasks{
  let (Some(recovery),Some(task_id),Some(key),Some(recipient))=(task["recovery_id"].as_str(),task["task_id"].as_str(),task["account_key"].as_str(),task["requester"].as_str())else{continue};
  let mut envelope=None;
  for summary in accounts::load_account_index().accounts {
   let Ok(snapshot)=accounts::credential_sync_snapshot(&summary.id,true)else{continue};
   if account_key(&snapshot).as_deref()!=Ok(key){continue;}
   if accounts::credential_sync_probe(&snapshot).await.is_err(){break;}
   let data=serde_json::to_vec(&Credential{identity:accounts::credential_sync_identity(&snapshot)?,tokens:snapshot.tokens.clone()}).map_err(|_|"协助凭据编码失败")?;
   let state=sdk::secret()?;let signing=super::load_or_create_device_signing_key()?;
   let header=crypto::Header{protocol:1,vault_id:load()?.vault_id.ok_or("同步库未配置")?,account_key:key.into(),key_epoch:state.epoch,sender:super::device_id(&signing),parent_revision:0,recipient:Some(recipient.into()),recovery_id:Some(recovery.into())};
   envelope=Some(crypto::seal(&sdk::keys(&state)?,&signing,header,&data)?);break;
  }
  sdk::call(Method::POST,&format!("/v1/recoveries/{recovery}/offers"),Some(json!({"task_id":task_id,"envelope":envelope}))).await?;
 }
 Ok(())
}
#[cfg(test)]mod tests{use super::*;#[test]fn only_explicit_auth_errors_trigger_recovery(){assert!(explicit_auth_error("invalid_grant"));assert!(explicit_auth_error("token_revoked"));for error in ["401 Unauthorized","403 Forbidden","429 rate limit","500 server error","timeout","proxy failure"]{assert!(!explicit_auth_error(error));}}}
