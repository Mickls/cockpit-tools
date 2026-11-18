use super::{crypto,transport,load,save,load_or_create_device_signing_key,device_id,CodexCredentialSyncSettings};
use base64::{engine::general_purpose::{STANDARD,URL_SAFE_NO_PAD},Engine};
use ed25519_dalek::{Signer,Signature,VerifyingKey};
use rand::RngCore;
use reqwest::Method;
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{collections::BTreeMap,fs,path::PathBuf,sync::Mutex};
static SECRET_LOCK:Mutex<()>=Mutex::new(());
#[derive(Clone,Serialize,Deserialize)]
pub struct SecretState {
 pub box_secret:[u8;32],pub keys:Option<Vec<u8>>,pub epoch:i64,
 pub trusted:BTreeMap<String,String>,pub pending:Option<Pending>,
}
#[derive(Clone,Serialize,Deserialize)]pub struct Pending {pub code:String,pub pinned_issuer:String}
#[derive(Clone,Serialize,Deserialize)]pub struct Invite {pub server:String,pub code:String,pub issuer:String}
#[derive(Clone,Serialize,Deserialize)]pub struct Certificate {
 pub issuer:String,pub member:String,pub public_key:String,pub box_public_key:String,
 pub vault:String,pub pairing:String,pub wrapped_hash:String,pub signature:String,
}
fn secret_path()->Result<PathBuf,String>{Ok(crate::modules::account::get_data_dir()?.join("codex-credential-sync-vault.json"))}
pub fn secret()->Result<SecretState,String>{
 let _guard=SECRET_LOCK.lock().map_err(|_|"同步密钥锁不可用")?;let p=secret_path()?;
 match fs::read_to_string(&p) {
  Ok(s)=>crate::modules::secure_account_storage::deserialize_encrypted_snapshot(&s),
  Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{
   let mut box_secret=[0;32];rand::rngs::OsRng.fill_bytes(&mut box_secret);
   let state=SecretState{box_secret,keys:None,epoch:1,trusted:BTreeMap::new(),pending:None};write_secret(&state)?;Ok(state)
  },Err(_)=>Err("无法读取同步密钥，未生成替代密钥".into())
 }
}
fn write_secret(state:&SecretState)->Result<(),String>{
 let text=crate::modules::secure_account_storage::serialize_account_file("codex-sync-vault",state)?;
 crate::modules::atomic_write::write_string_atomic(&secret_path()?,&text).map_err(|_|"同步密钥保存失败".into())
}
pub fn store_secret(state:&SecretState)->Result<(),String>{let _g=SECRET_LOCK.lock().map_err(|_|"同步密钥锁不可用")?;write_secret(state)}
pub fn keys(state:&SecretState)->Result<crypto::VaultKeys,String>{let b=state.keys.as_ref().ok_or("请先完成设备配对")?;if b.len()!=64{return Err("同步库密钥无效".into());}Ok(crypto::VaultKeys{content:b[..32].try_into().unwrap(),lookup:b[32..].try_into().unwrap()})}
pub fn ready()->bool{load().ok().is_some_and(|s|s.enabled&&s.vault_id.is_some())&&secret().ok().is_some_and(|s|s.keys.is_some())}
pub async fn call(method:Method,path:&str,body:Option<Value>)->Result<Value,String>{let settings=load()?;if !settings.enabled{return Err("凭据同步已关闭".into());}let key=load_or_create_device_signing_key()?;transport::request(&settings.server_url,&device_id(&key),&key,method,path,body).await}
fn cert_bytes(c:&Certificate)->Result<Vec<u8>,String>{serde_json::to_vec(&(&c.issuer,&c.member,&c.public_key,&c.box_public_key,&c.vault,&c.pairing,&c.wrapped_hash)).map_err(|_|"配对证明编码失败".into())}
fn public_key(value:&str)->Result<VerifyingKey,String>{let b:[u8;32]=STANDARD.decode(value).map_err(|_|"设备公钥无效")?.try_into().map_err(|_|"设备公钥长度无效")?;VerifyingKey::from_bytes(&b).map_err(|_|"设备公钥无效".into())}
fn verify_cert(c:&Certificate,issuer:&str)->Result<(),String>{let s=STANDARD.decode(&c.signature).map_err(|_|"配对签名无效")?;public_key(issuer)?.verify_strict(&cert_bytes(c)?,&Signature::from_slice(&s).map_err(|_|"配对签名无效")?).map_err(|_|"配对签名验证失败".into())}
pub async fn register(input:String)->Result<CodexCredentialSyncSettings,String>{
 let mut config=load()?;let key=load_or_create_device_signing_key()?;let id=device_id(&key);
 if secret()?.keys.is_some(){return Err("此设备已经配对；请勿重复登记".into());}
 let (code,pin)=if let Some(encoded)=input.trim().strip_prefix("cockpit-sync-v1:") {
  let invite:Invite=serde_json::from_slice(&URL_SAFE_NO_PAD.decode(encoded).map_err(|_|"配对码无效")?).map_err(|_|"配对码无效")?;
  if !config.server_url.is_empty()&&config.server_url!=invite.server{return Err("配对码服务器与设置不一致".into());}
  config.server_url=invite.server;(invite.code,invite.issuer)
 }else{(input.trim().to_string(),String::new())};
 let mut state=secret()?;let box_key=crypto_box::SecretKey::from(state.box_secret);
 let result=transport::enroll(&config.server_url,&code,&id,&STANDARD.encode(key.verifying_key().as_bytes()),&STANDARD.encode(box_key.public_key().as_bytes())).await?;
 config.device_id=Some(id.clone());config.vault_id=Some(result["vault_id"].as_str().ok_or("登记响应缺少同步库标识")?.into());config.enabled=true;
 if result["first_device"].as_bool()==Some(true){
  let k=crypto::VaultKeys::generate();let mut bytes=k.content.to_vec();bytes.extend(k.lookup);state.keys=Some(bytes);state.trusted.insert(id,STANDARD.encode(key.verifying_key().as_bytes()));state.pending=None;
 }else{if pin.is_empty(){return Err("添加设备请使用已有设备生成的配对码".into());}state.pending=Some(Pending{code,pinned_issuer:pin});}
 store_secret(&state)?;config.last_error=None;let result=super::update_settings(|s|{s.server_url=config.server_url;s.device_id=config.device_id;s.vault_id=config.vault_id;s.enabled=true;s.paired=state.keys.is_some();s.last_error=None;})?;super::start_worker();Ok(result)
}
pub async fn invite()->Result<String,String>{
 if !ready(){return Err("请先完成此设备配对".into());}
 let result=call(Method::POST,"/v1/invitations",Some(json!({}))).await?;
 let invite=Invite{server:load()?.server_url,code:result["code"].as_str().ok_or("服务未返回配对码")?.into(),issuer:STANDARD.encode(load_or_create_device_signing_key()?.verifying_key().as_bytes())};
 Ok(format!("cockpit-sync-v1:{}",URL_SAFE_NO_PAD.encode(serde_json::to_vec(&invite).map_err(|_|"配对码编码失败")?)))
}
pub async fn pairings()->Result<Value,String>{call(Method::GET,"/v1/pairings/pending",None).await}
pub async fn approve(pairing_id:String)->Result<(),String>{
 let list=pairings().await?;let item=list["pairings"].as_array().and_then(|a|a.iter().find(|v|v["pairing_id"].as_str()==Some(&pairing_id))).ok_or("配对请求已失效")?;
 let member=item["device_id"].as_str().ok_or("配对设备标识缺失")?.to_string();let public=item["public_key"].as_str().ok_or("配对设备公钥缺失")?.to_string();let enc=item["box_public_key"].as_str().ok_or("配对设备加密公钥缺失")?.to_string();
 let enc_bytes:[u8;32]=STANDARD.decode(&enc).map_err(|_|"配对加密公钥无效")?.try_into().map_err(|_|"配对加密公钥长度无效")?;
 let mut state=secret()?;let _=keys(&state)?;let signing=load_or_create_device_signing_key()?;
 state.trusted.insert(member.clone(),public.clone());
 let bundle=serde_json::to_vec(&(state.keys.clone(),state.epoch,&state.trusted)).map_err(|_|"配对密钥编码失败")?;
 let wrapped=crypto_box::PublicKey::from(enc_bytes).seal(&mut rand::rngs::OsRng,&bundle).map_err(|_|"配对加密失败")?;
 let mut cert=Certificate{issuer:device_id(&signing),member,public_key:public,box_public_key:enc,vault:load()?.vault_id.ok_or("同步库未初始化")?,pairing:pairing_id.clone(),wrapped_hash:format!("{:x}",Sha256::digest(&wrapped)),signature:String::new()};
 cert.signature=STANDARD.encode(signing.sign(&cert_bytes(&cert)?).to_bytes());
 call(Method::POST,&format!("/v1/pairings/{pairing_id}/approve"),Some(json!({"encrypted_key_wrap":STANDARD.encode(&wrapped),"membership_signature":serde_json::to_string(&cert).map_err(|_|"配对证明编码失败")?}))).await?;
 store_secret(&state)
}
pub async fn finish_pairing()->Result<bool,String>{
 let mut state=secret()?;let Some(pending)=state.pending.clone()else{return Ok(state.keys.is_some());};let config=load()?;let key=load_or_create_device_signing_key()?;
 let response=transport::poll_enrollment(&config.server_url,&pending.code,&device_id(&key)).await?;
 if response["status"].as_str()!=Some("approved"){return Ok(false);}
 let cert:Certificate=serde_json::from_str(response["membership_signature"].as_str().ok_or("缺少配对证明")?).map_err(|_|"配对证明无效")?;
 verify_cert(&cert,&pending.pinned_issuer)?;
 let wrapped=STANDARD.decode(response["encrypted_key_wrap"].as_str().ok_or("缺少加密配对数据")?).map_err(|_|"加密配对数据无效")?;
 if cert.member!=device_id(&key)||cert.vault!=config.vault_id.as_deref().unwrap_or("")||cert.public_key!=STANDARD.encode(key.verifying_key().as_bytes())||cert.wrapped_hash!=format!("{:x}",Sha256::digest(&wrapped)){return Err("配对身份或密钥绑定不一致".into());}
 let decrypted=crypto_box::SecretKey::from(state.box_secret).unseal(&wrapped).map_err(|_|"配对数据解密失败")?;
 let (keys,epoch,trusted):(Option<Vec<u8>>,i64,BTreeMap<String,String>)=serde_json::from_slice(&decrypted).map_err(|_|"配对数据无效")?;
 state.keys=keys;state.epoch=epoch;state.trusted=trusted;state.pending=None;let _=self::keys(&state)?;store_secret(&state)?;super::update_settings(|s|{s.paired=true;s.last_error=None;})?;Ok(true)
}
pub async fn devices()->Result<Value,String>{
 let value=call(Method::GET,"/v1/devices",None).await?;let mut state=secret()?;
 let items=value["devices"].as_array().ok_or("设备列表无效")?;
 for _ in 0..items.len(){for item in items{
  let Some(text)=item["certificate"].as_str()else{continue};let Ok(cert)=serde_json::from_str::<Certificate>(text)else{continue};
  if cert.vault!=load()?.vault_id.as_deref().unwrap_or("")||item["device_id"].as_str()!=Some(&cert.member)||item["public_key"].as_str()!=Some(&cert.public_key){continue;}
  if let Some(issuer)=state.trusted.get(&cert.issuer){if verify_cert(&cert,issuer).is_ok(){state.trusted.insert(cert.member,cert.public_key);}}
 }}
 store_secret(&state)?;Ok(value)
}
pub fn trusted_key(sender:&str)->Result<VerifyingKey,String>{let state=secret()?;public_key(state.trusted.get(sender).ok_or("候选设备未经本机信任链验证")?)}
