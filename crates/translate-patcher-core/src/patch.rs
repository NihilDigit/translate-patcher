use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use time::{format_description::FormatItem, macros::format_description, OffsetDateTime};

use crate::{
    asar::{write_asar, AsarArchive, Replacement},
    mtool::TranslationMap,
    tyrano::patch_scenario_conservative,
    Backend,
};

const BACKUP_FORMAT: &[FormatItem<'_>] =
    format_description!("[year repr:last_two][month][day]-[hour][minute][second]");

#[derive(Debug, Clone)]
pub struct PatchPreview {
    pub backend: Backend,
    pub asar_path: PathBuf,
    pub json_path: PathBuf,
    pub translation_entries: usize,
    pub scenario_files: usize,
    pub estimated_matches: usize,
    pub backup_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PatchReport {
    pub backend: Backend,
    pub asar_path: PathBuf,
    pub json_path: PathBuf,
    pub backup_path: PathBuf,
    pub report_path: PathBuf,
    pub modified_files: usize,
    pub applied_entries: usize,
    pub unused_entries: usize,
}

pub fn preview_patch(
    asar_path: impl AsRef<Path>,
    json_path: impl AsRef<Path>,
) -> Result<PatchPreview> {
    let asar_path = asar_path.as_ref().to_path_buf();
    let json_path = json_path.as_ref().to_path_buf();
    let archive = AsarArchive::from_path(&asar_path)?;
    let translations = TranslationMap::from_path(&json_path)?;

    let mut scenario_files = 0;
    let mut used = HashSet::new();
    for file in archive
        .files()
        .into_iter()
        .filter(|file| is_tyrano_scenario(&file.path))
    {
        scenario_files += 1;
        let text = String::from_utf8_lossy(archive.read_file(&file.path)?);
        let patched = patch_scenario_conservative(&text, &translations);
        used.extend(patched.used_sources);
    }

    Ok(PatchPreview {
        backend: Backend::TyranoAsar,
        asar_path: asar_path.clone(),
        json_path,
        translation_entries: translations.len(),
        scenario_files,
        estimated_matches: used.len(),
        backup_path: backup_path_for(&asar_path)?,
    })
}

pub fn apply_patch(preview: &PatchPreview) -> Result<PatchReport> {
    if preview.translation_entries == 0 {
        bail!("translation file has no usable entries");
    }

    let archive = AsarArchive::from_path(&preview.asar_path)?;
    let translations = TranslationMap::from_path(&preview.json_path)?;
    let mut replacements = Vec::new();
    let mut used = HashSet::new();

    for file in archive
        .files()
        .into_iter()
        .filter(|file| is_tyrano_scenario(&file.path))
    {
        let original = String::from_utf8_lossy(archive.read_file(&file.path)?);
        let patched = patch_scenario_conservative(&original, &translations);
        if patched.replacements > 0 && patched.text != original {
            used.extend(patched.used_sources);
            replacements.push(Replacement {
                path: file.path,
                content: patched.text.into_bytes(),
            });
        }
    }

    fs::copy(&preview.asar_path, &preview.backup_path)
        .with_context(|| format!("failed to create backup {}", preview.backup_path.display()))?;

    let out = archive.repack_with_replacements(&replacements)?;
    write_asar(&preview.asar_path, &out)?;

    let report_path = preview
        .asar_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("translate-patcher-report.txt");

    let report = PatchReport {
        backend: preview.backend,
        asar_path: preview.asar_path.clone(),
        json_path: preview.json_path.clone(),
        backup_path: preview.backup_path.clone(),
        report_path,
        modified_files: replacements.len(),
        applied_entries: used.len(),
        unused_entries: translations.len().saturating_sub(used.len()),
    };

    write_report(&report)?;
    Ok(report)
}

pub fn restore_backup(asar_path: impl AsRef<Path>, backup_path: impl AsRef<Path>) -> Result<()> {
    let asar_path = asar_path.as_ref();
    let backup_path = backup_path.as_ref();
    fs::copy(backup_path, asar_path).with_context(|| {
        format!(
            "failed to restore {} to {}",
            backup_path.display(),
            asar_path.display()
        )
    })?;
    Ok(())
}

pub fn is_tyrano_scenario(path: &str) -> bool {
    path.starts_with("data/scenario/") && path.ends_with(".ks")
}

fn backup_path_for(asar_path: &Path) -> Result<PathBuf> {
    let timestamp = OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(BACKUP_FORMAT)?;
    let file_name = asar_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("app.asar");
    Ok(asar_path.with_file_name(format!("{file_name}.{timestamp}.bak")))
}

fn write_report(report: &PatchReport) -> Result<()> {
    let text = format!(
        "\
translate-patcher report

backend: {}
asar: {}
json: {}
backup: {}
modified files: {}
applied entries: {}
unused entries: {}
",
        report.backend.label(),
        report.asar_path.display(),
        report.json_path.display(),
        report.backup_path.display(),
        report.modified_files,
        report.applied_entries,
        report.unused_entries,
    );
    fs::write(&report.report_path, text)
        .with_context(|| format!("failed to write {}", report.report_path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::asar::AsarArchive;

    use super::{apply_patch, preview_patch};

    #[test]
    fn applies_patch_and_writes_backup_and_report() {
        let dir = tempdir().unwrap();
        let resources = dir.path().join("resources");
        fs::create_dir(&resources).unwrap();
        let asar_path = resources.join("app.asar");
        let json_path = dir.path().join("game.json");

        fs::write(
            &asar_path,
            test_asar(&[
                (
                    "data/scenario/scene1.ks",
                    "#玲奈\n「兄さん、ごめんね…」[p]\n[bg storage=\"bg.png\"]\n",
                ),
                ("main.js", "console.log('ok')"),
            ]),
        )
        .unwrap();
        fs::write(
            &json_path,
            r#"{
                "玲奈": "Reina",
                "「兄さん、ごめんね…」": "「哥哥，对不起…」",
                "bg.png": "should-not-touch"
            }"#,
        )
        .unwrap();

        let preview = preview_patch(&asar_path, &json_path).unwrap();
        assert_eq!(preview.translation_entries, 3);
        assert_eq!(preview.scenario_files, 1);
        assert_eq!(preview.estimated_matches, 2);

        let report = apply_patch(&preview).unwrap();
        assert_eq!(report.modified_files, 1);
        assert_eq!(report.applied_entries, 2);
        assert!(report.backup_path.exists());
        assert!(report.report_path.exists());

        let archive = AsarArchive::from_path(&asar_path).unwrap();
        let scene = String::from_utf8(
            archive
                .read_file("data/scenario/scene1.ks")
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(scene.contains("#Reina"));
        assert!(scene.contains("「哥哥，对不起…」[p]"));
        assert!(scene.contains("[bg storage=\"bg.png\"]"));
    }

    fn test_asar(files: &[(&str, &str)]) -> Vec<u8> {
        let mut header = serde_json::json!({ "files": {} });
        let mut offset = 0usize;
        for (path, content) in files {
            insert_entry(&mut header, path, offset, content.len());
            offset += content.len();
        }
        let header_json = serde_json::to_vec(&header).unwrap();
        let padding = (4 - (header_json.len() % 4)) % 4;
        let inner_size = 4 + header_json.len() + padding;
        let packed_header_size = inner_size + 4;
        let mut out = Vec::new();
        out.extend_from_slice(&4u32.to_le_bytes());
        out.extend_from_slice(&(packed_header_size as u32).to_le_bytes());
        out.extend_from_slice(&(inner_size as u32).to_le_bytes());
        out.extend_from_slice(&(header_json.len() as u32).to_le_bytes());
        out.extend_from_slice(&header_json);
        out.extend(std::iter::repeat(0).take(padding));
        for (_, content) in files {
            out.extend_from_slice(content.as_bytes());
        }
        out
    }

    fn insert_entry(header: &mut serde_json::Value, path: &str, offset: usize, size: usize) {
        let mut node = header;
        let parts: Vec<_> = path.split('/').collect();
        for part in &parts[..parts.len() - 1] {
            let files = node.get_mut("files").unwrap().as_object_mut().unwrap();
            node = files
                .entry((*part).to_string())
                .or_insert_with(|| serde_json::json!({ "files": {} }));
        }
        node.get_mut("files")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(
                parts.last().unwrap().to_string(),
                serde_json::json!({ "offset": offset.to_string(), "size": size }),
            );
    }
}
