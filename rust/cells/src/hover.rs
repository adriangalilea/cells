//! What a place in a file's text is, said: a column (its header cell, what computes it,
//! what it names), a cell's value and what is wrong with it, a formula's name, a function,
//! a table, a named type. Every client says the same: the language server as Markdown,
//! the apps as plain lines.
//!
//! ```
//! let text = "item:text!,cost:EUR>=0,total:EUR=ROUND(cost)\nfuel,84.20,84.00=\n";
//! let doc = cells::read(text).unwrap();
//! let at = text.find("ROUND").unwrap() + 1;
//! let said = cells::hover::hover(&doc, text, at, |_: &[&str]| None::<std::sync::Arc<cells::Doc>>).unwrap();
//! assert!(said.text().starts_with("ROUND(x, digits)"));
//! ```

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Deref;

use crate::complete::{Followed, Reach, Spot, follow, ident, shown, spot, summary};
use crate::expr;
use crate::look::{self, Rule, Tint};
use crate::{Base, Column, Doc, Origin};

/// A part of what is said, as each client writes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    Plain(String),
    /// Written as code: a header cell, a formula, a name.
    Code(String),
    /// What matters most: a cell's value.
    Strong(String),
    /// A paragraph's end.
    Break,
}

/// What is said of a place: its pieces, and the bytes `from..to` of the text it is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hover {
    pub pieces: Vec<Piece>,
    pub from: u32,
    pub to: u32,
}

impl Hover {
    /// In plain lines, a paragraph a line: what an app shows.
    pub fn text(&self) -> String {
        self.pieces
            .iter()
            .map(|p| match p {
                Piece::Plain(s) | Piece::Code(s) | Piece::Strong(s) => s.as_str(),
                Piece::Break => "\n",
            })
            .collect()
    }

    /// In Markdown: what the language server answers.
    pub fn markdown(&self) -> String {
        self.pieces
            .iter()
            .map(|p| match p {
                Piece::Plain(s) => s.clone(),
                Piece::Code(s) => format!("`{s}`"),
                Piece::Strong(s) => format!("**{s}**"),
                Piece::Break => "\n\n".to_owned(),
            })
            .collect()
    }
}

fn plain(s: impl Into<String>) -> Piece {
    Piece::Plain(s.into())
}

/// A column in words: its header cell, what computes it, what it holds, what it names.
pub fn about(c: &Column) -> Vec<Piece> {
    // the column as a writer writes its header cell, one way wherever it is said
    let spelled = match &c.formula {
        Some(f) => format!("{}={f}", c.ty.shorthand()),
        None => c.ty.shorthand(),
    };
    let mut out = vec![Piece::Code(crate::prelude::header_cell(&c.name, &spelled))];
    let mut said: Vec<Vec<Piece>> = Vec::new();
    if c.ty.inferred {
        said.push(vec![plain("its type inferred from its values")]);
    }
    if c.ty.key {
        said.push(vec![plain("the key: in every row, once")]);
    }
    if let Base::Ref(targets) = &c.ty.base {
        let mut named = vec![plain("a key of ")];
        for (i, t) in targets.iter().enumerate() {
            if i > 0 {
                named.push(plain(" or "));
            }
            named.push(Piece::Code(format!("{}.{}", t.table, t.column)));
        }
        said.push(named);
    }
    if !said.is_empty() {
        out.push(Piece::Break);
        for (i, s) in said.into_iter().enumerate() {
            if i > 0 {
                out.push(plain("; "));
            }
            out.extend(s);
        }
    }
    out
}

/// A table in a line: its name, its rows, its columns.
fn table(name: &str, d: &Doc) -> Vec<Piece> {
    let columns: Vec<&str> = d.columns().iter().map(|c| c.name.as_str()).collect();
    vec![plain("table "), Piece::Code(name.to_owned()), plain(format!(": {} rows, {}", d.rows(), columns.join(", ")))]
}

