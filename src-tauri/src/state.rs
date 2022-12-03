use std::collections::HashMap;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tauri::PathResolver;
use tokio::fs::{File as TokioFile, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;

use crate::UsageInfo;

pub struct State {
    pub cache: RwLock<Cache>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct Cache {
    pub usage: HashMap<String, UsageInfo>,
}

impl State {
    pub async fn load(resolver: &PathResolver) -> anyhow::Result<Self> {
        let cache = Self::load_cache(resolver).await?;

        Ok(Self {
            cache: RwLock::new(cache),
        })
    }

    pub async fn save(&self, resolver: &PathResolver) -> anyhow::Result<()> {
        self.save_cache(resolver).await?;

        Ok(())
    }

    async fn cache_file(resolver: &PathResolver) -> anyhow::Result<TokioFile> {
        let cache_dir = resolver.app_cache_dir().context("no cache dir")?;
        tokio::fs::create_dir_all(&cache_dir).await?;

        let path = cache_dir.join("usage-cache.json");
        let file = OpenOptions::default()
            .create(true)
            .write(true)
            .read(true)
            .open(path)
            .await?;

        Ok(file)
    }

    pub async fn save_cache(&self, resolver: &PathResolver) -> anyhow::Result<()> {
        let mut file = Self::cache_file(resolver).await?;
        let json = serde_json::to_string(&*self.cache.read().await)?;
        file.write_all(json.as_bytes()).await?;

        Ok(())
    }

    pub async fn load_cache(resolver: &PathResolver) -> anyhow::Result<Cache> {
        let mut file = Self::cache_file(resolver).await?;
        let mut json = String::new();
        file.read_to_string(&mut json).await?;

        let cache = serde_json::from_str(&json).unwrap_or_default();
        Ok(cache)
    }
}
