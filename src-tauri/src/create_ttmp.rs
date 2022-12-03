use std::collections::HashMap;
use std::fs::File as StdFile;
use std::io::{BufWriter, Seek, SeekFrom, Write};

use anyhow::Context;
use serde::Serialize;
use tauri::{Manager, Runtime};
use tauri::window::Window;
use tokio::fs::File as TokioFile;
use tokio::io::AsyncSeekExt;
use ttmp::model::{ManifestKind, ModGroup, ModOption, ModPack, ModPackPage, SimpleMod};
use ttmp::mpd_encoder::{FileInfo, MpdEncoder};
use zip::CompressionMethod;
use zip::write::{FileOptions, ZipWriter};
use zstd::stream::write::Decoder;

use crate::{Group, ModInfo, NeededFiles};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase", tag = "kind")]
enum TtmpProgress {
    CreatingManifest,
    DownloadingFiles {
        current: usize,
        total: usize,
    },
    ProcessingArchive,
    Done,
}

impl TtmpProgress {
    pub const EVENT: &'static str = "ttmp-progress";

    pub fn emit<R: Runtime>(&self, window: &Window<R>) -> tauri::Result<()> {
        window.emit_all(Self::EVENT, self)
    }
}

lazy_static::lazy_static! {
    static ref DAT_FILES: HashMap<&'static str, &'static str> = maplit::hashmap! {
        "common" => "000000",
        "bgcommon" => "010000",
        "bg" => "020000",
        "cut" => "030000",
        "chara" => "040000",
        "shader" => "050000",
        "ui" => "060000",
        "sound" => "070000",
        "vfx" => "080000",
        "ui_script" => "090000",
        "exd" => "0a0000",
        "game_script" => "0b0000",
        "music" => "0c0000",
        "sqpack_test" => "120000",
        "debug" => "130000",
    };
}

