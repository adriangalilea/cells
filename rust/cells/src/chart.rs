//! A file's charts (SPEC.md, Charts): a fence, ```` ```chart bars ````, of entries, each a
//! key, a `:` and the rest of the line. Here what a chart is and what it is drawn from: the
//! kinds and the entries each takes, the entries read off a fence, and the summary a reader
//! draws (its places, its series and their tints, each place's value). Resolving a chart's
//! formulas and reading its rows are the doc's, which holds the columns.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::json::quote;
use crate::look::{TINTS, Tint};

/// What a chart is drawn as: the fence's label, or read from its entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Bars,
    Line,
    Area,
    Steps,
    Points,
    Heat,
    Figure,
}

impl Kind {
    pub const ALL: [Kind; 7] = [Kind::Bars, Kind::Line, Kind::Area, Kind::Steps, Kind::Points, Kind::Heat, Kind::Figure];

    /// The kind a fence's label names.
    pub fn of(label: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.word() == label)
    }

    /// As a fence's label spells it.
    pub fn word(self) -> &'static str {
        match self {
            Kind::Bars => "bars",
            Kind::Line => "line",
            Kind::Area => "area",
            Kind::Steps => "steps",
            Kind::Points => "points",
            Kind::Heat => "heat",
            Kind::Figure => "figure",
        }
    }

    /// The entries it takes, besides `title` and `from`, which every kind takes.
    pub fn takes(self) -> &'static [&'static str] {
        match self {
            Kind::Bars => &["x", "y", "color", "stack", "where", "top"],
            Kind::Line | Kind::Area | Kind::Steps | Kind::Points => &["x", "y", "color", "where"],
            Kind::Heat => &["x", "y", "value", "where"],
            Kind::Figure => &["value", "compare", "where"],
        }
    }
}

/// How a chart's series share a place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stack {
    Stacked,
    Grouped,
    /// Shares of the place.
    Percent,
}

impl Stack {
    pub fn of(word: &str) -> Option<Stack> {
        Some(match word {
            "stacked" => Stack::Stacked,
            "grouped" => Stack::Grouped,
            "percent" => Stack::Percent,
            _ => return None,
        })
    }

    pub fn word(self) -> &'static str {
        match self {
            Stack::Stacked => "stacked",
            Stack::Grouped => "grouped",
            Stack::Percent => "percent",
        }
    }
}

/// Every key a chart may hold.
pub const KEYS: [&str; 11] = ["x", "y", "color", "value", "compare", "stack", "where", "top", "title", "from", "layers"];

/// One line of a chart's fence: its key, its value as written (spaces after the `:` cut),
/// and its prelude line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub value: String,
    pub line: u32,
}

/// A fence's lines read as entries, `first` the prelude line of its first line: each a
/// key, a `:` and the rest of the line; a blank line nothing. A line with no key, a key no
/// chart holds, or one written twice is broken at its line and left out.
pub fn entries(body: &str, first: u32) -> (Vec<Entry>, Vec<(u32, String)>) {
    let mut out: Vec<Entry> = Vec::new();
    let mut broken = Vec::new();
    for (i, raw) in body.lines().enumerate() {
        let line = first + i as u32;
        let said = raw.trim_matches([' ', '\t']);
        if said.is_empty() {
            continue;
        }
        let Some((key, value)) = said.split_once(':') else {
            broken.push((line, format!("a chart's line is key: value, not \"{said}\"")));
            continue;
        };
        let key = key.trim_matches([' ', '\t']);
        if !KEYS.contains(&key) {
            broken.push((line, format!("a chart has no \"{key}\": {}", KEYS.join(", "))));
            continue;
        }
        if out.iter().any(|e| e.key == key) {
            broken.push((line, format!("{key} is written twice")));
            continue;
        }
        out.push(Entry { key: key.to_owned(), value: value.trim_matches([' ', '\t']).to_owned(), line });
    }
    (out, broken)
}

/// What a reader draws: the chart's places along its axis, its series, and each place's
/// value of each series, summed and folded as the chart says. Values are exact decimals
/// as text; a place a series has no row in holds none.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub kind: Kind,
    pub title: String,
    pub stack: Stack,
    /// What runs along the axis: `date`, `number` or `text` (the places' order follows it).
    pub axis: &'static str,
    /// Each place, as its value is written (a date ISO, a number plain).
    pub places: Vec<String>,
    /// Each series: its name (empty for the one series of a chart without `color`) and its
    /// tint.
    pub series: Vec<(String, Shade)>,
    /// `values[place][series]`.
    pub values: Vec<Vec<Option<String>>>,
    /// The measure's currency, when it is money.
    pub unit: Option<String>,
    /// Rows `where` held for that had no place or no value, and the rows read.
    pub skipped: usize,
    pub rows: usize,
    /// A chart of `layers`: each layer's own summary, drawn on one plot sharing its axes;
    /// its own places and series then empty.
    pub layers: Vec<Summary>,
}

