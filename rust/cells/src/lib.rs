//! Cells, data and its logic in one plain-text file: the one reader and writer every
//! surface uses. SPEC.md is the law.
//!
//! ```
//! let doc = cells::read("item:text!,cost:EUR>=0\nfuel,84.20\nsnacks,8x.50\n").unwrap();
//! assert_eq!(doc.rows(), 2);
//! assert_eq!(doc.describe(&doc.problems()[0]), r#"row 2, cost: "8x.50" is not a number"#);
//! let mut doc = doc;
//! doc.set_cell(1, 1, "8.50");
//! assert_eq!(doc.write(), "item:text!,cost:EUR>=0\nfuel,84.20\nsnacks,8.50\n");
//! ```
//!
//! `read` never refuses a file for its data: a cell that breaks its type is a `Problem`,
//! its text kept and its value null. It refuses a broken grammar with an `Error` naming
//! where (`At`). Columns are stored as Arrow lays them out (`column::Data`), and `arrow`
//! exports them through the Arrow C Data Interface without a copy.

#![no_std]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod arrow;
pub mod chart;
pub mod column;
pub mod complete;
mod doc;
pub mod exact;
pub mod expr;
pub mod files;
pub mod folder;
mod format;
pub mod hover;
mod identity;
pub mod intent;
mod json;
mod keys;
pub mod layout;
pub mod look;
pub mod prelude;
#[cfg(test)]
mod properties;
mod read;
pub mod run;
mod scan;
pub mod sha256;
pub mod sheet;
pub mod tokens;
pub mod types;
pub mod value;

pub use column::{Bits, Data, Kind, Problem, Severity, Value, Why};
#[cfg(feature = "std")]
pub use doc::read_parallel;
pub use doc::{
    Alone, At, Column, ColumnAt, Doc, Error, Import, Imports, Naming, Origin, Place, Row, Script, Span, Table, compose, read, read_as,
    read_with,
};
pub use format::{Dialect, Separator, detect};
pub use identity::{Change, Refused, RowAt, Version};
pub use look::{Locale, Look, Rule, Tint, Tinted};
pub use prelude::{Fenced, Schedule, Statement, Trigger};
pub use scan::quote;

/// Where a `.cells` file's cell splits, what it stores and how it is computed (SPEC.md, A
/// cell): the byte of its first `=` with no `'` before it; nothing for a written value.
pub fn split_cell(text: &str) -> Option<usize> {
    read::split(text.as_bytes())
}
pub use types::{Base, Bound, Currency, Format, NamedType, Target, Type, parse_type, split_formula, split_header};
pub use value::Decimal;
