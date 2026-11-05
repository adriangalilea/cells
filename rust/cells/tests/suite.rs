//! The conformance suite (tests/README.md): every case under `Cells_SUITE`, else the
//! repository's `tests/`, read, compared and written back. Prints every case it fails
//! with its path and fails if any does.

use std::fs;
use std::path::{Path, PathBuf};

use cells::{Base, ColumnAt, Dialect, Doc, Naming, Origin, Separator, Value};

/// JSON, as much of it as the suite writes.
#[derive(Clone, Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Number(String),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(o) => o.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn str(&self) -> &str {
        match self {
            Json::Str(s) => s,
            other => panic!("a string was expected, not {other:?}"),
        }
    }

    fn array(&self) -> &[Json] {
        match self {
            Json::Array(a) => a,
            other => panic!("an array was expected, not {other:?}"),
        }
    }
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn value(&mut self) -> Json {
        self.ws();
        let c = self.s[self.i];
        match c {
            b'{' => {
                self.i += 1;
                let mut o = Vec::new();
                loop {
                    self.ws();
                    if self.s[self.i] == b'}' {
                        self.i += 1;
                        return Json::Object(o);
                    }
                    let Json::Str(k) = self.value() else { panic!("a key") };
                    self.ws();
                    assert_eq!(self.s[self.i], b':');
                    self.i += 1;
                    let v = self.value();
                    o.push((k, v));
                    self.ws();
                    if self.s[self.i] == b',' {
                        self.i += 1;
                    }
                }
            }
            b'[' => {
                self.i += 1;
                let mut a = Vec::new();
                loop {
                    self.ws();
                    if self.s[self.i] == b']' {
                        self.i += 1;
                        return Json::Array(a);
                    }
                    a.push(self.value());
                    self.ws();
                    if self.s[self.i] == b',' {
                        self.i += 1;
                    }
                }
            }
            b'"' => {
                self.i += 1;
                let mut out = String::new();
                loop {
                    let c = self.s[self.i];
                    self.i += 1;
                    match c {
                        b'"' => return Json::Str(out),
                        b'\\' => {
                            let e = self.s[self.i];
                            self.i += 1;
                            match e {
                                b'n' => out.push('\n'),
                                b'r' => out.push('\r'),
                                b't' => out.push('\t'),
                                b'b' => out.push('\u{8}'),
                                b'f' => out.push('\u{c}'),
                                b'u' => {
                                    let hex = |p: &mut Parser| {
                                        let h = std::str::from_utf8(&p.s[p.i..p.i + 4]).unwrap();
                                        p.i += 4;
                                        u32::from_str_radix(h, 16).unwrap()
                                    };
                                    let mut cp = hex(self);
                                    if (0xD800..0xDC00).contains(&cp) {
                                        assert_eq!(&self.s[self.i..self.i + 2], b"\\u");
                                        self.i += 2;
                                        let lo = hex(self);
                                        cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                    }
                                    out.push(char::from_u32(cp).unwrap());
                                }
                                other => out.push(other as char),
                            }
                        }
                        _ => {
                            // Re-read multi-byte UTF-8 whole.
                            let start = self.i - 1;
                            let len = match c {
                                0..0x80 => 1,
                                0xC0..0xE0 => 2,
                                0xE0..0xF0 => 3,
                                _ => 4,
                            };
                            out.push_str(std::str::from_utf8(&self.s[start..start + len]).unwrap());
                            self.i = start + len;
                        }
                    }
                }
            }
            b't' => {
                self.i += 4;
                Json::Bool(true)
            }
            b'f' => {
                self.i += 5;
                Json::Bool(false)
            }
            b'n' => {
                self.i += 4;
                Json::Null
            }
            _ => {
                let start = self.i;
                while self.i < self.s.len() && matches!(self.s[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    self.i += 1;
                }
                Json::Number(String::from_utf8(self.s[start..self.i].to_vec()).unwrap())
            }
        }
    }
}

fn json(text: &str) -> Json {
    let mut p = Parser { s: text.as_bytes(), i: 0 };
    p.value()
}

