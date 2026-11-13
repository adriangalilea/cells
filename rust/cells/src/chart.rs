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

/// Each tint's colour on a light page and a dark one, the ten in `TINTS`' order: what an
/// SVG draws a named tint in (a reader with a theme of its own draws its own).
/// Apple's system colours, light and dark: a tint looks as its name says on a platform
/// that names it.
const SHADES: [(u32, u32); 10] = [
    (0xff3b30, 0xff453a),
    (0xff9500, 0xff9f0a),
    (0xffcc00, 0xffd60a),
    (0x34c759, 0x30d158),
    (0x30b0c7, 0x40c8e0),
    (0x007aff, 0x0a84ff),
    (0xaf52de, 0xbf5af2),
    (0xff2d55, 0xff375f),
    (0xa2845e, 0xac8e68),
    (0x8e8e93, 0x98989d),
];

/// The columns' colours in turn, as a reader colours a column with no tint of its own.
const COLUMNS: [u32; 6] = [0x14a3b8, 0x6250d6, 0xeb6834, 0x1baf7a, 0xe87ba4, 0xeda100];

impl Shade {
    /// As `#rrggbb` on a light page (`dark`: on a dark one).
    pub fn hex(self, dark: bool) -> String {
        let rgb = match self {
            Shade::Tint(Tint::Named(n)) => {
                let (l, d) = SHADES[n as usize];
                if dark { d } else { l }
            }
            Shade::Tint(Tint::Hex(rgb)) => rgb,
            Shade::Column(k) => COLUMNS[k % COLUMNS.len()],
            Shade::Quiet => 0x9a9a9a,
        };
        format!("#{rgb:06x}")
    }
}

