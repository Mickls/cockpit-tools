use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{Signer, SigningKey};
use reqwest::{Client, Method};
use serde_json::Value;
use sha2::{Digest,Sha256};
use std::time::Duration;

pub fn client(server: &str) -> Result<Client,String> {
    let url=url::Url::parse(server).map_err(|_|"同步服务器地址无效")?;
    if url.scheme()!="https" || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some(){return Err("同步服务器必须使用不含凭据、查询或片段的 HTTPS 地址".into());}
    let mut builder=Client::builder().timeout(Duration::from_secs(8)).redirect(reqwest::redirect::Policy::none());
    if url.host_str()==Some("100.100.1.1") {
        builder=builder.add_root_certificate(reqwest::Certificate::from_pem(include_bytes!("../../../../docs/credential-sync/server-ca.pem")).map_err(|_|"同步服务器证书无效")?);
    }
    builder.build().map_err(|_|"无法建立同步网络客户端".into())
}
pub async fn request(server:&str,device:&str,key:&SigningKey,method:Method,path:&str,body:Option<Value>)->Result<Value,String>{
    let timestamp=chrono::Utc::now().timestamp().to_string();let nonce=uuid::Uuid::new_v4().to_string();
    let bytes=body.map(|v|serde_json::to_vec(&v)).transpose().map_err(|_|"同步请求编码失败")?.unwrap_or_default();
    let canonical=format!("{}\n{}\n{}\n{}\n{:x}",method,path,timestamp,nonce,Sha256::digest(&bytes));
    let response=client(server)?.request(method,format!("{}{}",server.trim_end_matches('/'),path))
      .header("x-device-id",device).header("x-timestamp",timestamp).header("x-nonce",nonce)
      .header("x-signature",STANDARD.encode(key.sign(canonical.as_bytes()).to_bytes())).header("content-type","application/json").body(bytes).send().await.map_err(|_|"同步服务网络请求失败")?;
    let status=response.status();if !status.is_success(){return Err(format!("同步服务返回 HTTP {}",status.as_u16()));}
    let mut response=response;let mut bytes=Vec::new();
    while let Some(chunk)=response.chunk().await.map_err(|_|"同步服务读取失败")?{if bytes.len()+chunk.len()>131072{return Err("同步响应超过大小限制".into());}bytes.extend(chunk);}
    serde_json::from_slice(&bytes).map_err(|_|"同步服务响应格式无效".into())
}
pub async fn enroll(server:&str,code:&str,device_id:&str,public_key:&str,box_public_key:&str)->Result<Value,String>{
    let body=serde_json::json!({"code":code,"device_id":device_id,"public_key":public_key,"box_public_key":box_public_key});
    let response=client(server)?.post(format!("{}/v1/enroll",server.trim_end_matches('/'))).json(&body).send().await.map_err(|_|"设备登记请求失败")?;
    let status=response.status();if !status.is_success(){return Err(format!("设备登记返回 HTTP {}",status.as_u16()));}
    response.json().await.map_err(|_|"设备登记响应格式无效".into())
}

pub async fn poll_enrollment(server:&str,code:&str,device_id:&str)->Result<Value,String>{
 let response=client(server)?.post(format!("{}/v1/enroll/status",server.trim_end_matches('/'))).json(&serde_json::json!({"code":code,"device_id":device_id})).send().await.map_err(|_|"配对状态查询失败")?;
 if !response.status().is_success(){return Err(format!("配对状态 HTTP {}",response.status().as_u16()));}
 response.json().await.map_err(|_|"配对状态格式无效".into())
}
