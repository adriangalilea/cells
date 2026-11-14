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
    /// The scale its values are drawn on, every layer's; none for a figure or a heat drawn
    /// alone, whose rows stand where a scale's labels would.
    pub scale: Option<Scale>,
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
/// wear (SPEC.md, What a chart is drawn in). Gray is not among them: it is the quiet's,
/// `other`'s and the blank's.
pub const UNWORN: [&str; 9] = ["blue", "orange", "teal", "purple", "pink", "yellow", "brown", "green", "red"];

/// The tints of `UNWORN` none of `words` wears, in turn: as many words without a tint of
/// their own can be told apart.
pub fn free(words: &[String], own: impl Fn(&str) -> Option<Tint>) -> Vec<Tint> {
    let worn: Vec<Tint> = words.iter().filter_map(|w| own(w)).collect();
    UNWORN
        .iter()
        .map(|n| Tint::Named(TINTS.iter().position(|t| t == n).expect("an unworn tint is one of the ten") as u8))
        .filter(|t| !worn.contains(t))
        .collect()
}

/// Each word its tint: its own where it has one, else the next of `free`, the words in
/// their lasting order. The words without a tint are no more than the tints free.
pub fn shades(words: &[String], own: impl Fn(&str) -> Option<Tint>, free: &[Tint]) -> Vec<Shade> {
    let bare = words.iter().filter(|w| own(w).is_none()).count();
    assert!(bare <= free.len(), "{bare} words without a tint and {} tints free to tell them apart", free.len());
    let mut free = free.iter();
    words.iter().map(|w| Shade::Tint(own(w).unwrap_or_else(|| *free.next().expect("counted above")))).collect()
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
            "{{\"kind\":{},\"title\":{},\"stack\":{},\"axis\":{},\"places\":[{}],\"series\":[{}],\"values\":[{}],\"unit\":{},\"scale\":{},\"skipped\":{},\"rows\":{},\"layers\":[{}]}}",
            quote(self.kind.word()),
            quote(&self.title),
            quote(self.stack.word()),
            quote(self.axis),
            strings(&self.places),
            series.join(","),
            values.join(","),
            self.unit.as_ref().map_or("null".to_owned(), |u| quote(u)),
            self.scale.as_ref().map_or("null".to_owned(), Scale::json),
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

/// Layers on one plot, told apart (SPEC.md, What a chart is drawn in): a layer whose one
/// series wears a column's colour a layer before it wears takes the next of the columns'
/// colours none before it wears, so two layers measuring columns at one place in two
/// tables, or one column twice, differ. Every other series is as its layer draws it alone;
/// past the six colours, a layer keeps its column's.
pub fn apart(layers: Vec<Summary>) -> Vec<Summary> {
    let mut worn: Vec<usize> = Vec::new();
    layers
        .into_iter()
        .map(|mut layer| {
            if let [(_, Shade::Column(k))] = layer.series.as_mut_slice() {
                if let Some(free) = (*k..*k + COLUMNS.len()).find(|c| !worn.contains(&(c % COLUMNS.len()))) {
                    *k = free;
                }
                worn.push(*k % COLUMNS.len());
            }
            layer
        })
        .collect()
}

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
    /// The chart as an SVG of `width` by `height`, for a page or a README: its title, its
    /// scale, its marks in its series' shades (the light page's), each
    /// place named under its mark. A chart of layers draws each layer on one plot, a heat
    /// among them as its stacked bars; a heat drawn alone names its rows at their left.
    pub fn svg(&self, width: u32, height: u32) -> String {
        let (w, h) = (width as f64, height as f64);
        let parts: Vec<&Summary> = if self.layers.is_empty() { alloc::vec![self] } else { self.layers.iter().collect() };
        let heat = self.kind == Kind::Heat && self.layers.is_empty();
        // every series by its kind's mark and its name in its shade, once there is more than
        // one; a heat drawn alone has none, its series being its rows
        let legend: Vec<(Kind, &str, String)> = parts
            .iter()
            .flat_map(|p| p.series.iter().map(|(n, s)| (p.kind, if n.is_empty() { p.title.as_str() } else { n.as_str() }, s.hex(false))))
            .collect();
        let legend = if legend.len() > 1 && !heat { legend } else { Vec::new() };
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
        for (kind, name, hex) in &legend {
            out += &format!("{}<text x=\"{:.1}\" y=\"38\" fill=\"#444\">{}</text>\n", swatch(*kind, at, hex), at + 16.0, escape(name));
            at += 28.0 + 6.5 * name.chars().count() as f64;
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
        // the scale, the summary's; a heat drawn alone has none, its rows at the plot's left
        let stacked = |p: &Summary| (p.kind == Kind::Bars && p.stack == Stack::Stacked) || p.kind == Kind::Heat;
        let scale = self.scale.unwrap_or(Scale { lo: 0.0, hi: 1.0, step: 1.0 });
        assert!(heat || self.scale.is_some(), "{}: a plot with no scale", self.title);
        let (lo, hi) = (scale.lo, scale.hi);
        let ticks: Vec<f64> = if heat { Vec::new() } else { (0..=scale.steps()).map(|i| scale.tick(i)).collect() };
        let rows: Vec<&str> = if heat { self.series.iter().map(|(n, _)| n.as_str()).collect() } else { Vec::new() };
        // the labels' room: their widest, at a sans serif's average width of 11 points
        let widest = ticks.iter().map(|v| self.figure(*v).chars().count()).chain(rows.iter().map(|n| n.chars().count())).max().unwrap_or(1);
        let left = inset + 6.5 * widest as f64 + 6.0;
        let pw = w - left - right;
        let y = |v: f64| top + ph * (hi - v) / (hi - lo);
        let ch = ph / rows.len().max(1) as f64;
        for (k, name) in rows.iter().enumerate() {
            out += &format!(
                "<text x=\"{}\" y=\"{:.1}\" text-anchor=\"end\" fill=\"#666\">{}</text>\n",
                left - 6.0,
                top + ch * (k as f64 + 0.5) + 4.0,
                escape(name)
            );
        }
        for &v in &ticks {
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
                Kind::Heat if heat => {
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
                Kind::Bars | Kind::Heat => {
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

/// A series' mark in the legend, twelve points wide from `x` on its line's middle: what its
/// kind draws, in its shade.
fn swatch(kind: Kind, x: f64, hex: &str) -> String {
    let stroke = format!("fill=\"none\" stroke=\"{hex}\" stroke-width=\"2\" stroke-linejoin=\"round\"");
    match kind {
        Kind::Bars | Kind::Heat | Kind::Figure => {
            format!("<rect x=\"{:.1}\" y=\"30\" width=\"8\" height=\"8\" fill=\"{hex}\" rx=\"2\"/>", x + 2.0)
        }
        Kind::Line => format!("<path d=\"M{x:.1},34 H{:.1}\" {stroke}/>", x + 12.0),
        Kind::Area => format!(
            "<path d=\"M{x:.1},31 L{:.1},29 V38 H{x:.1} Z\" fill=\"{hex}\" fill-opacity=\"0.25\"/><path d=\"M{x:.1},31 L{:.1},29\" {stroke}/>",
            x + 12.0,
            x + 12.0
        ),
        Kind::Steps => format!("<path d=\"M{x:.1},38 H{:.1} V30 H{:.1}\" {stroke}/>", x + 6.0, x + 12.0),
        Kind::Points => format!("<circle cx=\"{:.1}\" cy=\"34\" r=\"3\" fill=\"{hex}\"/>", x + 6.0),
    }
}

/// The steps a chart's scale is about, in every view at every size (SPEC.md, Charts).
pub const STEPS: u32 = 4;

/// The scale a chart's values are drawn on: `lo` to `hi` in whole steps of `step`, 1, 2 or
/// 5 times a power of ten. The data's alone, never a view's: a view too short to label
/// every step labels fewer, on the same axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
    pub lo: f64,
    pub hi: f64,
    pub step: f64,
}

impl Scale {
    /// The scale over what `parts` draw (each a kind, its stack and its values, a layer
    /// each): from zero, or the least below it, to the most a place draws, a stacked
    /// bar's sum (a heat's, as a layer, is its stacked bars), in about `STEPS` steps.
    pub fn drawn(parts: &[(Kind, Stack, &[Vec<Option<String>>])]) -> Scale {
        let num = |v: &Option<String>| v.as_deref().and_then(|s| s.parse::<f64>().ok());
        let (mut lo, mut hi) = (0.0f64, 0.0f64);
        for &(kind, stack, values) in parts {
            let stacked = (kind == Kind::Bars && stack == Stack::Stacked) || kind == Kind::Heat;
            for row in values {
                let vals: Vec<f64> = row.iter().filter_map(num).collect();
                if stacked {
                    hi = hi.max(vals.iter().filter(|v| **v > 0.0).sum());
                    lo = lo.min(vals.iter().filter(|v| **v < 0.0).sum());
                } else {
                    for v in &vals {
                        hi = hi.max(*v);
                        lo = lo.min(*v);
                    }
                }
            }
        }
        if hi <= lo {
            hi = lo + 1.0;
        }
        let (m, e) = nice((hi - lo) / STEPS as f64);
        let (from, to) = (whole(lo / at(m, e), false) as i64, whole(hi / at(m, e), true) as i64);
        // `+ 0.0`: a zero is a zero, never -0.0
        Scale { lo: at(from * m, e) + 0.0, hi: at(to * m, e) + 0.0, step: at(m, e) }
    }

    /// How many steps it spans.
    pub fn steps(&self) -> u32 {
        ((self.hi - self.lo) / self.step + 0.5) as u32
    }

    /// The value at step `i` from `lo`.
    pub fn tick(&self, i: u32) -> f64 {
        self.lo + self.step * i as f64
    }

    pub fn json(&self) -> String {
        format!("{{\"lo\":{},\"hi\":{},\"step\":{}}}", self.lo, self.hi, self.step)
    }
}

/// `units` times ten to the `e`, rounded once: `3 * 10^-1` is 0.3, not 0.30000000000000004.
fn at(units: i64, e: i32) -> f64 {
    let mut power = 1.0;
    for _ in 0..e.unsigned_abs() {
        power *= 10.0;
    }
    if e >= 0 { units as f64 * power } else { units as f64 / power }
}

/// The step of a scale near `rough`: 1, 2 or 5 times a power of ten, as the multiple and
/// the power's exponent.
fn nice(rough: f64) -> (i64, i32) {
    let mut e = 0;
    while at(1, e + 1) <= rough {
        e += 1;
    }
    while at(1, e) > rough && e > -9 {
        e -= 1;
    }
    [1, 2, 5].into_iter().find(|&m| at(m, e) >= rough).map_or((1, e + 1), |m| (m, e))
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
        let own = |w: &str| (w == "income").then_some(Tint::Named(5));
        let free = free(&words, own);
        assert_eq!(free.len(), 8);
        let s = shades(&words, own, &free);
        assert_eq!(s, [Shade::Tint(Tint::Named(5)), Shade::Tint(Tint::Named(1)), Shade::Tint(Tint::Named(4)), Shade::Tint(Tint::Named(6))]);
    }

    /// A scale over what a chart draws: about four steps of 1, 2 or 5 times a power of
    /// ten, its ends whole steps, each a decimal rounded once.
    #[test]
    fn a_scale_is_the_datas() {
        let of = |kind: Kind, stack: Stack, rows: &[&[&str]]| {
            let values: Vec<Vec<Option<String>>> = rows.iter().map(|r| r.iter().map(|v| Some((*v).to_owned())).collect()).collect();
            let s = Scale::drawn(&[(kind, stack, &values)]);
            (s.lo, s.hi, s.step)
        };
        assert_eq!(of(Kind::Bars, Stack::Stacked, &[&["2400"], &["1800"]]), (0.0, 3000.0, 1000.0));
        assert_eq!(of(Kind::Bars, Stack::Stacked, &[&["1500", "900"]]), (0.0, 3000.0, 1000.0));
        assert_eq!(of(Kind::Bars, Stack::Grouped, &[&["1500", "900"]]), (0.0, 1500.0, 500.0));
        assert_eq!(of(Kind::Line, Stack::Stacked, &[&["0.3"]]), (0.0, 0.3, 0.1));
        assert_eq!(of(Kind::Line, Stack::Stacked, &[&["-40"], &["90"]]), (-50.0, 100.0, 50.0));
        assert_eq!(of(Kind::Line, Stack::Stacked, &[&["0"]]), (0.0, 1.0, 0.5));
        let s = Scale::drawn(&[(Kind::Line, Stack::Stacked, &[alloc::vec![Some("0.3".to_owned())]])]);
        assert_eq!(s.json(), "{\"lo\":0,\"hi\":0.3,\"step\":0.1}");
        assert_eq!(s.steps(), 3);
    }

    fn drawn(kind: Kind, title: &str, series: &[(&str, Shade)], layers: Vec<Summary>) -> Summary {
        let one = || alloc::vec![Some("2".to_owned()); series.len()];
        let values = if layers.is_empty() { alloc::vec![one(), one()] } else { Vec::new() };
        let parts: Vec<(Kind, Stack, &[Vec<Option<String>>])> = if layers.is_empty() {
            alloc::vec![(kind, Stack::Stacked, values.as_slice())]
        } else {
            layers.iter().map(|l| (l.kind, l.stack, l.values.as_slice())).collect()
        };
        let scale = (kind != Kind::Heat || !layers.is_empty()).then(|| Scale::drawn(&parts));
        Summary {
            kind,
            title: title.to_owned(),
            stack: Stack::Stacked,
            axis: "text",
            places: if layers.is_empty() { alloc::vec!["mon".to_owned(), "tue".to_owned()] } else { Vec::new() },
            series: series.iter().map(|(n, s)| ((*n).to_owned(), *s)).collect(),
            values,
            unit: None,
            scale,
            skipped: 0,
            rows: 2,
            layers,
        }
    }

    /// The SVG's legend names each series by its kind's mark in its own shade; a heat drawn
    /// alone has none, its rows named at their left in place of a scale.
    #[test]
    fn the_svg_legend_marks_each_series() {
        let bars = drawn(Kind::Bars, "actual", &[("", Shade::Column(1))], Vec::new());
        let line = drawn(Kind::Line, "planned", &[("", Shade::Column(2))], Vec::new());
        let svg = drawn(Kind::Bars, "both", &[], alloc::vec![bars, line]).svg(720, 320);
        assert!(svg.contains("<rect x=\"18.0\" y=\"30\" width=\"8\" height=\"8\" fill=\"#6250d6\""), "{svg}");
        assert!(svg.contains("<path d=\"M83.0,34 H95.0\" fill=\"none\" stroke=\"#eb6834\""), "{svg}");

        let blue = Shade::Tint(Tint::Named(5));
        let svg = drawn(Kind::Heat, "weekdays", &[("9", blue), ("14", blue)], Vec::new()).svg(720, 320);
        assert!(!svg.contains("y=\"30\"") && !svg.contains("cy=\"34\""), "a heat has a legend: {svg}");
        assert!(!svg.contains("stroke-opacity"), "a heat has a scale: {svg}");
        assert!(svg.contains("text-anchor=\"end\" fill=\"#666\">9<") && svg.contains("text-anchor=\"end\" fill=\"#666\">14<"), "{svg}");
    }
}