/// A decimal string in one spelling: no sign on zero, no leading or trailing zeros.
fn normal(n: &str) -> String {
    let (neg, digits) = match n.strip_prefix('-') {
        Some(d) => (true, d),
        None => (false, n.strip_prefix('+').unwrap_or(n)),
    };
    let (int, frac) = digits.split_once('.').unwrap_or((digits, ""));
    let int = int.trim_start_matches('0');
    let frac = frac.trim_end_matches('0');
    let int = if int.is_empty() { "0" } else { int };
    let body = if frac.is_empty() { int.to_string() } else { format!("{int}.{frac}") };
    if neg && body != "0" { format!("-{body}") } else { body }
}

/// Our value as the suite tags it: the kind and its text.
fn tagged(doc: &Doc, row: usize, column: usize) -> Option<(String, String, Option<String>)> {
    let c = &doc.columns()[column];
    let v = c.value(row);
    let tag = c.ty.base.tag().to_string();
    let currency = match &c.ty.base {
        Base::Money(cur) => Some(cur.as_str().to_string()),
        _ => None,
    };
    let text = match v {
        Value::Null => return None,
        Value::Text(t) | Value::Choice(t) => t.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Decimal(d) => normal(&d.to_string()),
        Value::Bool(b) => b.to_string(),
        Value::Date(d) => cells::value::format_date(d),
        Value::Time(t) => cells::value::format_time(t),
        Value::DateTime(t) => cells::value::format_datetime(t),
    };
    Some((tag, text, currency))
}

fn expected(v: &Json) -> Option<(String, String, Option<String>)> {
    let Json::Object(o) = v else { return None };
    // `{ "from": "formula" }`: a computed cell that gave nothing.
    let (tag, val) = o.iter().find(|(k, _)| k != "currency" && k != "from")?;
    let text = match val {
        Json::Str(s) => s.clone(),
        Json::Bool(b) => b.to_string(),
        Json::Number(n) => n.clone(),
        other => panic!("a value {other:?}"),
    };
    let numeric = matches!(tag.as_str(), "int" | "num" | "pct" | "money");
    let text = if numeric { normal(&text) } else { text };
    Some((tag.clone(), text, v.get("currency").map(|c| c.str().to_string())))
}

