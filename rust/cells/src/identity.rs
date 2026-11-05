//! Rows by identity, and changes addressed by it: the primitives under the engine's
//! intents (ENGINE.md, Writing). A row is its key's value, else its id, else its record's
//! exact text; a change names the row that way, never by position, and may say what the
//! cell held when it was made, so it applies only to the cell it was made against.
//! Nothing here reads a clock or a random source: ids come from bits the caller supplies.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::doc::{Doc, Error, decode_one};
use crate::types::Base;
use crate::value;

/// A row, by what it is rather than where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowAt {
    /// The key column's value, as text (`007` finds the row whose `int` key is `7`).
    Key(String),
    /// The `id` column's value.
    Id(String),
    /// The record's text exactly, for a row that has neither yet.
    Line(String),
}

/// A change to one file, addressed by identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// A cell's text. `was`, when given, is the text the change was made against: a cell
    /// that holds anything else refuses it.
    Cell { row: RowAt, column: String, was: Option<String>, text: String },
    /// A column's type, written in shorthand.
    Type { column: String, spelled: String },
}

/// Why a change was not applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// No row is that one (deleted, or its key changed).
    NoRow,
    /// No stored column has that name.
    NoColumn(String),
    /// The `id` column is the engine's: no change writes it.
    Id,
    /// The cell holds something else than the change was made against.
    Changed { now: String },
    /// The type does not read.
    Type(Error),
    /// The text holds a control character, which no cell holds.
    Control(Error),
}

/// What a file's text is now: its length and a 64-bit hash of its bytes. Equal versions,
/// equal bytes (but for a collision a caller with stakes rules out with its own hash).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Version {
    pub len: u64,
    pub hash: u64,
}

impl Doc {
    /// The column that is the rows' identity: the first key, else an `id` column.
    pub fn identity(&self) -> Option<usize> {
        let columns = self.columns();
        columns.iter().position(|c| c.ty.key).or_else(|| columns.iter().position(|c| c.ty.base == Base::Id))
    }

    /// A row's identity as a change names it.
    pub fn row_at(&self, row: usize) -> RowAt {
        match self.identity() {
            Some(c) if !self.text(row, c).is_empty() => {
                let t = self.text(row, c).into_owned();
                if self.columns()[c].ty.key { RowAt::Key(t) } else { RowAt::Id(t) }
            }
            _ => RowAt::Line(self.line(row).into()),
        }
    }

    /// The first row that is `at`.
    pub fn find(&self, at: &RowAt) -> Option<usize> {
        match at {
            RowAt::Line(l) => (0..self.rows()).find(|&r| self.line(r) == l),
            RowAt::Key(t) | RowAt::Id(t) => {
                let c = self.identity()?;
                let col = &self.columns()[c];
                let one = decode_one(col, t)?;
                let want = one.value(0);
                (0..self.rows()).find(|&r| col.value(r) == want)
            }
        }
    }

    /// The text's version, as `write` would give it.
    pub fn version(&self) -> Version {
        let text = self.write();
        Version { len: text.len() as u64, hash: crate::keys::hash(text.as_bytes()) }
    }

    /// Applies one change, the same way every time for the same file and change.
    pub fn apply(&mut self, change: &Change) -> Result<(), Refused> {
        match change {
            Change::Cell { row, column, was, text } => {
                let c = self.stored_column(column)?;
                if self.columns()[c].ty.base == Base::Id {
                    return Err(Refused::Id);
                }
                let r = self.find(row).ok_or(Refused::NoRow)?;
                let now = self.text(r, c);
                if was.as_ref().is_some_and(|w| *w != now) {
                    return Err(Refused::Changed { now: now.into_owned() });
                }
                self.set_cell(r, c, text).map_err(Refused::Control)
            }
            Change::Type { column, spelled } => {
                let c = self.stored_column(column)?;
                if self.columns()[c].ty.base == Base::Id {
                    return Err(Refused::Id);
                }
                self.set_type(c, spelled).map_err(Refused::Type)
            }
        }
    }

    fn stored_column(&self, name: &str) -> Result<usize, Refused> {
        self.columns().iter().position(|c| c.name == name).ok_or_else(|| Refused::NoColumn(name.into()))
    }

