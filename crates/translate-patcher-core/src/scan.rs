use std::path::{Path, PathBuf};

use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub scan_root: PathBuf,
    pub asar_candidates: Vec<PathBuf>,
    pub json_candidates: Vec<PathBuf>,
    pub selected_asar: Option<PathBuf>,
    pub selected_json: Option<PathBuf>,
}

pub fn scan_from(cwd: impl AsRef<Path>) -> ScanResult {
    let cwd = cwd.as_ref();
    let scan_root = cwd
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| cwd.to_path_buf());

    let mut asar_candidates = Vec::new();
    let mut json_candidates = Vec::new();

    for entry in WalkDir::new(&scan_root)
        .max_depth(5)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path().to_path_buf();
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("asar") => asar_candidates.push(path),
            Some("json") => json_candidates.push(path),
            _ => {}
        }
    }

    asar_candidates.sort();
    json_candidates.sort();

    let selected_asar = prefer_asar(cwd, &asar_candidates);
    let selected_json = prefer_json(cwd, &json_candidates);

    ScanResult {
        scan_root,
        asar_candidates,
        json_candidates,
        selected_asar,
        selected_json,
    }
}

fn prefer_asar(cwd: &Path, candidates: &[PathBuf]) -> Option<PathBuf> {
    let preferred = cwd.join("resources").join("app.asar");
    candidates
        .iter()
        .find(|path| **path == preferred)
        .or_else(|| {
            candidates
                .iter()
                .find(|path| path.ends_with("resources/app.asar"))
        })
        .or_else(|| {
            candidates
                .iter()
                .find(|path| path.file_name().is_some_and(|name| name == "app.asar"))
        })
        .or_else(|| candidates.first())
        .cloned()
}

fn prefer_json(cwd: &Path, candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|path| path.parent() == Some(cwd))
        .or_else(|| candidates.first())
        .cloned()
}
