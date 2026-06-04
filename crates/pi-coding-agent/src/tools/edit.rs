use crate::tools::path::resolve_to_cwd;
use pi_agent_core::{AgentTool, ToolFn};
use pi_ai::types::ContentBlock;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use unicode_normalization::UnicodeNormalization;

const DESCRIPTION: &str = "Edit a single file using exact text replacement. Every edits[].oldText must match a unique, non-overlapping region of the original file. Merge nearby changes into one edit; do not include large unchanged regions.";

struct Edit {
    old_text: String,
    new_text: String,
}

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type":"object",
        "properties":{
            "path":{"type":"string"},
            "edits":{"type":"array","items":{"type":"object",
                "properties":{"oldText":{"type":"string"},"newText":{"type":"string"}},
                "required":["oldText","newText"],"additionalProperties":false}}
        },
        "required":["path","edits"],"additionalProperties":false
    })
}

fn detect_crlf(s: &str) -> bool {
    match (s.find("\r\n"), s.find('\n')) {
        (Some(rn), Some(n)) => rn <= n,
        _ => s.contains("\r\n"),
    }
}

fn normalize_to_lf(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn restore_crlf(s: &str, crlf: bool) -> String {
    if crlf {
        s.replace('\n', "\r\n")
    } else {
        s.to_string()
    }
}

fn strip_bom(s: &str) -> (&str, &str) {
    if let Some(r) = s.strip_prefix('\u{feff}') {
        ("\u{feff}", r)
    } else {
        ("", s)
    }
}

fn normalize_for_fuzzy(text: &str) -> String {
    let nfkc: String = text.nfkc().collect();
    let trimmed: String = nfkc
        .split('\n')
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    trimmed
        .chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => '\'',
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => '"',
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
            | '\u{2212}' => '-',
            '\u{00A0}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{2002}'..='\u{200A}' => {
                ' '
            }
            other => other,
        })
        .collect()
}

fn count_occurrences(content: &str, old: &str) -> usize {
    let fc = normalize_for_fuzzy(content);
    let fo = normalize_for_fuzzy(old);
    if fo.is_empty() {
        return 0;
    }
    fc.matches(&fo).count()
}

fn apply_edits(normalized: &str, edits: &[Edit], path: &str) -> Result<(String, String), String> {
    let total = edits.len();
    let norm: Vec<Edit> = edits
        .iter()
        .map(|e| Edit {
            old_text: normalize_to_lf(&e.old_text),
            new_text: normalize_to_lf(&e.new_text),
        })
        .collect();

    for (i, e) in norm.iter().enumerate() {
        if e.old_text.is_empty() {
            return Err(if total == 1 {
                format!("oldText must not be empty in {path}.")
            } else {
                format!("edits[{i}].oldText must not be empty in {path}.")
            });
        }
    }

    let any_fuzzy = norm.iter().any(|e| {
        !normalized.contains(&e.old_text)
            && normalize_for_fuzzy(normalized).contains(&normalize_for_fuzzy(&e.old_text))
    });
    let base = if any_fuzzy {
        normalize_for_fuzzy(normalized)
    } else {
        normalized.to_string()
    };

    let mut matched: Vec<(usize, usize, usize, String)> = Vec::new();

    for (i, e) in norm.iter().enumerate() {
        let (idx, len, new) = if any_fuzzy {
            let fo = normalize_for_fuzzy(&e.old_text);
            match base.find(&fo) {
                Some(ix) => (ix, fo.len(), e.new_text.clone()),
                None => return Err(not_found(path, i, total)),
            }
        } else {
            match base.find(&e.old_text) {
                Some(ix) => (ix, e.old_text.len(), e.new_text.clone()),
                None => return Err(not_found(path, i, total)),
            }
        };
        let occ = count_occurrences(&base, &e.old_text);
        if occ > 1 {
            return Err(duplicate(path, i, total, occ));
        }
        matched.push((i, idx, len, new));
    }

    matched.sort_by_key(|m| m.1);
    for w in matched.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if a.1 + a.2 > b.1 {
            return Err(format!(
                "edits[{}] and edits[{}] overlap in {path}. Merge them into one edit or target disjoint regions.",
                a.0, b.0
            ));
        }
    }

    let mut new_content = base.clone();
    for (_, idx, len, new) in matched.iter().rev() {
        new_content.replace_range(*idx..*idx + *len, new);
    }
    if base == new_content {
        return Err(if total == 1 {
            format!("No changes made to {path}. The replacement produced identical content. This might indicate an issue with special characters or the text not existing as expected.")
        } else {
            format!("No changes made to {path}. The replacements produced identical content.")
        });
    }
    Ok((base, new_content))
}