    /// Every row given an identity. A file with a key has one already; a file with an `id`
    /// column gets an id in every row without one and in every later copy of a repeated
    /// one; a file with neither gets an `id:id!` column first. `bits` supplies 40 random
    /// bits per id asked for (a seeded source gives the same ids every run); an id drawn
    /// twice is drawn again.
    pub fn assign_ids(self, mut bits: impl FnMut() -> u64) -> Result<Doc, Error> {
        let rows = self.rows();
        let fresh = |seen: &mut BTreeSet<[u8; 8]>, bits: &mut dyn FnMut() -> u64| loop {
            let id = value::id_from(bits());
            if seen.insert(id) {
                // Crockford's alphabet: ASCII.
                return String::from(core::str::from_utf8(&id).unwrap());
            }
        };
        match self.identity() {
            Some(c) if self.columns()[c].ty.key && self.columns()[c].ty.base != Base::Id => Ok(self),
            Some(c) => {
                let mut doc = self;
                let mut seen: BTreeSet<[u8; 8]> = BTreeSet::new();
                let mut needs = Vec::new();
                for r in 0..rows {
                    let t = doc.text(r, c);
                    let ok = value::is_id(t.as_bytes()) && seen.insert(t.as_bytes().try_into().unwrap());
                    if !ok {
                        needs.push(r);
                    }
                }
                for r in needs {
                    let id = fresh(&mut seen, &mut bits);
                    // Crockford's alphabet holds no control character.
                    doc.set_cell(r, c, &id).unwrap();
                }
                Ok(doc)
            }
            None => {
                if self.columns().is_empty() {
                    return Ok(self);
                }
                // The text as it stands, read again so every record's span is its own.
                let written = crate::read(self.write())?;
                let (src, at) = (written.source(), written.records());
                let body = written.body();
                let mut seen = BTreeSet::new();
                let mut out = String::with_capacity(src.len() + rows * 9 + 8);
                out.push_str(&src[..body]);
                out.push_str("id:id!,");
                out.push_str(&src[body..at[0] as usize]);
                for r in 0..rows {
                    out.push_str(&fresh(&mut seen, &mut bits));
                    out.push(',');
                    out.push_str(&src[at[r] as usize..at[r + 1] as usize]);
                }
                crate::read(out).map_err(|e| Error { at: e.at, why: format!("with ids added: {}", e.why), within: e.within })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_by_identity() {
        let mut doc = crate::read("shelf:int!,title:text\n7,Moss\n12,Salt\n").unwrap();
        assert_eq!(doc.find(&RowAt::Key("007".into())), Some(0));
        assert_eq!(doc.row_at(1), RowAt::Key("12".into()));
        let change =
            Change::Cell { row: RowAt::Key("12".into()), column: "title".into(), was: Some("Salt".into()), text: "Salt Road".into() };
        doc.apply(&change).unwrap();
        assert_eq!(doc.write(), "shelf:int!,title:text\n7,Moss\n12,Salt Road\n");
        assert_eq!(doc.apply(&change), Err(Refused::Changed { now: "Salt Road".into() }));
        assert_eq!(
            doc.apply(&Change::Cell { row: RowAt::Key("99".into()), column: "title".into(), was: None, text: "x".into() }),
            Err(Refused::NoRow)
        );
    }

    #[test]
    fn ids_from_given_bits() {
        let mut n = 0u64;
        let next = || {
            n += 1;
            n * 0x1F_0F0F
        };
        let doc = crate::read("title:text\r\nMoss\r\nSalt\r\n").unwrap().assign_ids(next).unwrap();
        assert_eq!(doc.columns()[0].name, "id");
        assert!(doc.columns()[0].ty.key);
        assert_eq!(doc.problems(), &[]);
        let text = doc.write();
        assert!(text.starts_with("id:id!,title:text\r\n"), "{text:?}");
        assert_eq!(text.lines().count(), 3);
        let id = doc.text(1, 0).into_owned();
        assert_eq!(doc.find(&RowAt::Key(id)), Some(1));

        // A repeated id is replaced on the later copy; a blank one is filled.
        let mut m = 0u64;
        let mut doc = crate::read("id:id,title:text\nk7m2q9xa,Moss\nk7m2q9xa,Moss\n,Salt\n")
            .unwrap()
            .assign_ids(|| {
                m += 1;
                m
            })
            .unwrap();
        assert_eq!(doc.text(0, 0), "k7m2q9xa");
        assert_ne!(doc.text(1, 0), "k7m2q9xa");
        assert!(value::is_id(doc.text(2, 0).as_bytes()));
        assert_eq!(
            doc.apply(&Change::Cell { row: RowAt::Id("k7m2q9xa".into()), column: "id".into(), was: None, text: "x".into() }),
            Err(Refused::Id)
        );
    }
}
