//! `cells pack`: every import of a file written into its prelude as a packed copy, so the
//! file reads whole where its imports cannot be reached (SPEC.md, Packed imports): in place,
//! or as a self-contained copy elsewhere (`--out`), or every copy taken out (`--remove`).
//! The text is the engine's (`Doc::packed`), the same call Cells.app's Pack Imports and its
//! self-contained export make.

use std::fs;
use std::path::Path;

use cells::{Pack, Reach, Statement};

/// What `cells pack`'s arguments say: whether it removes, where it writes, the file.
pub fn args(args: &[String]) -> Option<(bool, Option<&str>, &str)> {
    let (mut remove, mut out, mut file) = (false, None, None);
    let mut at = 0;
    while at < args.len() {
        match args[at].as_str() {
            "--remove" => remove = true,
            "--out" => {
                out = Some(args.get(at + 1)?.as_str());
                at += 1;
            }
            f if file.is_none() && !f.starts_with("--") => file = Some(f),
            _ => return None,
        }
        at += 1;
    }
    Some((remove, out, file?))
}

/// The file packed as asked: what was done, a line an import, and whether every import
/// was packed (one whose source is not reached and has no copy cannot be).
pub fn pack(path: &Path, remove: bool, out: Option<&Path>) -> Result<(Vec<String>, bool), String> {
    let shown = path.display();
    let bytes = fs::read(path).map_err(|e| format!("{shown}: {e}"))?;
    let doc = cells_disk::read(path, bytes.clone(), 1).map_err(|e| format!("{shown} {e}"))?;
    let text = doc.packed(if remove { Pack::Remove } else { Pack::All });
    let mut said = Vec::new();
    let mut whole = true;
    for (i, s) in doc.statements().iter().enumerate() {
        let Statement::Import { line, from, .. } = s else { continue };
        let reach = doc.reach(i).expect("an import has a reach");
        let place = format!("{shown} prelude line {line}: {}", cells::say::code(from));
        said.push(match (remove, reach) {
            (true, _) => format!("{place}: its copies taken out"),
            (false, Reach::Reached) => format!("{place}: packed"),
            (false, Reach::Standing) => format!("{place}: not reachable, its copy kept as it is"),
            (false, Reach::Missing) => {
                whole = false;
                format!("{place}: not reachable and never packed, so it is not in the file")
            }
        });
    }
    match out {
        Some(to) => fs::write(to, &text).map_err(|e| format!("{}: {e}", to.display()))?,
        None if text.as_bytes() != bytes => {
            let tmp = path.with_extension("cells.pack");
            fs::write(&tmp, &text).map_err(|e| format!("{}: {e}", tmp.display()))?;
            fs::rename(&tmp, path).map_err(|e| format!("{shown}: {e}"))?;
        }
        None => {}
    }
    if let Some(to) = out {
        said.push(format!("{}: written, {shown} untouched", to.display()));
    }
    Ok((said, whole))
}