fn not_found(path: &str, i: usize, total: usize) -> String {
    if total == 1 {
        format!("Could not find the exact text in {path}. The old text must match exactly including all whitespace and newlines.")
    } else {
        format!("Could not find edits[{i}] in {path}. The oldText must match exactly including all whitespace and newlines.")
    }
}

fn duplicate(path: &str, i: usize, total: usize, n: usize) -> String {
    if total == 1 {
        format!("Found {n} occurrences of the text in {path}. The text must be unique. Please provide more context to make it unique.")
    } else {
        format!("Found {n} occurrences of edits[{i}] in {path}. Each oldText must be unique. Please provide more context to make it unique.")
    }
}

fn parse_edits(args: &serde_json::Value) -> Result<Vec<Edit>, String> {
    let mut edits_val = args
        .get("edits")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    if let Some(s) = edits_val.as_str() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
            edits_val = v;
        }
    }
    let mut out: Vec<Edit> = Vec::new();
    if let Some(arr) = edits_val.as_array() {
        for e in arr {
            let o = e.get("oldText").and_then(|v| v.as_str());
            let n = e.get("newText").and_then(|v| v.as_str());
            if let (Some(o), Some(n)) = (o, n) {
                out.push(Edit {
                    old_text: o.into(),
                    new_text: n.into(),
                });
            }
        }
    }
    if let (Some(o), Some(n)) = (
        args.get("oldText").and_then(|v| v.as_str()),
        args.get("newText").and_then(|v| v.as_str()),
    ) {
        out.push(Edit {
            old_text: o.into(),
            new_text: n.into(),
        });
    }
    if out.is_empty() {
        return Err("Edit tool input is invalid. edits must contain at least one replacement.".into());
    }
    Ok(out)
}

pub async fn edit_execute(
    cwd: &Path,
    args: serde_json::Value,
) -> Result<Vec<ContentBlock>, String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("edit: missing or non-string 'path' argument")?
        .to_string();
    let edits = parse_edits(&args)?;
    let abs = resolve_to_cwd(&path, cwd);
    let raw = tokio::fs::read(&abs)
        .await
        .map_err(|e| format!("Could not edit file: {path}. {e}."))?;
    let content = String::from_utf8_lossy(&raw).into_owned();
    let (bom, body) = strip_bom(&content);
    let crlf = detect_crlf(body);
    let normalized = normalize_to_lf(body);
    let (_base, new_content) = apply_edits(&normalized, &edits, &path)?;
    let final_content = format!("{bom}{}", restore_crlf(&new_content, crlf));
    tokio::fs::write(&abs, final_content)
        .await
        .map_err(|e| format!("edit: failed to write {}: {e}", abs.display()))?;
    Ok(vec![ContentBlock::Text {
        text: format!("Successfully replaced {} block(s) in {path}.", edits.len()),
        text_signature: None,
    }])
}

pub fn edit_tool(cwd: PathBuf) -> AgentTool {
    let execute: ToolFn = Arc::new(move |args| {
        let cwd = cwd.clone();
        Box::pin(async move { edit_execute(&cwd, args).await })
    });
    AgentTool {
        name: "edit".into(),
        description: DESCRIPTION.into(),
        parameters: schema(),
        execute,
    }
}