impl Summary {
    /// The chart as an SVG of `width` by `height`, for a page or a README: its title, a
    /// scale of four steps, its marks in its series' shades (the light page's), each
    /// place named under its mark. A chart of layers draws each layer on one plot.
    pub fn svg(&self, width: u32, height: u32) -> String {
        let (w, h) = (width as f64, height as f64);
        let parts: Vec<&Summary> = if self.layers.is_empty() { alloc::vec![self] } else { self.layers.iter().collect() };
        // every series by its name in its shade, once there is more than one
        let legend: Vec<(&str, String)> = parts
            .iter()
            .flat_map(|p| p.series.iter().map(|(n, s)| (if n.is_empty() { p.title.as_str() } else { n.as_str() }, s.hex(false))))
            .collect();
        let legend = if legend.len() > 1 { legend } else { Vec::new() };
        // the title's and a figure's inset; a plot's left is as wide as its scale's labels
        let (inset, right, bottom) = (16.0, 16.0, 32.0);
        let top = if legend.is_empty() { 40.0 } else { 56.0 };
        let left = inset;
        let ph = h - top - bottom;
        let num = |v: &Option<String>| v.as_deref().and_then(|s| s.parse::<f64>().ok());
        let mut out = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height}\" width=\"{width}\" height=\"{height}\" font-family=\"-apple-system, system-ui, sans-serif\" font-size=\"11\">\n<text x=\"{left}\" y=\"18\" font-size=\"13\" font-weight=\"600\">{}</text>\n",
            escape(&self.title)
        );
        let mut at = inset;
        for (name, hex) in &legend {
            out += &format!(
                "<circle cx=\"{:.1}\" cy=\"34\" r=\"4\" fill=\"{hex}\"/><text x=\"{:.1}\" y=\"38\" fill=\"#444\">{}</text>\n",
                at + 4.0,
                at + 12.0,
                escape(name)
            );
            at += 24.0 + 6.5 * name.chars().count() as f64;
        }
        if self.kind == Kind::Figure && self.layers.is_empty() {
            let run: Vec<f64> = self.values.iter().filter_map(|r| num(&r[0])).collect();
            let now = run.last().copied().unwrap_or(0.0);
            out += &format!("<text x=\"{left}\" y=\"{}\" font-size=\"40\" font-weight=\"600\">{}</text>\n", top + 44.0, self.figure(now));
            if run.len() > 1 {
                let change = run[run.len() - 1] - run[run.len() - 2];
                let tint = if change >= 0.0 { "#1baf7a" } else { "#e34948" };
                out += &format!(
                    "<text x=\"{left}\" y=\"{}\" fill=\"{tint}\">{}{}</text>\n",
                    top + 66.0,
                    if change >= 0.0 { "+" } else { "" },
                    self.figure(change)
                );
            }
            return out + "</svg>\n";
        }
        // the scale: from zero (or the least, below zero) to the most, stacked where bars stack
        let stacked = |p: &Summary| p.kind == Kind::Bars && p.stack == Stack::Stacked;
        let mut lo: f64 = 0.0;
        let mut hi: f64 = 0.0;
        for p in &parts {
            for row in &p.values {
                let vals: Vec<f64> = row.iter().filter_map(num).collect();
                if stacked(p) {
                    hi = hi.max(vals.iter().filter(|v| **v > 0.0).sum());
                    lo = lo.min(vals.iter().filter(|v| **v < 0.0).sum());
                } else {
                    vals.iter().for_each(|v| {
                        hi = hi.max(*v);
                        lo = lo.min(*v);
                    });
                }
            }
        }
        if hi <= lo {
            hi = lo + 1.0;
        }
        // a scale a person reads: steps of 1, 2 or 5 times a power of ten, its ends whole steps
        let step = nice((hi - lo) / 4.0);
        let (lo, hi) = (whole(lo / step, false) * step, whole(hi / step, true) * step);
        let steps = ((hi - lo) / step + 0.5) as i64;
        // the labels' room: their widest, at a sans serif's average width of 11 points
        let widest = (0..=steps).map(|i| self.figure(lo + step * i as f64).chars().count()).max().unwrap_or(1);
        let left = inset + 6.5 * widest as f64 + 6.0;
        let pw = w - left - right;
        let y = |v: f64| top + ph * (hi - v) / (hi - lo);
        for i in 0..=steps {
            let v = lo + step * i as f64;
            out += &format!(
                "<line x1=\"{left}\" x2=\"{}\" y1=\"{y0:.1}\" y2=\"{y0:.1}\" stroke=\"#000\" stroke-opacity=\"0.08\"/>\n<text x=\"{}\" y=\"{:.1}\" text-anchor=\"end\" fill=\"#666\">{}</text>\n",
                left + pw,
                left - 6.0,
                y(v) + 4.0,
                self.figure(v),
                y0 = y(v)
            );
        }
        let places = parts.iter().map(|p| p.places.len()).max().unwrap_or(0).max(1);
        let band = pw / places as f64;
        let x = |p: usize| left + band * (p as f64 + 0.5);
        for (i, name) in parts[0].places.iter().enumerate() {
            if places <= 12 || i % places.div_ceil(12) == 0 {
                out +=
                    &format!("<text x=\"{:.1}\" y=\"{}\" text-anchor=\"middle\" fill=\"#666\">{}</text>\n", x(i), h - 12.0, escape(name));
            }
        }
        for part in &parts {
            let shades: Vec<String> = part.series.iter().map(|(_, s)| s.hex(false)).collect();
            match part.kind {
                Kind::Bars => {
                    let n = part.series.len().max(1) as f64;
                    for (p, row) in part.values.iter().enumerate() {
                        let mut up = 0.0;
                        let mut down = 0.0;
                        for (s, v) in row.iter().enumerate() {
                            let Some(v) = num(v) else { continue };
                            let (from, to, bx, bw) = if stacked(part) {
                                let base = if v >= 0.0 { up } else { down };
                                if v >= 0.0 {
                                    up += v
                                } else {
                                    down += v
                                }
                                (base, base + v, x(p) - band * 0.35, band * 0.7)
                            } else {
                                let bw = band * 0.7 / n;
                                (0.0, v, x(p) - band * 0.35 + bw * s as f64, bw)
                            };
                            out += &format!(
                                "<rect x=\"{bx:.1}\" y=\"{:.1}\" width=\"{bw:.1}\" height=\"{:.1}\" fill=\"{}\" rx=\"2\"/>\n",
                                y(from.max(to)),
                                (y(from) - y(to)).abs(),
                                shades[s]
                            );
                        }
                    }
                }
                Kind::Heat => {
                    let cells = part.series.len().max(1) as f64;
                    let ch = ph / cells;
                    let all: Vec<f64> = part.values.iter().flatten().filter_map(num).collect();
                    let (vl, vh) = all.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
                    for (p, row) in part.values.iter().enumerate() {
                        for (s, v) in row.iter().enumerate() {
                            let Some(v) = num(v) else { continue };
                            let depth = if vh > vl { 0.15 + 0.85 * (v - vl) / (vh - vl) } else { 1.0 };
                            out += &format!(
                                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{band:.1}\" height=\"{ch:.1}\" fill=\"{}\" fill-opacity=\"{depth:.2}\"/>\n",
                                left + band * p as f64,
                                top + ch * s as f64,
                                shades.first().map_or("#2a78d6", |s| s.as_str())
                            );
                        }
                    }
                }
                _ => {
                    for (s, shade) in shades.iter().enumerate() {
                        let pts: Vec<(f64, f64)> =
                            part.values.iter().enumerate().filter_map(|(p, row)| num(&row[s]).map(|v| (x(p), y(v)))).collect();
                        if part.kind == Kind::Points {
                            for (px, py) in &pts {
                                out += &format!("<circle cx=\"{px:.1}\" cy=\"{py:.1}\" r=\"3\" fill=\"{shade}\"/>\n");
                            }
                            continue;
                        }
                        let mut d = String::new();
                        for (i, (px, py)) in pts.iter().enumerate() {
                            if i == 0 {
                                d += &format!("M{px:.1},{py:.1}");
                            } else if part.kind == Kind::Steps {
                                d += &format!(" H{px:.1} V{py:.1}");
                            } else {
                                d += &format!(" L{px:.1},{py:.1}");
                            }
                        }
                        if part.kind == Kind::Area
                            && let (Some(first), Some(last)) = (pts.first(), pts.last())
                        {
                            out += &format!(
                                "<path d=\"{d} L{:.1},{:.1} L{:.1},{:.1} Z\" fill=\"{shade}\" fill-opacity=\"0.25\"/>\n",
                                last.0,
                                y(0.0),
                                first.0,
                                y(0.0)
                            );
                        }
                        out +=
                            &format!("<path d=\"{d}\" fill=\"none\" stroke=\"{shade}\" stroke-width=\"2\" stroke-linejoin=\"round\"/>\n");
                    }
                }
            }
        }
        out + "</svg>\n"
    }

    /// A value as the chart says it: an amount with its currency's code, a number plain.
    pub fn figure(&self, v: f64) -> String {
        // a zero is a zero: an empty sum of floats is -0.0
        let v = if v == 0.0 { 0.0 } else { v };
        // whole when it is its own truncation (no `fract` without std)
        let n = if v == (v as i64) as f64 { format!("{v:.0}") } else { format!("{v:.2}") };
        match &self.unit {
            Some(u) => format!("{n} {u}"),
            None => n,
        }
    }
}

/// A scale a person reads over `lo..hi` in about `steps` steps: its ends whole steps, its
/// step 1, 2 or 5 times a power of ten. What every reader draws a chart's scale by.
pub fn scale(lo: f64, hi: f64, steps: u32) -> (f64, f64, f64) {
    let step = nice((hi - lo) / steps.max(1) as f64);
    (whole(lo / step, false) * step, whole(hi / step, true) * step, step)
}

/// The step of a scale near `rough`: 1, 2 or 5 times a power of ten.
fn nice(rough: f64) -> f64 {
    let mut power = 1.0;
    while power * 10.0 <= rough {
        power *= 10.0;
    }
    while power > rough && power > 1e-9 {
        power /= 10.0;
    }
    [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * power).find(|s| *s >= rough).unwrap_or(10.0 * power)
}

/// `x` to a whole number: up (`ceil`) or down (`floor`), without std's.
fn whole(x: f64, up: bool) -> f64 {
    let t = x as i64 as f64;
    if t == x {
        t
    } else if up == (x > 0.0) {
        t + if up { 1.0 } else { -1.0 }
    } else {
        t
    }
}

/// Text as XML holds it.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
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
