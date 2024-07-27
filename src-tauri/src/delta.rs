use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use anyhow::Context;
use blake3::Hasher as Blake3;
use blake3::traits::digest::Digest;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{Manager, Runtime, Window};
use uuid::Uuid;
use zip::{ZipArchive, ZipWriter};
use zip::write::SimpleFileOptions;

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
    #[serde(default)]
    pub version: Option<i32>,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub default_settings: u32,
    #[serde(rename = "Type", flatten)]
    pub kind: GroupKind,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "Type")]
pub enum GroupKind {
    Single {
        #[serde(rename = "Options", default)]
        options: Vec<PenumbraStandardOption>,
    },
    Multi {
        #[serde(rename = "Options", default)]
        options: Vec<PenumbraStandardOption>,
    },
    Imc {
        #[serde(rename = "Identifier")]
        identifier: serde_json::Value,

        #[serde(rename = "AllVariants", default)]
        all_variants: bool,

        #[serde(rename = "DefaultEntry")]
        default_entry: serde_json::Value,

        #[serde(rename = "Options", default)]
        options: Vec<PenumbraImcOption>,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PenumbraStandardOptionSimple {
    #[serde(default)]
    pub version: Option<i32>,
    #[serde(default)]
    pub files: HashMap<String, String>,
    #[serde(default)]
    pub file_swaps: HashMap<String, String>,
    #[serde(default)]
    pub manipulations: Vec<Value>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PenumbraStandardOption {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub priority: i32,
    #[serde(flatten)]
    pub simple: PenumbraStandardOptionSimple,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PenumbraImcOption {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_disable_sub_mod: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute_mask: Option<i32>,
}

pub fn delta_inner<R: Runtime>(window: Window<R>, path: &str, info: DeltaInfo) -> anyhow::Result<()> {
    DeltaProgress::SettingUp.emit(&window)?;

    let path = Path::new(path);
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("pmp") => pmp_delta(window, path, info),
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
    let mut default: PenumbraStandardOptionSimple = {
        let file = zip.by_name("default_mod.json")
            .context("could not get default_mod.json")?;
        let without_bom = skip_bom(file)
            .context("could not skip bom")?;
        serde_json::from_reader(without_bom)
            .context("invalid pmp: invalid default_mod.json")?
    };
    for path in group_paths {
        let file = zip.by_name(path)
            .with_context(|| format!("could not get {path}"))?;
        let without_bom = skip_bom(file)
            .context("could not skip bom")?;
        let group: PenumbraGroup = serde_json::from_reader(without_bom)
            .with_context(|| format!("invalid pmp: invalid group {path}"))?;
        groups.push(group);
    }

    // add all the paths referenced
    let mut to_hash = HashSet::new();
    for file in default.files.values() {
        to_hash.insert(file.replace('\\', "/"));
    }

    for group in &groups {
        match &group.kind {
            GroupKind::Single { options }
            | GroupKind::Multi { options } => {
                for option in options {
                    for file in option.simple.files.values() {
                        to_hash.insert(file.replace('\\', "/"));
                    }
                }
            }
            _ => {}
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
        match &mut group.kind {
            GroupKind::Single { options }
            | GroupKind::Multi { options } => {
                for option in options {
                    for local_path in option.simple.files.values_mut() {
                        let hash = &path_hashes[&*local_path];
                        *local_path = format!("files/{hash}");
                    }
                }
            }
            _ => {}
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
        new_file.start_file(format!("files/{hash}"), SimpleFileOptions::default())?;
        let path = hash_to_path[hash];
        std::io::copy(&mut zip.by_name(path)?, &mut new_file)?;

        current += 1;
        DeltaProgress::CreatingUpdateFile {
            current,
            total,
        }.emit(&window)?;
    }

    // add the delta manifest
    new_file.start_file("heliosphere_delta.json", SimpleFileOptions::default())?;
    serde_json::to_writer(&mut new_file, &DeltaManifest {
        updates: info.version_id,
    })?;

    current += 1;
    DeltaProgress::CreatingUpdateFile {
        current,
        total,
    }.emit(&window)?;

    // add the jsons
    new_file.start_file("meta.json", SimpleFileOptions::default())?;
    std::io::copy(&mut zip.by_name("meta.json")?, &mut new_file)?;

    current += 1;
    DeltaProgress::CreatingUpdateFile {
        current,
        total,
    }.emit(&window)?;

    new_file.start_file("default_mod.json", SimpleFileOptions::default())?;
    serde_json::to_writer(&mut new_file, &default)?;

    current += 1;
    DeltaProgress::CreatingUpdateFile {
        current,
        total,
    }.emit(&window)?;

    for (i, group) in groups.iter().enumerate() {
        new_file.start_file(format!("group_{:<03}_group.json", i + 1), SimpleFileOptions::default())?;
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

pub fn skip_bom<R: Read>(reader: R) -> std::io::Result<BufReader<R>> {
    let mut reader = BufReader::new(reader);
    let filled = reader.fill_buf()?;
    if filled.len() >= 3 && matches!(&filled[..3], [0xEF, 0xBB, 0xBF]) {
        // skip the bom
        reader.consume(3);
    }

    Ok(reader)
}
