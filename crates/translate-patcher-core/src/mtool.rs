use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct TranslationMap {
    entries: HashMap<String, String>,
    ordered: Vec<(String, String)>,
}

impl TranslationMap {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        Self::from_slice(&bytes).with_context(|| format!("failed to parse {}", path.display()))
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        let raw: HashMap<String, String> = serde_json::from_slice(bytes)?;
        let entries: HashMap<String, String> = raw
            .into_iter()
            .filter(|(source, target)| !source.is_empty() && !target.is_empty() && source != target)
            .collect();
        let mut ordered: Vec<_> = entries
            .iter()
            .map(|(source, target)| (source.clone(), target.clone()))
            .collect();
        ordered.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
        Ok(Self { entries, ordered })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, source: &str) -> Option<&str> {
        self.entries.get(source).map(String::as_str)
    }

    pub fn ordered(&self) -> &[(String, String)] {
        &self.ordered
    }
}
