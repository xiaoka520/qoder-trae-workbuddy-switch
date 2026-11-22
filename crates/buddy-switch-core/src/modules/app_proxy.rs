//! 应用请求的显式出站代理。各调用方保留各自的超时和缺省客户端。

use std::{fs, io, path::PathBuf, sync::Mutex};

use reqwest::{Client, ClientBuilder, Proxy, Url};
use serde::{Deserialize, Serialize};

use super::config::{atomic_write, ensure_private_dir, store_dir};

#[derive(Default, Deserialize, Serialize)]
pub struct AppProxyConfig {
    pub url: String,
}

impl AppProxyConfig {
    fn path() -> PathBuf {
        store_dir().join("outbound_proxy.json")
    }

    pub fn load() -> Result<Self, String> {
        match fs::read_to_string(Self::path()) {
            Ok(content) => {
                serde_json::from_str(&content).map_err(|_| "出站代理配置无法读取".into())
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(_) => Err("出站代理配置无法读取".into()),
        }
    }

    pub fn save(self) -> Result<Self, String> {
        let url = self.url.trim().to_owned();
        if !url.is_empty() {
            parse_proxy(&url)?;
        }
        let saved = Self { url };
        let content = serde_json::to_string_pretty(&saved).map_err(|error| error.to_string())?;
        ensure_private_dir(&store_dir()).map_err(|_| "无法打开出站代理配置目录".to_string())?;
        atomic_write(&Self::path(), &content).map_err(|_| "无法保存出站代理配置".to_string())?;
        Ok(saved)
    }
}

fn parse_proxy(raw: &str) -> Result<Proxy, String> {
    let invalid = || "代理地址无效，请填写 HTTP、HTTPS、SOCKS5 或 SOCKS5H 地址".to_string();
    let (scheme, _) = raw.split_once("://").ok_or_else(invalid)?;
    if !matches!(scheme, "http" | "https" | "socks5" | "socks5h") {
        return Err(invalid());
    }
    let url = Url::parse(raw).map_err(|_| invalid())?;
    if url.host().is_none()
        || !matches!(url.path(), "" | "/")
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    Proxy::all(url).map_err(|_| invalid())
}

/// 每条链路缓存最后一个显式代理的连接池。配置变化后，新请求使用新出口。
pub struct ClientCache(Mutex<Option<(String, Client)>>);

impl ClientCache {
    pub const fn new() -> Self {
        Self(Mutex::new(None))
    }

    pub fn get(
        &self,
        default: impl FnOnce() -> Result<Client, String>,
        builder: impl FnOnce() -> ClientBuilder,
    ) -> Result<Client, String> {
        let mut cached = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let AppProxyConfig { url } = AppProxyConfig::load()?;
        if url.is_empty() {
            *cached = None;
            drop(cached);
            return default();
        }
        if let Some((existing, client)) = cached.as_ref() {
            if existing == &url {
                return Ok(client.clone());
            }
        }
        let client = builder()
            .no_proxy()
            .proxy(parse_proxy(&url)?)
            .build()
            .map_err(|_| "无法初始化出站代理客户端".to_string())?;
        *cached = Some((url, client.clone()));
        Ok(client)
    }
}

impl Default for ClientCache {
    fn default() -> Self {
        Self::new()
    }
}