/// What a series is drawn in: a tint, or a column's own colour (its index, the reader's
/// palette in turn), or the quiet gray of `other` and the blank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shade {
    Tint(Tint),
    Column(usize),
    Quiet,
}

/// The tints a series takes when its word has none, in turn, past those its siblings' words
/// wear (SPEC.md, What a chart is drawn in).
pub const UNWORN: [&str; 10] = ["blue", "orange", "teal", "purple", "pink", "yellow", "brown", "green", "red", "gray"];

/// Each word its tint: its own where it has one, else the next of `UNWORN` none of the
/// words wears, the words in their lasting order.
pub fn shades(words: &[String], own: impl Fn(&str) -> Option<Tint>) -> Vec<Shade> {
    let worn: Vec<Option<Tint>> = words.iter().map(|w| own(w)).collect();
    let free: Vec<Tint> = UNWORN
        .iter()
        .map(|n| Tint::Named(TINTS.iter().position(|t| t == n).expect("an unworn tint is one of the ten") as u8))
        .filter(|t| !worn.contains(&Some(*t)))
        .collect();
    let mut free = free.into_iter();
    worn.into_iter().map(|t| Shade::Tint(t.or_else(|| free.next()).unwrap_or(Tint::Named(9)))).collect()
}

impl Summary {
    /// As JSON: what a host decodes (the app's Swift, a page's script).
    pub fn json(&self) -> String {
        let shade = |s: &Shade| match s {
            Shade::Tint(Tint::Named(n)) => format!("{{\"tint\":{}}}", quote(TINTS[*n as usize])),
            Shade::Tint(Tint::Hex(rgb)) => format!("{{\"hex\":{}}}", quote(&format!("#{rgb:06x}"))),
            Shade::Column(k) => format!("{{\"column\":{k}}}"),
            Shade::Quiet => "{\"quiet\":true}".to_owned(),
        };
        let strings = |v: &[String]| v.iter().map(|s| quote(s)).collect::<Vec<_>>().join(",");
        let series = self.series.iter().map(|(n, s)| format!("{{\"name\":{},\"shade\":{}}}", quote(n), shade(s))).collect::<Vec<_>>();
        let values = self
            .values
            .iter()
            .map(|row| {
                let cells = row.iter().map(|v| v.as_ref().map_or("null".to_owned(), |s| quote(s))).collect::<Vec<_>>();
                format!("[{}]", cells.join(","))
            })
            .collect::<Vec<_>>();
        let layers = self.layers.iter().map(Summary::json).collect::<Vec<_>>();
        format!(
            "{{\"kind\":{},\"title\":{},\"stack\":{},\"axis\":{},\"places\":[{}],\"series\":[{}],\"values\":[{}],\"unit\":{},\"skipped\":{},\"rows\":{},\"layers\":[{}]}}",
            quote(self.kind.word()),
            quote(&self.title),
            quote(self.stack.word()),
            quote(self.axis),
            strings(&self.places),
            series.join(","),
            values.join(","),
            self.unit.as_ref().map_or("null".to_owned(), |u| quote(u)),
            self.skipped,
            self.rows,
            layers.join(","),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_by_line() {
        let (e, broken) = entries("x: DATE_TRUNC('month', date)\n\ny:   SUM(amount)\nnope: 1\nx: date\nwhat\n", 4);
        assert_eq!(e.len(), 2);
        assert_eq!((e[0].key.as_str(), e[0].value.as_str(), e[0].line), ("x", "DATE_TRUNC('month', date)", 4));
        assert_eq!((e[1].key.as_str(), e[1].value.as_str(), e[1].line), ("y", "SUM(amount)", 6));
        assert_eq!(broken.iter().map(|(l, _)| *l).collect::<Vec<_>>(), [7, 8, 9]);
    }

    #[test]
    fn unworn_tints_skip_the_worn() {
        let words: Vec<String> = ["income", "cost", "transfer", "other"].iter().map(|w| (*w).to_owned()).collect();
        // income wears blue: the next free is orange
        let s = shades(&words, |w| (w == "income").then_some(Tint::Named(5)));
        assert_eq!(s, [Shade::Tint(Tint::Named(5)), Shade::Tint(Tint::Named(1)), Shade::Tint(Tint::Named(4)), Shade::Tint(Tint::Named(6))]);
    }
}
