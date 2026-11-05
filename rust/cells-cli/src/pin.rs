//! `cells pin`: every import of a file from a URL pinned by the hash of what the URL serves
//! now, written into its `with { sha256: "…" }`; the import line's comment kept.

use std::fs;
use std::path::Path;

use crate::disk::{cache, fetch};
use crate::json;
use cells::Statement;
use cells::files::is_url;

/// The import statement written again, its hash `sha256`, its other attributes kept.
fn statement(default: &Option<String>, names: &[String], from: &str, with: &[(String, String)], sha256: &str) -> String {
    let head = match default {
        Some(d) => d.clone(),
        None => format!("{{ {} }}", names.join(", ")),
    };
    let mut attrs = vec![format!("sha256: {}", json::quote(sha256))];
    attrs.extend(with.iter().filter(|(k, _)| k != "sha256").map(|(k, v)| format!("{k}: {}", json::quote(v))));
    format!("import {head} from {} with {{ {} }}", json::quote(from), attrs.join(", "))
}

/// The file at `path` with each URL it imports pinned: what changed, a line each.
pub fn pin(path: &Path) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let bom = if text.starts_with('\u{feff}') { 3 } else { 0 };
    if !text[bom..].starts_with("---") {
        return Ok(Vec::new());
    }
    // The imports that read are pinned; a statement that does not is `cells check`'s to say.
    let statements = cells::prelude::parse(&text, bom).map_err(|why| format!("{} the prelude: {why}", path.display()))?.statements;
    let mut lines: Vec<String> = text[bom..].split_inclusive('\n').map(str::to_owned).collect();
    let mut said = Vec::new();
    for s in &statements {
        let Statement::Import { line, default, names, from, with } = s else { continue };
        if !is_url(from) {
            continue;
        }
        let bytes = fetch(from).ok_or_else(|| format!("{} prelude line {line}: {from} cannot be fetched", path.display()))?;
        let hash = cells::sha256::hex(&bytes);
        let was = with.iter().find(|(k, _)| k == "sha256").map(|(_, v)| v.as_str());
        if was == Some(hash.as_str()) {
            continue;
        }
        fs::create_dir_all(cache()).map_err(|e| e.to_string())?;
        fs::write(cache().join(&hash), &bytes).map_err(|e| e.to_string())?;
        let l = &lines[*line as usize];
        let content = l.trim_end_matches(['\r', '\n']);
        let ending = &l[content.len()..];
        let said_part = cells::prelude::uncomment(content);
        let kept = &content[said_part.trim_end_matches([' ', '\t']).len()..];
        let indent = &content[..content.len() - content.trim_start_matches([' ', '\t']).len()];
        lines[*line as usize] = format!("{indent}{}{kept}{ending}", statement(default, names, from, with, &hash));
        said.push(match was {
            Some(w) => format!("{} prelude line {line}: {from} pinned {hash}, was {w}", path.display()),
            None => format!("{} prelude line {line}: {from} pinned {hash}", path.display()),
        });
    }
    if !said.is_empty() {
        let out = format!("{}{}", &text[..bom], lines.concat());
        let tmp = path.with_extension("cells.pin");
        fs::write(&tmp, out).map_err(|e| format!("{}: {e}", tmp.display()))?;
        fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(said)
}
