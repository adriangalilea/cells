//! An intent: a client's edit of one file, addressed by identity (ENGINE.md, Writing), as
//! its ops apply to the file's text. A cell is set in a row named by its key or its id,
//! a row is added, a row is deleted; never a position, which a line inserted elsewhere
//! moves. Nothing here reaches a disk, a clock or a random source: the engine hands in the
//! text the intent applies to, the bits new ids are made from, and a reader that follows
//! the file's imports, so an intent applied again to the same text gives the same bytes.
//!
//! The rows' identity is the engine's (SPEC.md, Every row has an identity). A file with a
//! key is identified by it; any other file gets an `id` column the first time an intent
//! writes it, an id in every row without one, and a new one on the later copy of a
//! repeated one. No op writes an id.
//!
//! An intent made against the version it applies to applies as made. Made against an
//! earlier one (someone wrote the file since), it applies only where it says what it saw:
//! a cell set names what it held (`was`), a row deleted its line as it was; one that
//! finds anything else, or says nothing, is refused, the conflict said and never guessed.
//! A row added touches nothing that was there.

use alloc::borrow::ToOwned;
use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::column::Severity;
use crate::identity::{Change, RowAt};
use crate::types::Base;
use crate::{Doc, value};

/// One op of an intent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// The cell of `column` in the row `row` given `text`; `was`, the text it held when the
    /// op was made.
    Set { row: RowAt, column: String, was: Option<String>, text: String },
    /// A row added after the last, each named column given its text, the rest blank; its
    /// id, in a file identified by one, the engine's.
    Add { cells: Vec<(String, String)> },
    /// The row `row` removed; `was`, its line as it was when the op was made.
    Delete { row: RowAt, was: Option<String> },
}

/// What an intent left: the file's text, and the ids it gave rows that had none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    pub text: String,
    pub assigned: Vec<String>,
}

/// Why an intent was refused, whole: nothing of it is applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The file does not read.
    Unread(String),
    /// Op `op` (from 0) cannot apply: the reason, in words.
    Op { op: usize, why: String },
    /// Applied, the file would break a rule in a row the intent touched: the problem.
    Breaks(String),
}

impl core::fmt::Display for Refused {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Refused::Unread(why) => write!(f, "the file does not read: {why}"),
            Refused::Op { op, why } => write!(f, "op {}: {why}", op + 1),
            Refused::Breaks(why) => write!(f, "it would break a rule: {why}"),
        }
    }
}

/// `ops` applied, in order, to `text`. `stale` says the text is not the version the intent
/// was made against. `bits` gives 40 random bits per id the engine assigns; `read` reads a
/// version of the file as its host does, imports followed.
pub fn apply(
    text: &str,
    ops: &[Op],
    stale: bool,
    mut bits: impl FnMut() -> u64,
    read: &mut dyn FnMut(&str) -> Result<Doc, String>,
) -> Result<Applied, Refused> {
    let first = read(text).map_err(Refused::Unread)?;
    let ids_before = ids(&first);
    // the rows given their identity first, so every op finds its row by it
    let mut text = given_ids(first.clone(), &mut bits, read)?;
    let mut touched: Vec<RowAt> = Vec::new();
    for (k, op) in ops.iter().enumerate() {
        let refuse = |why: String| Refused::Op { op: k, why };
        let mut doc = read(&text).map_err(Refused::Unread)?;
        match op {
            Op::Set { row, column, was, text: to } => {
                if stale && was.is_none() {
                    return Err(refuse("the file changed since this intent was made, and it does not say what the cell held".into()));
                }
                let change = Change::Cell { row: row.clone(), column: column.clone(), was: was.clone(), text: to.clone() };
                doc.apply(&change).map_err(|why| refuse(said(&why, column)))?;
                touched.push(row.clone());
                text = doc.write();
            }
            Op::Add { cells } => {
                let (added, at) = add(&doc, &text, cells, &mut bits).map_err(refuse)?;
                text = added;
                // its computed cells stored, as a write stores what it changes
                let mut doc = read(&text).map_err(Refused::Unread)?;
                let r = doc.rows() - 1;
                for c in 0..doc.columns().len() {
                    if doc.columns()[c].formula.is_some() && doc.text(r, c).is_empty() {
                        doc.set_cell(r, c, "").map_err(|e| refuse(e.why))?;
                    }
                }
                text = doc.write();
                touched.push(at);
            }
            Op::Delete { row, was } => {
                if stale && was.is_none() {
                    return Err(refuse("the file changed since this intent was made, and it does not say what the row held".into()));
                }
                let r = doc.find(row).ok_or_else(|| refuse("no row is that one: it was deleted, or its key changed".into()))?;
                if let Some(was) = was
                    && doc.line(r) != was
                {
                    return Err(refuse(format!("the row changed since this intent was made: it reads {:?} now", doc.line(r))));
                }
                let written = doc.write();
                let doc = read(&written).map_err(Refused::Unread)?;
                let (from, to) = (doc.records()[r] as usize, doc.records()[r + 1] as usize);
                text = format!("{}{}", &written[..from], &written[to..]);
            }
        }
    }
    let last = read(&text).map_err(Refused::Unread)?;
    breaks(&first, &last, &touched)?;
    let assigned = ids(&last).into_iter().filter(|i| !ids_before.contains(i)).collect();
    Ok(Applied { text, assigned })
}