pub async fn create_ttmp_inner<R: Runtime>(window: Window<R>, path: &str, info: ModInfo, groups: Vec<Group>, needed_files: NeededFiles) -> anyhow::Result<()> {
    TtmpProgress::CreatingManifest.emit(&window)?;

    // create a manifest
    let mut pages: HashMap<String, ModPackPage> = HashMap::new();
    let mut simple_mods = Vec::new();

    let groups: HashMap<String, Group> = groups.into_iter()
        .map(|group| (group.name.clone(), group))
        .collect();

    for uses in needed_files.files.values() {
        for use_ in uses {
            let dat_file = use_.2.split('/')
                .next()
                .and_then(|first| DAT_FILES.get(first))
                .copied()
                .unwrap_or("040000")
                .to_string();

            let simple = SimpleMod {
                name: "".into(),
                full_path: use_.2.clone(),
                mod_offset: 0, // this will be set by the encoder
                mod_size: 0, // this will be set by the encoder
                mod_pack_entry: None,
                category: "Mod file".into(),
                dat_file,
                is_default: false,
            };

            if use_.0.is_none() && use_.1.is_none() {
                simple_mods.push(simple);
                continue;
            }

            let group = use_.0.as_ref().unwrap();
            let option = use_.1.as_ref().unwrap();

            let num_pages = pages.len();
            let page = pages.entry(group.clone())
                .or_insert_with(|| {
                    let g = groups.get(group).unwrap();
                    ModPackPage {
                        mod_groups: vec![
                            ModGroup {
                                group_name: group.clone(),
                                option_list: g.options.iter()
                                    .map(|opt| ModOption {
                                        name: opt.name.clone(),
                                        selection_type: g.selection_type.into(),
                                        group_name: g.name.clone(),
                                        description: opt.description.clone(),
                                        image_path: opt.image_path.clone(),
                                        is_checked: false,
                                        mods_jsons: Vec::new(),
                                    })
                                    .collect(),
                                selection_type: g.selection_type.into(),
                            }
                        ],
                        page_index: num_pages as i32,
                    }
                });

            println!("use_: {:?}", use_);
            println!("page: {:#?}", page);

            let page_group = &mut page.mod_groups[0];
            if let Some(opt) = page_group.option_list.iter_mut().find(|opt| {
                println!("opt.name: {:?}", opt.name);
                println!("option: {:?}", option);
                println!("equal?: {}", opt.name == *option);
                opt.name == *option
            }) {
                println!("pushing");
                opt.mods_jsons.push(simple);
            }
        }
    }

    let mod_pack = ModPack {
        ttmp_version: "1.3w".into(),
        minimum_framework_version: Some("1.3.0.0".into()),
        name: info.name,
        author: info.author,
        version: info.version,
        description: Some(info.description),
        url: Some(info.url),
        mod_pack_pages: Some(pages.into_values().collect()),
        simple_mods_list: Some(simple_mods),
    };

    let client = reqwest::ClientBuilder::new()
        .build()?;

    let num_files = needed_files.files.len();

    TtmpProgress::DownloadingFiles {
        current: 0,
        total: num_files,
    }.emit(&window)?;

    let mpd = tempfile::tempfile().context("could not create temp file")?;
    let mut encoder = Some(MpdEncoder::new(mpd, ManifestKind::V2(mod_pack)));
    let mut staging = Some(TokioFile::from_std(tempfile::tempfile().context("could not create temp file")?));
    for (i, (hash, uses)) in needed_files.files.into_iter().enumerate() {
        // truncate temp file
        staging.as_mut().unwrap().seek(SeekFrom::Start(0)).await?;
        staging.as_ref().unwrap().set_len(0).await?;

        let mut url = needed_files.base_uri.clone();
        url.path_segments_mut()
            .ok()
            .context("url cannot be a base")?
            .pop_if_empty()
            .push(&hash);

        // download file and save to temp file
        let mut resp = client.get(url).send().await?
            .error_for_status()?;

        let mut std_staging = staging.take().unwrap().into_std().await;
        let mut decoder = Decoder::new(BufWriter::new(&mut std_staging))?;

        while let Some(chunk) = resp.chunk().await? {
            decoder.write_all(chunk.as_ref())?;
        }

        decoder.flush()?;
        drop(decoder);

        staging = Some(TokioFile::from_std(std_staging));

        let file_size = staging.as_mut().unwrap().stream_position().await?;

        let mut std_staging = staging.take().unwrap().into_std().await;
        let mut std_encoder = encoder.take().unwrap();
        let res = tauri::async_runtime::spawn_blocking(move || {
            for use_ in uses {
                std_staging.seek(SeekFrom::Start(0))?;

                let info = FileInfo {
                    group: use_.0,
                    option: use_.1,
                    game_path: use_.2,
                };

                let path = &info.game_path;

                if path.ends_with(".tex") || path.ends_with(".atex") {
                    std_encoder.add_texture_file(info, file_size as usize, &mut std_staging)?;
                } else if path.ends_with(".mdl") {
                    std_encoder.add_model_file(info, file_size as usize, &mut std_staging)?;
                } else {
                    std_encoder.add_standard_file(info, file_size as usize, &mut std_staging)?;
                }
            }

            let tuple = (Some(std_encoder), Some(TokioFile::from_std(std_staging)));
            Result::<_, anyhow::Error>::Ok(tuple)
        }).await??;

        encoder = res.0;
        staging = res.1;

        TtmpProgress::DownloadingFiles {
            current: i + 1,
            total: num_files,
        }.emit(&window)?;
    }

    TtmpProgress::ProcessingArchive.emit(&window)?;

    let path = path.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let (manifest, mut file) = encoder.unwrap().finalize()?;
        file.seek(SeekFrom::Start(0))?;

        let ttmp_file = StdFile::create(path)?;
        let mut zip = ZipWriter::new(ttmp_file);
        zip.start_file("TTMPL.mpl", FileOptions::default().compression_method(CompressionMethod::Deflated))?;
        match manifest {
            ManifestKind::V2(packs) => serde_json::to_writer(&mut zip, &packs)?,
            ManifestKind::V1(mods) => {
                for mod_ in mods {
                    serde_json::to_writer(&mut zip, &mod_)?;
                    zip.write_all(b"\n")?;
                }
            }
        }

        zip.start_file("TTMPD.mpd", FileOptions::default().compression_method(CompressionMethod::Stored))?;
        std::io::copy(&mut file, &mut zip)?;

        zip.finish()?;

        Result::<(), anyhow::Error>::Ok(())
    }).await??;

    TtmpProgress::Done.emit(&window)?;

    Ok(())
}