fn check_read(doc: &Doc, want: &Json) -> Vec<String> {
    let mut fails = Vec::new();
    let cols = want.get("columns").unwrap().array();
    if cols.len() != doc.columns().len() {
        fails.push(format!("{} columns, the suite says {}", doc.columns().len(), cols.len()));
        return fails;
    }
    for (k, (w, c)) in cols.iter().zip(doc.columns()).enumerate() {
        let name = w.get("name").unwrap().str();
        let ty = w.get("type").unwrap().str();
        if c.name != name {
            fails.push(format!("column {k}: name {:?}, the suite says {name:?}", c.name));
        }
        let ours = c.ty.shorthand();
        if ours != ty {
            fails.push(format!("column {name}: type {ours:?}, the suite says {ty:?}"));
        }
        let inferred = w.get("inferred") == Some(&Json::Bool(true));
        if c.ty.inferred != inferred {
            fails.push(format!("column {name}: inferred {}, the suite says {inferred}", c.ty.inferred));
        }
        let formula = w.get("formula").map(|e| e.str());
        if c.formula.as_deref() != formula {
            fails.push(format!("column {name}: formula {:?}, the suite says {formula:?}", c.formula));
        }
    }
    if let Some(Json::Object(types)) = want.get("types") {
        for (name, def) in types {
            match doc.named().iter().find(|t| &t.name == name) {
                None => fails.push(format!("no named type {name}")),
                Some(t) if t.ty.shorthand() != def.str() => {
                    fails.push(format!("type {name}: {:?}, the suite says {:?}", t.ty.shorthand(), def.str()))
                }
                Some(_) => {}
            }
        }
        if types.len() != doc.named().len() {
            fails.push(format!("{} named types, the suite says {}", doc.named().len(), types.len()));
        }
    }
    let rows = want.get("rows").unwrap().array();
    if rows.len() != doc.rows() {
        fails.push(format!("{} rows, the suite says {}", doc.rows(), rows.len()));
    } else {
        for (r, w) in rows.iter().enumerate() {
            for (k, c) in doc.columns().iter().enumerate() {
                let Some(wv) = w.get(&c.name) else {
                    fails.push(format!("row {r}: the suite has no {}", c.name));
                    continue;
                };
                let ours = tagged(doc, r, k);
                let theirs = expected(wv);
                if ours != theirs {
                    fails.push(format!("row {r}, {}: {ours:?}, the suite says {theirs:?}", c.name));
                }
                let from = match c.origin(r) {
                    Origin::Written => None,
                    Origin::Formula => Some("formula"),
                    Origin::Cell => Some("cell"),
                };
                let said = wv.get("from").map(Json::str);
                if from != said {
                    fails.push(format!("row {r}, {}: from {from:?}, the suite says {said:?}", c.name));
                }
            }
        }
    }
    let problems = want.get("problems").unwrap().array();
    let spans = doc.spans();
    let ours: Vec<(u32, String, &str, (u32, u32))> = doc
        .problems()
        .iter()
        .zip(&spans)
        .map(|(p, s)| (p.row, doc.columns()[p.column as usize].name.clone(), p.kind().as_str(), (s.start, s.len)))
        .collect();
    let theirs: Vec<(u32, String, &str, (u32, u32))> = problems
        .iter()
        .map(|p| {
            let Json::Number(n) = p.get("row").unwrap() else { panic!("a row") };
            (n.parse().unwrap(), p.get("column").unwrap().str().to_string(), p.get("kind").unwrap().str(), span(p))
        })
        .collect();
    if ours != theirs {
        fails.push(format!("problems {ours:?}, the suite says {theirs:?}"));
    }
    let ours: Vec<(String, (u32, u32))> = doc
        .broken()
        .iter()
        .zip(doc.broken_spans())
        .map(|(e, s)| (e.at.to_string(), s.map_or((u32::MAX, 0), |s| (s.start, s.len))))
        .collect();
    let theirs: Vec<(String, (u32, u32))> = want
        .get("broken")
        .map(|b| b.array())
        .unwrap_or_default()
        .iter()
        .map(|b| (b.get("at").unwrap().str().to_string(), span(b)))
        .collect();
    if ours != theirs {
        let whys: Vec<String> = doc.broken().iter().map(|e| e.to_string()).collect();
        fails.push(format!("broken at {ours:?}, the suite says {theirs:?} ({})", whys.join("; ")));
    }
    fails
}

/// A problem's or a break's `span`, `[start, length]` in bytes.
fn span(p: &Json) -> (u32, u32) {
    let s = p.get("span").expect("a span").array();
    let n = |j: &Json| match j {
        Json::Number(n) => n.parse().unwrap(),
        other => panic!("a span's number, not {other:?}"),
    };
    (n(&s[0]), n(&s[1]))
}

