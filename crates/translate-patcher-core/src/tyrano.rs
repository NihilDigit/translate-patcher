use std::collections::HashSet;

use crate::mtool::TranslationMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScenarioPatchResult {
    pub text: String,
    pub replacements: usize,
    pub used_sources: HashSet<String>,
}

pub fn patch_scenario_conservative(
    source: &str,
    translations: &TranslationMap,
) -> ScenarioPatchResult {
    let line_ending = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut output = String::with_capacity(source.len());
    let mut replacements = 0;
    let mut used_sources = HashSet::new();

    for (index, line) in source.split_inclusive('\n').enumerate() {
        if index > 0 && !output.ends_with('\n') {
            output.push_str(line_ending);
        }
        let (body, ending) = split_line_ending(line);
        let patched = patch_line(body, translations, &mut replacements, &mut used_sources);
        output.push_str(&patched);
        output.push_str(ending);
    }

    if !source.ends_with('\n') && source.is_empty() {
        output.clear();
    }

    ScenarioPatchResult {
        text: output,
        replacements,
        used_sources,
    }
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(stripped) = line.strip_suffix("\r\n") {
        (stripped, "\r\n")
    } else if let Some(stripped) = line.strip_suffix('\n') {
        (stripped, "\n")
    } else {
        (line, "")
    }
}

fn patch_line(
    line: &str,
    translations: &TranslationMap,
    replacements: &mut usize,
    used_sources: &mut HashSet<String>,
) -> String {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('*') {
        return line.to_string();
    }

    if let Some(name) = trimmed.strip_prefix('#') {
        let prefix_len = line.len() - trimmed.len();
        if let Some(translated) = translations.get(name.trim()) {
            *replacements += 1;
            used_sources.insert(name.trim().to_string());
            return format!("{}#{}", &line[..prefix_len], translated);
        }
        return line.to_string();
    }

    patch_text_segments(line, translations, replacements, used_sources)
}

fn patch_text_segments(
    line: &str,
    translations: &TranslationMap,
    replacements: &mut usize,
    used_sources: &mut HashSet<String>,
) -> String {
    let mut result = String::with_capacity(line.len());
    let mut segment = String::new();
    let mut in_tag = false;
    let chars = line.chars();

    for ch in chars {
        if in_tag {
            result.push(ch);
            if ch == ']' {
                in_tag = false;
            }
            continue;
        }

        if ch == '[' {
            flush_segment(
                &mut result,
                &mut segment,
                translations,
                replacements,
                used_sources,
            );
            result.push(ch);
            in_tag = true;
        } else {
            segment.push(ch);
        }
    }

    flush_segment(
        &mut result,
        &mut segment,
        translations,
        replacements,
        used_sources,
    );
    result
}

fn flush_segment(
    result: &mut String,
    segment: &mut String,
    translations: &TranslationMap,
    replacements: &mut usize,
    used_sources: &mut HashSet<String>,
) {
    if segment.is_empty() {
        return;
    }

    let patched = patch_segment(segment, translations, replacements, used_sources);
    result.push_str(&patched);
    segment.clear();
}

fn patch_segment(
    segment: &str,
    translations: &TranslationMap,
    replacements: &mut usize,
    used_sources: &mut HashSet<String>,
) -> String {
    let mut text = segment.to_string();

    if let Some(translated) = translations.get(segment.trim()) {
        *replacements += 1;
        used_sources.insert(segment.trim().to_string());
        return preserve_outer_space(segment, translated);
    }

    for (source, target) in translations.ordered() {
        if text.contains(source) {
            text = text.replace(source, target);
            *replacements += 1;
            used_sources.insert(source.clone());
        }
    }

    text
}

fn preserve_outer_space(original: &str, replacement: &str) -> String {
    let leading = original.len() - original.trim_start().len();
    let trailing = original.len() - original.trim_end().len();
    format!(
        "{}{}{}",
        &original[..leading],
        replacement,
        &original[original.len() - trailing..]
    )
}

#[cfg(test)]
mod tests {
    use crate::mtool::TranslationMap;

    use super::patch_scenario_conservative;

    #[test]
    fn patches_text_and_character_names_without_touching_tags() {
        let map = TranslationMap::from_slice(
            r#"{
                "玲奈": "Reina",
                "「兄さん、ごめんね…」": "「哥哥，对不起…」",
                "放課後": "放学后",
                "bg_scenario/school.png": "should-not-touch"
            }"#
            .as_bytes(),
        )
        .unwrap();

        let input =
            "#玲奈\n「兄さん、ごめんね…」[p]\n[bg storage=\"bg_scenario/school.png\"]\n放課後[p]\n";
        let patched = patch_scenario_conservative(input, &map);

        assert!(patched.text.contains("#Reina"));
        assert!(patched.text.contains("「哥哥，对不起…」[p]"));
        assert!(patched
            .text
            .contains("[bg storage=\"bg_scenario/school.png\"]"));
        assert!(patched.text.contains("放学后[p]"));
        assert_eq!(patched.replacements, 3);
    }
}