/// A refusal of the core's in words, the column named.
fn said(why: &crate::Refused, column: &str) -> String {
    match why {
        crate::Refused::NoRow => "no row is that one: it was deleted, or its key changed".into(),
        crate::Refused::NoColumn(c) => format!("no column is named {c}"),
        crate::Refused::Id => "the id column is the engine's: no edit writes it".into(),
        crate::Refused::Changed { now } => format!("{column} changed since this intent was made: it holds {now:?} now"),
        crate::Refused::Type(e) | crate::Refused::Control(e) => e.why.clone(),
    }
}

/// Every id the file's `id` column holds.
fn ids(doc: &Doc) -> BTreeSet<String> {
    match doc.identity() {
        Some(c) if doc.columns()[c].ty.base == Base::Id => (0..doc.rows()).map(|r| doc.text(r, c).into_owned()).collect(),
        _ => BTreeSet::new(),
    }
}

/// The text with every row given its identity (`Doc::assign_ids`), read again as the host
/// reads it.
fn given_ids(doc: Doc, bits: &mut dyn FnMut() -> u64, read: &mut dyn FnMut(&str) -> Result<Doc, String>) -> Result<String, Refused> {
    let doc = doc.assign_ids(bits).map_err(|e| Refused::Unread(e.why))?;
    let text = doc.write();
    read(&text).map_err(Refused::Unread)?;
    Ok(text)
}

/// `text` with a row added after its last: each column given its text from `cells`, an
/// `id` column a new id, the rest blank. The row's identity too.
fn add(doc: &Doc, text: &str, cells: &[(String, String)], bits: &mut dyn FnMut() -> u64) -> Result<(String, RowAt), String> {
    let columns = doc.columns();
    for (name, _) in cells {
        let c = columns.iter().position(|c| c.name == *name).ok_or_else(|| format!("no column is named {name}"))?;
        if columns[c].ty.base == Base::Id {
            return Err("the id column is the engine's: no edit writes it".into());
        }
    }
    let taken = ids(doc);
    let mut fields: Vec<String> = Vec::with_capacity(columns.len());
    let mut at = None;
    for c in columns {
        let field = if c.ty.base == Base::Id {
            let id = loop {
                let id = value::id_from(bits());
                // Crockford's alphabet: ASCII
                let id = core::str::from_utf8(&id).expect("an id is ASCII").to_owned();
                if !taken.contains(&id) {
                    break id;
                }
            };
            at = Some(RowAt::Id(id.clone()));
            id
        } else {
            let given = cells.iter().find(|(n, _)| *n == c.name).map(|(_, t)| t.clone()).unwrap_or_default();
            if c.ty.key {
                at = Some(RowAt::Key(given.clone()));
            }
            crate::quote(&given).into_owned()
        };
        fields.push(field);
    }
    let at = at.ok_or("a row added names no identity: the file has no key and no id")?;
    let end = doc.records()[doc.rows()] as usize;
    let nl = doc.newline();
    let before = &text[..end];
    let lead = if before.ends_with('\n') || before.is_empty() { "" } else { nl };
    Ok((format!("{before}{lead}{}{nl}{}", fields.join(","), &text[end..]), at))
}

/// A problem that weighs as an error, new in a row the intent touched: the rule it would
/// break.
fn breaks(before: &Doc, after: &Doc, touched: &[RowAt]) -> Result<(), Refused> {
    let rows: BTreeSet<usize> = touched.iter().filter_map(|at| after.find(at)).collect();
    let was: BTreeSet<(String, u32, crate::Kind)> = before
        .problems()
        .iter()
        .filter(|p| (p.row as usize) < before.rows())
        .map(|p| (key(&before.row_at(p.row as usize)), p.column, p.kind()))
        .collect();
    for p in after.problems() {
        if p.kind().severity() != Severity::Error || !rows.contains(&(p.row as usize)) {
            continue;
        }
        if !was.contains(&(key(&after.row_at(p.row as usize)), p.column, p.kind())) {
            return Err(Refused::Breaks(after.describe(p)));
        }
    }
    Ok(())
}