/// The edits applied in order, and the file's text after them: as written, or after a last
/// `as` edit the file written in that dialect.
fn apply(doc: &mut Doc, edits: &Json) -> Result<String, String> {
    let column = |doc: &Doc, e: &Json, key: &str| {
        let name = e.get(key).unwrap().str().to_string();
        doc.column(&name).ok_or_else(|| format!("no column {name}"))
    };
    for e in edits.array() {
        if let Some(to) = e.get("as") {
            let dialect = match to.str() {
                "cells" => Dialect::Cells,
                s => Dialect::Plain(Separator::of(s.as_bytes()[0]).ok_or_else(|| format!("no separator {s:?}"))?),
            };
            let header: Option<Vec<&str>> = e.get("header").map(|h| h.array().iter().map(Json::str).collect());
            return Ok(doc.written_as(dialect, header.as_deref()));
        }
        let stored: Vec<ColumnAt> = (0..doc.columns().len()).map(ColumnAt::Kept).collect();
        let reshaped = if let Some(Json::Number(at)) = e.get("insert") {
            let cells: Vec<&str> = e.get("cells").map(|c| c.array().iter().map(Json::str).collect()).unwrap_or_default();
            let mut order = stored;
            order.insert(at.parse().unwrap(), ColumnAt::New(e.get("header").unwrap().str(), &cells));
            Some(doc.reshaped(&order, 1))
        } else if e.get("remove").is_some() {
            let k = column(doc, e, "remove")?;
            let mut order = stored;
            order.remove(k);
            Some(doc.reshaped(&order, 1))
        } else if e.get("move").is_some() {
            let k = column(doc, e, "move")?;
            let Some(Json::Number(to)) = e.get("to") else { panic!("a place") };
            let mut order = stored;
            let moved = order.remove(k);
            order.insert(to.parse().unwrap(), moved);
            Some(doc.reshaped(&order, 1))
        } else if e.get("rename").is_some() {
            let k = column(doc, e, "rename")?;
            let mut order = stored;
            order[k] = ColumnAt::Renamed(k, e.get("to").unwrap().str());
            Some(doc.reshaped(&order, 1))
        } else {
            None
        };
        if let Some(r) = reshaped {
            if let Some(d) = taken(r, e)? {
                *doc = d;
            }
            continue;
        }
        let name = e.get("column").unwrap().str();
        let col = doc.column(name).ok_or_else(|| format!("no column {name}"))?;
        if let Some(ty) = e.get("type") {
            taken(doc.set_type(col, ty.str()), e)?;
            continue;
        }
        let Json::Number(row) = e.get("row").unwrap() else { panic!("a row") };
        let row: usize = row.parse().unwrap();
        let set = if let Some(t) = e.get("text") {
            doc.set_cell(row, col, t.str())
        } else {
            let v = e.get("value").unwrap();
            let held;
            let value = match expected(v) {
                None => Value::Null,
                Some((tag, text, _)) => {
                    held = text;
                    let b = held.as_bytes();
                    match tag.as_str() {
                        "bool" => Value::Bool(held == "true"),
                        "int" => Value::Int(cells::value::parse_int(b).unwrap()),
                        "num" | "pct" | "money" => Value::Decimal(cells::Decimal::parse(b).unwrap()),
                        "date" => Value::Date(cells::value::parse_date(b).unwrap()),
                        "time" => Value::Time(cells::value::parse_time(b).unwrap()),
                        "datetime" => Value::DateTime(cells::value::parse_datetime(b).unwrap()),
                        "choice" => Value::Choice(&held),
                        _ => Value::Text(&held),
                    }
                }
            };
            doc.set_value(row, col, value)
        };
        taken(set, e)?;
    }
    Ok(doc.write())
}

/// An edit's result against what the suite says of it: what it gave when taken, nothing
/// when refused where the edit's `refused` says.
fn taken<T>(r: Result<T, cells::Error>, e: &Json) -> Result<Option<T>, String> {
    match (r, e.get("refused").map(Json::str)) {
        (Ok(d), None) => Ok(Some(d)),
        (Err(err), Some(at)) if err.at.to_string() == at => Ok(None),
        (Err(err), want) => Err(format!("refused at {} ({err}), the suite says {want:?}", err.at)),
        (Ok(_), Some(at)) => Err(format!("taken, the suite says it is refused at {at}")),
    }
}

/// A case's file: a `.cell`, or under `plain/` a `.csv`, `.tsv` or `.txt`; never an
/// edit's `.out.` file.
fn cases(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            cases(&p, out);
            continue;
        }
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        let plain = p.parent().is_some_and(|d| d.ends_with("plain"));
        let case = if plain { matches!(ext, "csv" | "tsv" | "txt") } else { ext == "cell" };
        let stem = p.file_stem().unwrap().to_string_lossy();
        if case && !stem.ends_with(".out") {
            out.push(p);
        }
    }
}

