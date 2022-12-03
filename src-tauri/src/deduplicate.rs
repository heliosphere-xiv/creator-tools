use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

use serde::Serialize;
use tauri::{Manager, Runtime, Window};
use ttmp::model::ManifestKind;
use ttmp::mpd_encoder::{FileInfo, MpdEncoder};
use ttmp::ttmp_extractor::TtmpExtractor;
use zip::{CompressionMethod, ZipWriter};
use zip::write::FileOptions;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase", tag = "kind")]
enum DeduplicateProgress {
    SettingUp,
    ProcessingFiles {
        current: usize,
        total: usize,
    },
    CreatingArchive,
    Done,
}

impl DeduplicateProgress {
    pub const EVENT: &'static str = "deduplicate-progress";

    pub fn emit<R: Runtime>(&self, window: &Window<R>) -> tauri::Result<()> {
        window.emit_all(Self::EVENT, self)
    }
}

pub fn deduplicate_inner<R: Runtime>(window: Window<R>, path: &str) -> anyhow::Result<()> {
    DeduplicateProgress::SettingUp.emit(&window)?;

    let file = File::open(path)?;
    let extractor = TtmpExtractor::new(file)?;

    let files = extractor.all_files_sorted();
    let files_len = files.len();
    let mut zip = extractor.zip().borrow_mut();
    let mut data = zip.by_name("TTMPD.mpd")?;

    let mpd = tempfile::tempfile()?;
    let mut encoder = MpdEncoder::new(mpd, extractor.manifest().clone());
    let mut staging = tempfile::tempfile()?;

    DeduplicateProgress::ProcessingFiles {
        current: 0,
        total: files_len,
    }.emit(&window)?;

    for (i, file) in files.into_iter().enumerate() {
        staging.seek(SeekFrom::Start(0))?;
        staging.set_len(0)?;

        TtmpExtractor::extract_one_into(&file, &mut data, &mut staging)?;
        let size = staging.metadata()?.len() as usize;
        staging.seek(SeekFrom::Start(0))?;

        let info = FileInfo {
            group: file.group.map(ToOwned::to_owned),
            option: file.option.map(ToOwned::to_owned),
            game_path: file.file.full_path.clone(),
        };

        if info.game_path.ends_with(".mdl") {
            encoder.add_model_file(info, size, &mut staging)?;
        } else if info.game_path.ends_with(".tex") || info.game_path.ends_with(".atex") {
            encoder.add_texture_file(info, size, &mut staging)?;
        } else {
            encoder.add_standard_file(info, size, &mut staging)?;
        }

        DeduplicateProgress::ProcessingFiles {
            current: i + 1,
            total: files_len,
        }.emit(&window)?;
    }

    let (manifest, mut mpd) = encoder.finalize()?;
    mpd.seek(SeekFrom::Start(0))?;

    let path = Path::new(&path);
    let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("ttmp2");
    let new_path = path.with_extension(format!("deduplicated.{}", extension));

    DeduplicateProgress::CreatingArchive.emit(&window)?;

    let new_file = File::create(new_path)?;
    let mut zip = ZipWriter::new(new_file);

    zip.start_file("TTMPL.mpl", FileOptions::default().compression_method(CompressionMethod::Deflated))?;
    match manifest {
        ManifestKind::V1(mods) => for mod_ in mods {
            serde_json::to_writer(&mut zip, &mod_)?;
            zip.write_all(b"\n")?;
        }
        ManifestKind::V2(pack) => serde_json::to_writer(&mut zip, &pack)?,
    }

    zip.start_file("TTMPD.mpd", FileOptions::default().compression_method(CompressionMethod::Stored))?;
    std::io::copy(&mut mpd, &mut zip)?;

    zip.finish()?;

    DeduplicateProgress::Done.emit(&window)?;

    Ok(())
}