/// A function: its call as written, and what it gives.
fn function(name: &str) -> Option<Vec<Piece>> {
    let (call, gives) = expr::signature(name)?;
    Some(vec![Piece::Code(call.to_owned()), Piece::Break, plain(gives)])
}

/// The word of `ident` characters around byte `at` of `text`, and where it starts.
fn word(text: &str, at: usize) -> (&str, usize) {
    let from = text[..at].rfind(|c: char| !ident(c)).map_or(0, |i| i + 1);
    let to = at + text[at..].find(|c: char| !ident(c)).unwrap_or(text.len() - at);
    (&text[from..to], from)
}

/// What is said of byte `at` of `text`, the text `doc` was read from: a header cell's
/// column, a cell's value (the row its reference names), a formula's name or function, a
/// named type or a table in the prelude. What is wrong there is never said here: it is a
/// problem, which every host shows as its own (an editor's diagnostic, the app's tip and
/// the prelude's list), so a hover repeating it says it twice. The tables it names are the
/// host's: `held` reaches them by name, as `complete` does.
pub fn hover<D, H>(doc: &Doc, text: &str, at: usize, held: H) -> Option<Hover>
where
    D: Deref<Target = Doc>,
    H: Fn(&[&str]) -> Option<D>,
{
    assert!(at <= text.len() && text.is_char_boundary(at), "hover at {at}, not a place in the text");
    let reach = Reach { doc, held };
    let said = |pieces: Vec<Piece>, from: u32, len: u32| Some(Hover { pieces, from, to: from + len });
    match spot(doc, text, at) {
        Spot::Type(..) if tinted(text, at).is_some() => tinted(text, at),
        Spot::Name(c, s) | Spot::Type(c, s) => {
            let mut out = about(&doc.columns()[c]);
            let looks = doc.looks_of(c);
            if !looks.is_empty() {
                out.push(Piece::Break);
                out.push(plain("looks "));
                out.push(Piece::Code(looks.iter().map(|l| l.spelled.as_str()).collect::<Vec<_>>().join(", ")));
            }
            said(out, s.start, s.len)
        }
        Spot::Cell(r, c, s) => {
            let col = &doc.columns()[c];
            let mut out = about(col);
            out.push(Piece::Break);
            if col.origin(r) == Origin::Formula {
                out.push(plain("computed: "));
            }
            out.push(Piece::Strong(shown(col, r)));
            if let Base::Ref(targets) = &col.ty.base {
                let key = doc.text(r, c);
                for t in targets {
                    let Some(d) = reach.get(&[t.table.as_str()]) else { continue };
                    let Some(k) = d.column(&t.column) else { continue };
                    if let Some(row) = (0..d.rows()).find(|&row| d.text(row, k) == key) {
                        out.push(Piece::Break);
                        out.push(plain(format!("{}: {}", t.table, summary(&d, row))));
                    }
                }
            }
            said(out, s.start, s.len)
        }
        Spot::Formula(f) => {
            let here = at - f.base as usize;
            // a function's name, written before its `(`
            let (w, start) = word(f.src, here);
            if let Some(pieces) = function(w).filter(|_| f.src[start + w.len()..].trim_start().starts_with('(')) {
                return said(pieces, f.base + start as u32, w.len() as u32);
            }
            let here = here as u32;
            let names = expr::names(f.src).ok()?;
            let (n, part) =
                names.iter().find_map(|n| n.parts.iter().position(|(_, (s, l))| here >= *s && here <= s + l).map(|k| (n, k)))?;
            let (name, (s, l)) = &n.parts[part];
            let pieces = match follow(&reach, n, part)? {
                Followed::Column(held, k) => {
                    let d: &Doc = held.as_ref().map_or(doc, |(_, d)| d);
                    about(&d.columns()[k])
                }
                Followed::Table(_, d) => table(name, &d),
            };
            said(pieces, f.base + s, *l)
        }
        Spot::Prelude => prelude(doc, text, at),
        Spot::Nothing => None,
    }
}

