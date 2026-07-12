use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::mtool::TranslationMap;

const JSON_AUTO_DETECT_LIMIT: usize = 5;

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
        .filter(|path| path.parent() == Some(cwd))
        .chain(candidates.iter().filter(|path| path.parent() != Some(cwd)))
        .take(JSON_AUTO_DETECT_LIMIT)
        .find(|path| {
            TranslationMap::from_path(path).is_ok_and(|translations| !translations.is_empty())
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{prefer_json, JSON_AUTO_DETECT_LIMIT};

    #[test]
    fn skips_non_translation_json_when_auto_selecting() {
        let temp = tempdir().unwrap();
        let runtime_config = temp.path().join("vk_swiftshader_icd.json");
        let translations = temp.path().join("translations.json");
        fs::write(&runtime_config, r#"{"ICD":{"library_path":"vulkan.dll"}}"#).unwrap();
        fs::write(&translations, r#"{"こんにちは":"你好"}"#).unwrap();

        let candidates = vec![runtime_config, translations.clone()];

        assert_eq!(prefer_json(temp.path(), &candidates), Some(translations));
    }

    #[test]
    fn tries_at_most_five_json_candidates() {
        let temp = tempdir().unwrap();
        let mut candidates = Vec::new();
        for index in 0..JSON_AUTO_DETECT_LIMIT {
            let path = temp.path().join(format!("{index}.json"));
            fs::write(&path, r#"{"config":{"enabled":true}}"#).unwrap();
            candidates.push(path);
        }
        let translations = temp.path().join("translations.json");
        fs::write(&translations, r#"{"こんにちは":"你好"}"#).unwrap();
        candidates.push(translations);

        assert_eq!(prefer_json(temp.path(), &candidates), None);
    }
}