/// A plain case's header problems, as the suite writes them.
fn check_names(doc: &Doc, want: &Json) -> Vec<String> {
    let ours: Vec<(u32, &str)> = doc
        .naming()
        .iter()
        .map(|n| match n {
            Naming::Blank { column } => (*column, "blank"),
            Naming::Twice { column, .. } => (*column, "twice"),
        })
        .collect();
    let theirs: Vec<(u32, &str)> = want
        .get("names")
        .map(|n| n.array())
        .unwrap_or_default()
        .iter()
        .map(|n| {
            let Json::Number(c) = n.get("column").unwrap() else { panic!("a column") };
            (c.parse().unwrap(), n.get("kind").unwrap().str())
        })
        .collect();
    if ours != theirs { Vec::from([format!("names {ours:?}, the suite says {theirs:?}")]) } else { Vec::new() }
}

#[test]
fn conformance() {
    let root =
        std::env::var("Cells_SUITE").map(PathBuf::from).unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests"));
    let mut all = Vec::new();
    cases(&root, &mut all);
    // Every `.cell` file by its path in the suite, for a case's imports to follow.
    let files: std::collections::BTreeMap<String, String> = all
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "cell"))
        .filter_map(|p| Some((p.strip_prefix(&root).unwrap().to_string_lossy().into_owned(), String::from_utf8(fs::read(p).ok()?).ok()?)))
        .collect();
    let mut failed = 0;
    let mut passed = 0;
    for path in &all {
        let bytes = fs::read(path).unwrap();
        let stem = path.with_extension("");
        let ext = path.extension().unwrap().to_string_lossy();
        let edit = PathBuf::from(format!("{}.edit.json", stem.display()));
        let mut fails = Vec::new();
        let plain = ext != "cell";
        let dialect = if plain { Dialect::Plain(cells::detect(&bytes)) } else { Dialect::Cells };
        let at = path.strip_prefix(&root).unwrap().to_string_lossy().into_owned();
        let read = match plain {
            true => cells::read_as(bytes.clone(), dialect, 1),
            false => cells::read_with(bytes.clone(), &mut cells::files::Files::new(&files, &at), 1),
        };
        if edit.exists() {
            let out = fs::read(format!("{}.out.{ext}", stem.display())).unwrap();
            match read {
                Err(e) => fails.push(format!("does not read: {e}")),
                Ok(mut doc) => {
                    if doc.write().as_bytes() != bytes {
                        fails.push("does not round-trip".into());
                    }
                    match apply(&mut doc, &json(&fs::read_to_string(&edit).unwrap())) {
                        Err(e) => fails.push(format!("an edit fails: {e}")),
                        Ok(got) => {
                            if got.as_bytes() != out {
                                fails.push(format!("writes {got:?}, the suite says {:?}", String::from_utf8_lossy(&out)));
                            }
                        }
                    }
                }
            }
        } else {
            let want = json(&fs::read_to_string(path.with_extension("json")).unwrap());
            match (read, want.get("error")) {
                (Err(e), Some(err)) => {
                    let at = err.get("at").unwrap().str();
                    if e.at.to_string() != at {
                        fails.push(format!("refused at {}, the suite says {at} ({e})", e.at));
                    }
                }
                (Err(e), None) => fails.push(format!("does not read: {e}")),
                (Ok(_), Some(err)) => fails.push(format!("reads, the suite says it breaks at {}", err.get("at").unwrap().str())),
                (Ok(doc), None) => {
                    fails.extend(check_read(&doc, &want));
                    if plain {
                        let sep = (dialect.separator().byte() as char).to_string();
                        let theirs = want.get("separator").map(Json::str).unwrap_or("none");
                        if sep != theirs {
                            fails.push(format!("separator {sep:?}, the suite says {theirs:?}"));
                        }
                        fails.extend(check_names(&doc, &want));
                    }
                    if doc.write().as_bytes() != bytes {
                        fails.push("does not round-trip".into());
                    }
                }
            }
        }
        if fails.is_empty() {
            passed += 1;
        } else {
            failed += 1;
            println!("FAIL {}", path.strip_prefix(&root).unwrap().display());
            for f in fails {
                println!("    {f}");
            }
        }
    }
    println!("{passed} passed, {failed} failed, of {}", all.len());
    assert!(!all.is_empty(), "no cases under {}", root.display());
    assert_eq!(failed, 0, "{failed} cases fail");
}
