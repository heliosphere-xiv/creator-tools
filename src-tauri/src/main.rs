#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{Manager, Runtime, Window, WindowEvent};
use url::Url;
use crate::delta::DeltaInfo;

use crate::state::State;

mod state;
mod multi_writer;
mod converters;

mod create_ttmp;
mod deduplicate;
mod delta;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(tauri::generate_handler![create_ttmp, deduplicate, delta])
        .setup(|app| {
            let resolver = app.path_resolver();
            let state = tauri::async_runtime::block_on(async {
                State::load(&resolver).await
            })?;

            app.manage(Arc::new(state));

            Ok(())
        })
        .on_window_event(|event| {
            if let WindowEvent::Destroyed = event.event() {
                let path_resolver = event.window().app_handle().path_resolver();
                let state = event.window().state::<Arc<State>>();
                tauri::async_runtime::block_on(async {
                    state.save(&path_resolver).await
                }).ok();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command(async)]
fn delta<R: Runtime>(window: Window<R>, path: &str, info: DeltaInfo) -> Result<(), String> {
    delta::delta_inner(window, path, info)
        .map_err(|e| format!("{e:#}\n{}", e.backtrace()))
}

#[tauri::command]
async fn create_ttmp<R: Runtime>(window: Window<R>, path: &str, info: ModInfo, groups: Vec<Group>, needed_files: NeededFiles) -> Result<(), String> {
    create_ttmp::create_ttmp_inner(window, path, info, groups, needed_files).await
        .map_err(|e| format!("{:#}\n{}", e, e.backtrace()))
}

#[tauri::command(async)]
fn deduplicate<R: Runtime>(window: Window<R>, path: &str, compression: u32, threads: usize) -> Result<(), String> {
    deduplicate::deduplicate_inner(window, path, compression, threads)
        .map_err(|e| format!("{:#}\n{}", e, e.backtrace()))
}

#[derive(Deserialize, Serialize)]
pub struct UsageInfo {
    pub file_name: String,
    pub ttmp_size: usize,
    pub hashes: HashMap<String, (usize, Vec<String>)>,
}

type GroupOptionGameArchive = (Option<String>, Option<String>, String, Option<String>);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NeededFiles {
    base_uri: Url,
    files: HashMap<String, Vec<GroupOptionGameArchive>>,
}

#[derive(Deserialize, Copy, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum SelectionType {
    Single,
    Multi,
}

impl From<SelectionType> for ttmp::model::SelectionType {
    fn from(ty: SelectionType) -> Self {
        match ty {
            SelectionType::Single => ttmp::model::SelectionType::Single,
            SelectionType::Multi => ttmp::model::SelectionType::Multi,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionItem {
    pub name: String,
    pub image_path: Option<String>,
    pub description: Option<String>,
    pub game_paths: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub name: String,
    pub selection_type: SelectionType,
    pub options: Vec<OptionItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInfo {
    name: String,
    author: String,
    version: String,
    description: String,
    url: String,
}
