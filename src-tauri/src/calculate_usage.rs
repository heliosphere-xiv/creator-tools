use std::collections::{HashMap, HashSet};
use std::io::Seek;
use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use sha3::{Digest, Sha3_256};
use tauri::{Manager, Runtime, State as TauriState, Window};
use tokio::fs::File as TokioFile;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use ttmp::ttmp_extractor::TtmpExtractor;

use crate::{multi_writer::MultiWriter, State, UsageInfo};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalculatedUsage {
    total: usize,
    hashes: HashMap<String, Calculated>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Calculated {
    size: usize,
    files: Vec<String>,
    counts: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase", tag = "kind")]
enum StorageProgress {
    HashingTtmp,
    ExtractingFiles {
        current: usize,
        total: usize,
    },
    CalculatingSize,
    Done,
}

impl StorageProgress {
    pub const EVENT: &'static str = "storage-progress";

    pub fn emit<R: Runtime>(&self, window: &Window<R>) -> tauri::Result<()> {
        window.emit_all(Self::EVENT, self)
    }
}

pub async fn calculate_usage_inner<R: Runtime>(window: Window<R>, state: TauriState<'_, Arc<State>>, path: &str, hashes: HashSet<&str>) -> anyhow::Result<CalculatedUsage> {
    let mut buf = [0; 4096];

    StorageProgress::HashingTtmp.emit(&window)?;

    let mut sha = Sha3_256::default();
    let mut file = TokioFile::open(path).await?;

    let mut ttmp_size = 0;
    loop {
        let read = file.read(&mut buf).await?;
        if read == 0 {
            break;
        }

        ttmp_size += read;
        sha.update(&buf[..read]);
    }

    let ttmp_hash = sha.finalize_reset();
    let ttmp_hash = hex::encode(ttmp_hash);
    let (contained, ttmp_hashes) = if let Some(cached) = state.cache.read().await.usage.get(&ttmp_hash) {
        (true, cached.hashes.clone())
    } else {
        let window = window.clone();

        file.rewind().await?;
        let file = file.into_std().await;

        tokio::task::spawn_blocking(move || {
            let extractor = TtmpExtractor::new(file)?;
            let files = extractor.all_files_sorted();
            let files_len = files.len();
            let mut zip = extractor.zip().borrow_mut();
            let mut mpd = zip.by_name("TTMPD.mpd")?;

            let mut hashes = HashMap::new();

            StorageProgress::ExtractingFiles {
                current: 0,
                total: files_len,
            }.emit(&window)?;

            let mut last_offset = None;

            let mut staging = tempfile::tempfile()?;
            let mut compressed = tempfile::tempfile()?;
            for (i, file) in files.into_iter().enumerate() {
                // handle deduped ttmps
                if Some(file.file.mod_offset) == last_offset {
                    continue;
                }

                last_offset = Some(file.file.mod_offset);

                staging.rewind()?;
                staging.set_len(0)?;

                compressed.rewind()?;
                compressed.set_len(0)?;

                // extract file to disk (seek-able)
                TtmpExtractor::extract_one_into(&file, &mut mpd, &mut staging)?;
                staging.rewind()?;

                // copy from file to hasher and compressor
                let mut compressor = zstd::Encoder::new(&mut compressed, 9)?;
                let mut writer = MultiWriter::new(
                    &mut sha,
                    &mut compressor,
                );
                std::io::copy(&mut staging, &mut writer)?;
                compressor.finish()?;

                let size = compressed.metadata()?.len();
                let hash = hex::encode(sha.finalize_reset());

                hashes.entry(hash)
                    .or_insert_with(|| (size as usize, Vec::with_capacity(1)))
                    .1
                    .push(file.file.full_path.clone());

                StorageProgress::ExtractingFiles {
                    current: i + 1,
                    total: files_len,
                }.emit(&window)?;
            }

            Result::<_, anyhow::Error>::Ok((false, hashes))
        }).await??
    };

    if !contained {
        state.cache.write().await.usage.insert(ttmp_hash, UsageInfo {
            file_name: Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("<unknown>")
                .to_string(),
            ttmp_size,
            hashes: ttmp_hashes.clone(),
        });

        state.save_cache(&window.app_handle().path_resolver()).await?;
    }

    StorageProgress::CalculatingSize.emit(&window)?;

    let ttmp_keys: HashSet<_> = ttmp_hashes.keys().map(|s| s.as_str()).collect();
    let new_size: usize = ttmp_keys.difference(&hashes)
        .map(|&key| ttmp_hashes[key].0)
        .sum();
    let calculated_hashes = ttmp_hashes
        .into_iter()
        .map(|(key, (size, files))| (key.clone(), Calculated {
            size,
            files,
            counts: !hashes.contains(&*key),
        }))
        .collect();

    StorageProgress::Done.emit(&window)?;

    let calculated = CalculatedUsage {
        total: new_size,
        hashes: calculated_hashes,
    };

    Ok(calculated)
}
