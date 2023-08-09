use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::{Path, PathBuf};

use blake3::Hasher as Blake3;
use blake3::traits::digest::Digest;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{Manager, Runtime, Window};
use uuid::Uuid;
use zip::{ZipArchive, ZipWriter};
use zip::write::FileOptions;

use crate::NeededFiles;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase", tag = "kind")]
enum DeltaProgress {
    SettingUp,
    HashingFiles {
        current: usize,
        total: usize,
    },
    CalculatingDifference,
    CreatingUpdateFile {
        current: usize,
        total: usize,
    },
    Done,
}

impl DeltaProgress {
    pub const EVENT: &'static str = "delta-progress";

    pub fn emit<R: Runtime>(&self, window: &Window<R>) -> tauri::Result<()> {
        window.emit_all(Self::EVENT, self)
    }
}

#[derive(Deserialize)]
pub struct DeltaInfo {
    pub version_id: Uuid,
    pub needed_files: NeededFiles,
}

#[derive(Serialize)]
struct DeltaManifest {
    updates: Uuid,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PenumbraGroup {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub default_settings: u32,
    #[serde(rename = "Type")]
    pub kind: SelectionKind,
    pub options: Vec<PenumbraOption>,
}

#[derive(Deserialize, Serialize)]
pub enum SelectionKind {
    Single,
    Multi,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PenumbraOption {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub priority: i32,
    pub files: HashMap<String, String>,
    pub file_swaps: HashMap<String, String>,
    pub manipulations: Vec<Value>,
}

pub fn delta_inner<R: Runtime>(window: Window<R>, path: &str, info: DeltaInfo) -> anyhow::Result<()> {
    DeltaProgress::SettingUp.emit(&window)?;

    let path = Path::new(path);
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("pmp") => return pmp_delta(window, path, info),
        Some("ttmp" | "ttmp2") => anyhow::bail!("textools not supported"),
        Some(_) | None => anyhow::bail!("could not determine file type from extension"),
    }
}

fn pmp_delta<R: Runtime>(window: Window<R>, path: &Path, info: DeltaInfo) -> anyhow::Result<()> {
    // open the pmp and make sure it has a meta.json
    let file = File::open(path)?;
    let mut zip = ZipArchive::new(file)?;
    let files: Vec<_> = zip.file_names().map(ToOwned::to_owned).collect();

    if !files.iter().any(|file| file == "meta.json") {
        anyhow::bail!("invalid pmp: missing meta.json");
    }

    // find all the groups and the default group, parse them
    let group_paths: Vec<_> = files.iter()
        .filter(|file| file.starts_with("group_") && file.ends_with(".json"))
        .sorted_unstable()
        .collect();
    let mut groups = Vec::with_capacity(group_paths.len());
    let mut default: PenumbraOption = serde_json::from_reader(zip.by_name("default_mod.json")?)?;
    for path in group_paths {
        let file = zip.by_name(path)?;
        let group: PenumbraGroup = serde_json::from_reader(file)?;
        groups.push(group);
    }

    // add all the paths referenced
    let mut to_hash = HashSet::new();
    for file in default.files.values() {
        to_hash.insert(file.clone());
    }

    for group in &groups {
        for option in &group.options {
            for file in option.files.values() {
                to_hash.insert(file.clone());
            }
        }
    }

    // hash all the paths
    DeltaProgress::HashingFiles {
        current: 0,
        total: to_hash.len(),
    }.emit(&window)?;

    let mut hasher = Blake3::default();
    let mut path_hashes = HashMap::new();
    let mut hash_to_path = HashMap::new();
    for (i, path) in to_hash.iter().enumerate() {
        std::io::copy(&mut zip.by_name(path)?, &mut hasher)?;
        let hash = data_encoding::BASE64URL_NOPAD.encode(&hasher.finalize_reset());
        path_hashes.insert(path, hash.clone());
        hash_to_path.insert(hash, path);

        DeltaProgress::HashingFiles {
            current: i,
            total: to_hash.len(),
        }.emit(&window)?;
    }

    DeltaProgress::CalculatingDifference.emit(&window)?;

    // find the new hashes in this zip
    let local_hashes: HashSet<_> = path_hashes.values().cloned().collect();
    let remote_hashes: HashSet<_> = info.needed_files.files.keys()
        .cloned()
        .collect();
    let new_hashes: Vec<_> = local_hashes.difference(&remote_hashes).collect();

    // update the jsons to point to standardised paths
    for local_path in default.files.values_mut() {
        let hash = &path_hashes[&*local_path];
        *local_path = format!("files/{hash}");
    }

    for group in &mut groups {
        for option in &mut group.options {
            for local_path in option.files.values_mut() {
                let hash = &path_hashes[&*local_path];
                *local_path = format!("files/{hash}");
            }
        }
    }


    let mut current = 0;
    let total = new_hashes.len() + 3 + groups.len();
    DeltaProgress::CreatingUpdateFile {
        current,
        total,
    }.emit(&window)?;
    // create a new zip
    let mut new_path = PathBuf::from(path);
    new_path.set_extension("delta.hsp");
    let mut new_file = ZipWriter::new(File::create(new_path)?);

    // write the new files into the zip
    for hash in new_hashes {
        new_file.start_file(format!("files/{hash}"), FileOptions::default())?;
        let path = hash_to_path[hash];
        std::io::copy(&mut zip.by_name(path)?, &mut new_file)?;

        current += 1;
        DeltaProgress::CreatingUpdateFile {
            current,
            total,
        }.emit(&window)?;
    }

    // add the delta manifest
    new_file.start_file("heliosphere_delta.json", FileOptions::default())?;
    serde_json::to_writer(&mut new_file, &DeltaManifest {
        updates: info.version_id,
    })?;

    current += 1;
    DeltaProgress::CreatingUpdateFile {
        current,
        total,
    }.emit(&window)?;

    // add the jsons
    new_file.start_file("meta.json", FileOptions::default())?;
    std::io::copy(&mut zip.by_name("meta.json")?, &mut new_file)?;

    current += 1;
    DeltaProgress::CreatingUpdateFile {
        current,
        total,
    }.emit(&window)?;

    new_file.start_file("default_mod.json", FileOptions::default())?;
    serde_json::to_writer(&mut new_file, &default)?;

    current += 1;
    DeltaProgress::CreatingUpdateFile {
        current,
        total,
    }.emit(&window)?;

    for (i, group) in groups.iter().enumerate() {
        new_file.start_file(format!("group_{:<03}_group.json", i + 1), FileOptions::default())?;
        serde_json::to_writer(&mut new_file, group)?;

        current += 1;
        DeltaProgress::CreatingUpdateFile {
            current,
            total,
        }.emit(&window)?;
    }

    new_file.finish()?;

    DeltaProgress::Done.emit(&window)?;

    Ok(())
}