/// A choice's tint at byte `at`, after its `#` or its `tint`: what it is.
fn tinted(text: &str, at: usize) -> Option<Hover> {
    let (w, start) = word(text, at);
    let hashed = text[..start].ends_with('#');
    let worded = text[..start].trim_end_matches([' ', '\t']).strip_suffix("tint").is_some_and(|b| b.ends_with([' ', '\t']));
    if w.is_empty() || !(hashed || worded) {
        return None;
    }
    let t = Tint::after_hash(w).ok()?;
    Some(Hover { pieces: vec![plain(t.said())], from: (start - hashed as usize) as u32, to: (start + w.len()) as u32 })
}

/// In the prelude, outside a fence: a tint, a look's rule, a named type where it is named
/// or used, a table where it is named (an import's, an inline table's, a script's).
fn prelude(doc: &Doc, text: &str, at: usize) -> Option<Hover> {
    let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let fences = text[..line_start].lines().filter(|l| l.trim_start().starts_with("```")).count();
    if fences % 2 == 1 {
        return None;
    }
    if let Some(h) = tinted(text, at) {
        return Some(h);
    }
    let line_end = text[at..].find('\n').map_or(text.len(), |i| at + i);
    let line = crate::prelude::uncomment(&text[line_start..line_end]);
    if let Some((_, _, rules)) = look::spans(line) {
        let here = at - line_start;
        if let Some(r) = rules.iter().find(|r| r.contains(&here) || r.end == here) {
            let spelled = &line[r.clone()];
            let rule = look::rule(spelled);
            // A tint the rule names, before any condition.
            let (w, start) = word(text, at);
            let hashed = text[..start].ends_with('#');
            let before_when = !matches!(&rule, Ok(Rule::When { .. })) || here < r.start + spelled.find("when").unwrap_or(0);
            let named = if hashed { Tint::after_hash(w) } else { Tint::parse(w) };
            if let (Ok(t), true) = (named, before_when && !w.is_empty()) {
                let from = (start - hashed as usize) as u32;
                return Some(Hover { pieces: vec![plain(t.said())], from, to: (start + w.len()) as u32 });
            }
            let said = rule.map_or_else(|why| why, |r| r.said());
            let from = (line_start + r.start) as u32;
            return Some(Hover {
                pieces: vec![Piece::Code(spelled.to_owned()), Piece::Break, plain(said)],
                from,
                to: from + spelled.len() as u32,
            });
        }
    }
    let (w, start) = word(text, at);
    if w.is_empty() {
        return None;
    }
    let (from, len) = (start as u32, w.len() as u32);
    if w == "look" && text[line_start..start].trim().is_empty() {
        let pieces = vec![
            Piece::Code("look".into()),
            Piece::Break,
            plain(
                "how a column's, a named type's or the row's values show, wherever they show: a table's cell and a chart's mark alike; never a value",
            ),
        ];
        return Some(Hover { pieces, from, to: from + len });
    }
    if let Some(t) = doc.named().iter().find(|t| t.name == w) {
        let pieces = vec![plain("type "), Piece::Code(format!("{} = {}", t.name, t.ty.shorthand()))];
        return Some(Hover { pieces, from, to: from + len });
    }
    let t = doc.tables().iter().find(|t| t.name == w)?;
    let pieces = match &t.doc {
        Some(d) => table(&t.name, d),
        None => vec![plain("table "), Piece::Code(t.name.clone()), plain(": not reached")],
    };
    Some(Hover { pieces, from, to: from + len })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;

    fn hovered(text: &str, at: usize) -> Option<Hover> {
        let doc = crate::read(text).unwrap();
        hover(&doc, text, at, |_: &[&str]| None::<Arc<Doc>>)
    }

    #[test]
    fn a_column_in_a_formula() {
        let text = "item: text!,cost: EUR>=0,qty: int,total: EUR = cost * qty\nfuel,84.20,1,84.20=\n";
        let at = text.find("cost *").unwrap() + 1;
        let h = hovered(text, at).unwrap();
        assert_eq!(h.text(), "cost: EUR>=0");
        assert_eq!(h.markdown(), "`cost: EUR>=0`");
        assert_eq!((h.from as usize, h.to as usize), (at - 1, at + 3));
    }

    #[test]
    fn a_function() {
        let text = "item:text!,cost:EUR,total:EUR=ROUND(cost, 1)\nfuel,84.20,84.20=\n";
        let at = text.find("ROUND").unwrap() + 2;
        let h = hovered(text, at).unwrap();
        assert!(h.text().starts_with("ROUND(x, digits)\nx rounded half to even"), "{}", h.text());
        assert_eq!(h.to - h.from, 5);
    }

    #[test]
    fn a_named_type_in_the_prelude() {
        let text = "---\ntype Money = EUR>=0\ntype Price = Money\n---\ncost:Price\n1.00\n";
        let at = text.rfind("Money").unwrap() + 1;
        let h = hovered(text, at).unwrap();
        assert_eq!(h.text(), "type Money = EUR>=0");
    }

    #[test]
    fn a_table_in_the_prelude() {
        let text = "---\nfees = ```cells\nkind:text!,pct:num\ncard,1.5\n```\n---\nitem:text\nx\n";
        let at = text.find("fees").unwrap() + 1;
        let h = hovered(text, at).unwrap();
        assert_eq!(h.text(), "table fees: 1 rows, kind, pct");
        // inside its fence, the rows are the table's own: nothing said there
        let inside = text.find("card").unwrap() + 1;
        assert!(hovered(text, inside).is_none());
    }

    #[test]
    fn tints_and_looks() {
        let text = "---\ntype Kind = income#green|cost#3fa34d\nlook amount: grouped, red when amount < 0 // why\n---\nkind:Kind,amount:EUR\nincome,1.00\n";
        let at = text.find("green").unwrap() + 1;
        let h = hovered(text, at).unwrap();
        assert!(h.text().starts_with("the tint green"), "{}", h.text());
        assert_eq!(&text[h.from as usize..h.to as usize], "#green");
        let h = hovered(text, text.find("3fa34d").unwrap()).unwrap();
        assert_eq!(h.text(), "the colour #3fa34d, the same in every theme");
        let h = hovered(text, text.find("grouped").unwrap() + 2).unwrap();
        assert_eq!(h.text(), "grouped\nits thousands grouped, in the reader's marks");
        let h = hovered(text, text.find("red when").unwrap() + 1).unwrap();
        assert!(h.text().starts_with("the tint red"), "{}", h.text());
        let h = hovered(text, text.find("< 0").unwrap()).unwrap();
        assert_eq!(h.text(), "red when amount < 0\ndrawn in red where amount < 0 holds");
        // a column says its looks
        let h = hovered(text, text.find("amount:EUR").unwrap() + 1).unwrap();
        assert!(h.text().ends_with("looks grouped, red when amount < 0"), "{}", h.text());
    }

    /// Every import the accounts table.
    struct Accounts;

    impl crate::Imports for Accounts {
        fn import(&mut self, _: &crate::Import) -> Result<Option<Doc>, String> {
            Ok(Some(crate::read("account:text!,kind:text\nbank,assets\n").unwrap()))
        }
    }

    #[test]
    fn an_import_name() {
        let text = "---\nimport accounts from \"./accounts.cells\"\n---\nto:accounts.account\nbank\n";
        let doc = crate::read_with(text, &mut Accounts, 1).unwrap();
        let at = text.find("accounts from").unwrap() + 1;
        let h = hover(&doc, text, at, |_: &[&str]| None::<Arc<Doc>>).unwrap();
        assert_eq!(h.text(), "table accounts: 1 rows, account, kind");
        // and a cell naming one of its rows says the row
        let cell = text.rfind("bank").unwrap();
        let held = |p: &[&str]| (p == ["accounts"]).then(|| doc.tables()[0].doc.clone().unwrap());
        let row = hover(&doc, text, cell, held).unwrap();
        assert!(row.text().ends_with("accounts: account bank · kind assets"), "{}", row.text());
    }
}