fn key(at: &RowAt) -> String {
    match at {
        RowAt::Key(k) | RowAt::Id(k) | RowAt::Line(k) => k.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(t: &str) -> Result<Doc, String> {
        crate::read(t).map_err(|e| e.why)
    }

    fn counter() -> impl FnMut() -> u64 {
        let mut n = 0u64;
        move || {
            n += 1;
            n * 0x9E37_79B9
        }
    }

    const KEYED: &str = "item:text!,cost:EUR>=0,qty:int>=1,total:EUR=cost * qty\nfuel,84.20,1,84.20=\nferry,50.00,2,100.00=\n";

    #[test]
    fn a_cell_set_by_its_row_s_key_stores_what_it_reaches() {
        let ops = [Op::Set { row: RowAt::Key("ferry".into()), column: "qty".into(), was: Some("2".into()), text: "3".into() }];
        let out = apply(KEYED, &ops, false, counter(), &mut read).unwrap();
        assert_eq!(out.text, KEYED.replace("ferry,50.00,2,100.00=", "ferry,50.00,3,150.00="));
        assert!(out.assigned.is_empty(), "a keyed file needs no ids");
    }

    #[test]
    fn a_row_added_and_one_deleted() {
        let ops = [
            Op::Add { cells: alloc::vec![("item".into(), "taxi".into()), ("cost".into(), "12.00".into()), ("qty".into(), "2".into())] },
            Op::Delete { row: RowAt::Key("fuel".into()), was: Some("fuel,84.20,1,84.20=".into()) },
        ];
        let out = apply(KEYED, &ops, false, counter(), &mut read).unwrap();
        assert_eq!(out.text, "item:text!,cost:EUR>=0,qty:int>=1,total:EUR=cost * qty\nferry,50.00,2,100.00=\ntaxi,12.00,2,24.00=\n");
    }

    #[test]
    fn a_file_without_a_key_gets_ids_once_and_no_op_writes_one() {
        let plain = "item:text,cost:EUR\nfuel,84.20\nfuel,84.20\n";
        let ops = [Op::Add { cells: alloc::vec![("item".into(), "taxi".into())] }];
        let out = apply(plain, &ops, false, counter(), &mut read).unwrap();
        let doc = read(&out.text).unwrap();
        assert_eq!(doc.columns()[0].name, "id");
        assert_eq!(out.assigned.len(), 3, "every row, the two copies apart");
        assert_ne!(doc.text(0, 0), doc.text(1, 0));
        // applied again to what it wrote, the ids stay
        let again = apply(&out.text, &[], false, counter(), &mut read).unwrap();
        assert_eq!((again.text, again.assigned), (out.text.clone(), Vec::new()));
        let id = doc.text(2, 0).into_owned();
        let write = [Op::Set { row: RowAt::Id(id), column: "id".into(), was: None, text: "k7m2q9xa".into() }];
        assert!(matches!(apply(&out.text, &write, false, counter(), &mut read), Err(Refused::Op { op: 0, .. })));
        let add = [Op::Add { cells: alloc::vec![("id".into(), "k7m2q9xa".into())] }];
        assert_eq!(
            apply(&out.text, &add, false, counter(), &mut read),
            Err(Refused::Op { op: 0, why: "the id column is the engine's: no edit writes it".into() })
        );
    }

    #[test]
    fn a_stale_intent_applies_where_it_says_what_it_saw() {
        // someone changed fuel's cost since; the intent sets ferry's qty, untouched
        let now = KEYED.replace("fuel,84.20,1,84.20=", "fuel,90.00,1,90.00=");
        let ops = [Op::Set { row: RowAt::Key("ferry".into()), column: "qty".into(), was: Some("2".into()), text: "3".into() }];
        assert!(apply(&now, &ops, true, counter(), &mut read).unwrap().text.contains("ferry,50.00,3,150.00="));
        // the cell it set was changed since: refused, both versions said
        let touched = [Op::Set { row: RowAt::Key("fuel".into()), column: "cost".into(), was: Some("84.20".into()), text: "85.00".into() }];
        let why = apply(&now, &touched, true, counter(), &mut read).unwrap_err().to_string();
        assert_eq!(why, "op 1: cost changed since this intent was made: it holds \"90.00\" now");
        // saying nothing of what it saw, it cannot be applied to another version
        let blind = [Op::Set { row: RowAt::Key("ferry".into()), column: "qty".into(), was: None, text: "3".into() }];
        assert!(apply(&now, &blind, true, counter(), &mut read).is_err());
        assert!(apply(&now, &blind, false, counter(), &mut read).is_ok(), "made against this very version, it applies");
    }

    #[test]
    fn an_intent_breaking_a_rule_in_its_rows_is_refused_whole() {
        let dup =
            [Op::Add { cells: alloc::vec![("item".into(), "fuel".into()), ("cost".into(), "1.00".into()), ("qty".into(), "1".into())] }];
        assert!(matches!(apply(KEYED, &dup, false, counter(), &mut read), Err(Refused::Breaks(_))), "a key twice");
        let below = [Op::Set { row: RowAt::Key("fuel".into()), column: "cost".into(), was: None, text: "-1.00".into() }];
        assert!(matches!(apply(KEYED, &below, false, counter(), &mut read), Err(Refused::Breaks(_))), "a bound");
        let word = [Op::Set { row: RowAt::Key("fuel".into()), column: "qty".into(), was: None, text: "two".into() }];
        assert!(matches!(apply(KEYED, &word, false, counter(), &mut read), Err(Refused::Breaks(_))), "a type");
    }
}
