//! How a value looks (SPEC.md, How a value looks): a choice's word in a tint, and the rules
//! a `look` statement gives a column, a named type or the row. A look is presentation
//! alone: it changes what a reader draws, never a value, a key, a reference or what a
//! writer writes. It is the value's wherever the value shows, a table's cell and a chart's
//! mark alike.
//!
//! What a reader needs of the world to draw one (its theme's colours, its locale's
//! separators and month names) it hands in: the core names a tint and formats with the
//! `Locale` given, and keeps no theme and no locale of its own.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ops::Range;

use crate::value::{Decimal, Instant, civil_from_days, pow10};

/// The tints a name says, in the order a picker shows them. Each reader draws a name in its
/// own theme's colour, light and dark, at a contrast that reads.
pub const TINTS: [&str; 10] = ["red", "orange", "yellow", "green", "teal", "blue", "purple", "pink", "brown", "gray"];

/// A tint: one of `TINTS` by its index, or an exact colour, `0xRRGGBB`, the same in every
/// theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tint {
    Named(u8),
    Hex(u32),
}

impl Tint {
    /// A tint as written after a choice's `#`: a name, or six hexadecimal digits.
    pub fn after_hash(word: &str) -> Result<Tint, String> {
        if let Some(k) = TINTS.iter().position(|t| *t == word) {
            return Ok(Tint::Named(k as u8));
        }
        if word.len() == 6 && word.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Ok(Tint::Hex(u32::from_str_radix(word, 16).expect("six hex digits")));
        }
        Err(format!("\"{word}\" is no tint: {}, or # and six hex digits", TINTS.join(", ")))
    }

    /// A tint as a look's rule writes it: a name, or `#` and six hexadecimal digits.
    pub fn parse(word: &str) -> Result<Tint, String> {
        match word.strip_prefix('#') {
            Some(hex) if hex.len() == 6 && hex.bytes().all(|c| c.is_ascii_hexdigit()) => Tint::after_hash(hex),
            Some(_) => Err(format!("\"{word}\" is no tint: # is followed by six hex digits")),
            None => match TINTS.iter().position(|t| *t == word) {
                Some(k) => Ok(Tint::Named(k as u8)),
                None => Err(format!("\"{word}\" is no tint: {}, or # and six hex digits", TINTS.join(", "))),
            },
        }
    }

    /// `green`, `#3fa34d`.
    pub fn spelled(self) -> String {
        match self {
            Tint::Named(k) => TINTS[k as usize].to_owned(),
            Tint::Hex(rgb) => format!("#{rgb:06x}"),
        }
    }

    /// What follows a choice's `#`: `green`, `3fa34d`.
    pub fn hashed(self) -> String {
        match self {
            Tint::Named(k) => TINTS[k as usize].to_owned(),
            Tint::Hex(rgb) => format!("{rgb:06x}"),
        }
    }

    pub fn name(self) -> Option<&'static str> {
        match self {
            Tint::Named(k) => Some(TINTS[k as usize]),
            Tint::Hex(_) => None,
        }
    }

    /// What it is, in words: what a hover says.
    pub fn said(self) -> String {
        match self {
            Tint::Named(k) => format!("the tint {}: each reader draws its own theme's {}, light and dark", TINTS[k as usize], TINTS[k as usize]),
            Tint::Hex(rgb) => format!("the colour #{rgb:06x}, the same in every theme"),
        }
    }
}

/// What a value is drawn in: one tint, or a point between two (a scale's), `at` from 0 (the
/// first) to 1000 (the second), so a reader mixes the two of its own theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tinted {
    One(Tint),
    Between { from: Tint, to: Tint, at: u16 },
}

/// One rule of a `look` statement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rule {
    /// `grouped`: thousands grouped, in the reader's locale.
    Grouped,
    /// `decimals 2`: a number shown at that many digits after its point, for display only.
    Decimals(u8),
    /// `negative in parentheses`: `(42.10)`, as accounts write a negative amount.
    Parentheses,
    /// `negative red`, `positive green`: a number's colour by its sign.
    Negative(Tint),
    Positive(Tint),
    /// `red when balance < budget`: a formula over the row, a boolean.
    When {
        tint: Tint,
        condition: String,
    },
    /// `scale red..green`: a colour from the column's least value to its greatest.
    Scale {
        from: Tint,
        to: Tint,
    },
    /// `as 'D MMM YYYY'`: a date's or a time's display form.
    As(Pattern),
}

impl Rule {
    /// Whether it says how a number reads (and so the reader's decimal mark).
    pub fn formats_numbers(&self) -> bool {
        matches!(self, Rule::Grouped | Rule::Decimals(_) | Rule::Parentheses)
    }

    /// Whether it holds only of a number.
    pub fn wants_number(&self) -> bool {
        matches!(self, Rule::Grouped | Rule::Decimals(_) | Rule::Parentheses | Rule::Negative(_) | Rule::Positive(_) | Rule::Scale { .. })
    }

    /// What it does, in words: what a hover says.
    pub fn said(&self) -> String {
        match self {
            Rule::Grouped => "its thousands grouped, in the reader's marks".to_owned(),
            Rule::Decimals(n) => format!("shown at {n} digits after the point, rounded half to even for display only"),
            Rule::Parentheses => "below zero shown in parentheses, as accounts write it: (950.00)".to_owned(),
            Rule::Negative(t) => format!("drawn in {} below zero", t.spelled()),
            Rule::Positive(t) => format!("drawn in {} above zero", t.spelled()),
            Rule::When { tint, condition } => format!("drawn in {} where {condition} holds", tint.spelled()),
            Rule::Scale { from, to } => {
                format!("drawn from {} at the column's least value to {} at its greatest", from.spelled(), to.spelled())
            }
            Rule::As(p) => format!("a date or a time shown as {}, in the reader's words; the file writes it as ever", p.spelled),
        }
    }
}

/// The words a look's rule opens with, and what each says: what an editor completes.
pub const RULE_WORDS: [(&str, &str); 7] = [
    ("grouped", "thousands grouped, in the reader's marks"),
    ("decimals", "decimals 2: a num or a pct at that many digits"),
    ("negative", "negative red, or negative in parentheses"),
    ("positive", "positive green: its colour above zero"),
    ("scale", "scale red..green: from the least value to the greatest"),
    ("as", "as 'D MMM YYYY': a date's or a time's display form"),
    ("when", "red when balance < 0: a tint where a condition over the row holds"),
];

/// A rule as a look statement holds it: as written, what it says, and the prelude line it
/// is written on in the file whose rule it is (the line of the import that brought it, for
/// a type another file exports).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Look {
    pub spelled: String,
    pub rule: Rule,
    pub line: u32,
}

/// What a `look` statement names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// `look row:`, every row by its conditions.
    Row,
    /// A column or a named type, by its name; `"row"` quoted names a column of that name.
    Name(String),
}

/// `target: rules`, what follows `look`: the target and each rule as written, split at the
/// commas outside parentheses and quotes.
pub fn statement(said: &str) -> Result<(Target, Vec<String>), String> {
    let (target, rest) = if let Some(q) = said.strip_prefix('"') {
        let close = q.find('"').ok_or("a quoted name is never closed")?;
        let rest = q[close + 1..].trim_start_matches([' ', '\t']).strip_prefix(':').ok_or("look names what it is for, then a `:`")?;
        (Target::Name(q[..close].to_owned()), rest)
    } else {
        let colon = said.find(':').ok_or("look names what it is for, then a `:`: look amount: grouped")?;
        let name = said[..colon].trim_matches([' ', '\t']);
        if name.is_empty() {
            return Err("look names a column, a type or row: look amount: grouped".to_owned());
        }
        (if name == "row" { Target::Row } else { Target::Name(name.to_owned()) }, &said[colon + 1..])
    };
    let rules = split(rest);
    if rules.iter().all(|r| r.is_empty()) {
        return Err("look says at least one rule: grouped, negative red, red when …".to_owned());
    }
    if let Some(empty) = rules.iter().position(String::is_empty) {
        return Err(format!("rule {} is empty: rules are joined by commas", empty + 1));
    }
    Ok((target, rules))
}

/// `s` split at its commas outside parentheses and quotes, each part's spaces cut: a
/// look's rules as written.
pub fn split(s: &str) -> Vec<String> {
    ranges(s).into_iter().map(|r| s[r].to_owned()).collect()
}

/// Where each part of `s` stands, split as `split` splits it, its spaces cut.
pub fn ranges(s: &str) -> Vec<Range<usize>> {
    let b = s.as_bytes();
    let trimmed = |mut from: usize, mut to: usize| {
        while from < to && matches!(b[from], b' ' | b'\t') {
            from += 1;
        }
        while to > from && matches!(b[to - 1], b' ' | b'\t') {
            to -= 1;
        }
        from..to
    };
    let mut out = Vec::new();
    let (mut depth, mut quote, mut from) = (0i32, 0u8, 0);
    for (i, &c) in b.iter().enumerate() {
        match c {
            _ if quote != 0 => {
                if c == quote {
                    quote = 0;
                }
            }
            b'\'' | b'"' => quote = c,
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                out.push(trimmed(from, i));
                from = i + 1;
            }
            _ => {}
        }
    }
    out.push(trimmed(from, b.len()));
    out
}

/// A prelude line read as a look statement, for a client marking its parts: where its
/// target stands, its `:`, and each rule; none for a line that opens no look statement
/// or names nothing before a `:`.
pub fn spans(line: &str) -> Option<(Range<usize>, usize, Vec<Range<usize>>)> {
    let blank = [' ', '\t'];
    let opened = line.trim_start_matches(blank).strip_prefix("look").filter(|r| r.starts_with(blank))?;
    let start = line.len() - opened.trim_start_matches(blank).len();
    let target = if line[start..].starts_with('"') {
        start..start + 1 + line[start + 1..].find('"')? + 1
    } else {
        let colon = start + line[start..].find(':')?;
        start..start + line[start..colon].trim_end_matches(blank).len()
    };
    let colon = target.end + line[target.end..].len() - line[target.end..].trim_start_matches(blank).len();
    if target.is_empty() || line.as_bytes().get(colon) != Some(&b':') {
        return None;
    }
    let rules = ranges(&line[colon + 1..]).into_iter().map(|r| colon + 1 + r.start..colon + 1 + r.end).collect();
    Some((target, colon, rules))
}

/// A rule as written: `grouped`, `decimals 2`, `negative in parentheses`, `negative red`,
/// `positive green`, `scale red..green`, `as 'D MMM YYYY'`, `<tint> when <condition>`.
pub fn rule(spelled: &str) -> Result<Rule, String> {
    let words: Vec<&str> = spelled.split([' ', '\t']).filter(|w| !w.is_empty()).collect();
    match words.as_slice() {
        ["grouped"] => Ok(Rule::Grouped),
        ["decimals", n] => {
            n.parse::<u8>().ok().filter(|n| *n <= 18).map(Rule::Decimals).ok_or_else(|| format!("decimals takes 0 to 18, not \"{n}\""))
        }
        ["negative", "in", "parentheses"] => Ok(Rule::Parentheses),
        ["negative", t] => Tint::parse(t).map(Rule::Negative),
        ["positive", t] => Tint::parse(t).map(Rule::Positive),
        ["scale", range] => {
            let (a, b) = range.split_once("..").ok_or("a scale runs from one tint to another: scale red..green")?;
            Ok(Rule::Scale { from: Tint::parse(a)?, to: Tint::parse(b)? })
        }
        ["as", ..] => {
            let rest = spelled.trim_start_matches([' ', '\t'])[2..].trim_matches([' ', '\t']);
            let inner = rest
                .strip_prefix('\'')
                .and_then(|r| r.strip_suffix('\''))
                .filter(|r| !r.is_empty())
                .ok_or("as takes a pattern in single quotes: as 'D MMM YYYY'")?;
            Pattern::parse(&inner.replace("''", "'")).map(Rule::As)
        }
        [t, "when", ..] => {
            let tint = Tint::parse(t)?;
            let at = spelled.find("when").expect("the word is there") + 4;
            let condition = spelled[at..].trim_matches([' ', '\t']);
            if condition.is_empty() {
                return Err("when is followed by a condition over the row: red when balance < 0".to_owned());
            }
            Ok(Rule::When { tint, condition: condition.to_owned() })
        }
        _ => Err(format!(
            "\"{spelled}\" is no rule: grouped, decimals 2, negative in parentheses, negative red, positive green, scale red..green, as 'D MMM YYYY', red when …"
        )),
    }
}

/// One piece of a display pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    /// `YYYY`, `YY`.
    Year(u8),
    /// `MMMM` (its name), `MMM` (its short name), `MM`, `M`.
    Month(u8),
    /// `DD`, `D`.
    Day(u8),
    /// `dddd` (its name), `ddd` (its short name).
    Weekday(u8),
    /// `HH`, `H` (24 hours), `hh`, `h` (12).
    Hour {
        twelve: bool,
        width: u8,
    },
    /// `mm`, `ss`.
    Minute,
    Second,
    /// `A`: the locale's word for before or after noon.
    Meridiem,
    Literal(String),
}

impl Piece {
    fn is_date(&self) -> bool {
        matches!(self, Piece::Year(_) | Piece::Month(_) | Piece::Day(_) | Piece::Weekday(_))
    }

    fn is_time(&self) -> bool {
        matches!(self, Piece::Hour { .. } | Piece::Minute | Piece::Second | Piece::Meridiem)
    }
}

/// A date's or a time's display form: `D MMM YYYY`, `dddd, D MMMM`, `HH:mm`, `h:mm A`.
/// Letters are pieces; a word is written as it is between brackets (`D [de] MMMM`), every
/// other character as it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub spelled: String,
    pub pieces: Vec<Piece>,
}

impl Pattern {
    pub fn parse(spelled: &str) -> Result<Pattern, String> {
        const PIECES: [(&str, Piece); 17] = [
            ("YYYY", Piece::Year(4)),
            ("YY", Piece::Year(2)),
            ("MMMM", Piece::Month(4)),
            ("MMM", Piece::Month(3)),
            ("MM", Piece::Month(2)),
            ("M", Piece::Month(1)),
            ("DD", Piece::Day(2)),
            ("D", Piece::Day(1)),
            ("dddd", Piece::Weekday(4)),
            ("ddd", Piece::Weekday(3)),
            ("HH", Piece::Hour { twelve: false, width: 2 }),
            ("H", Piece::Hour { twelve: false, width: 1 }),
            ("hh", Piece::Hour { twelve: true, width: 2 }),
            ("h", Piece::Hour { twelve: true, width: 1 }),
            ("mm", Piece::Minute),
            ("ss", Piece::Second),
            ("A", Piece::Meridiem),
        ];
        let mut pieces: Vec<Piece> = Vec::new();
        let mut rest = spelled;
        while let Some(c) = rest.chars().next() {
            if let Some(word) = rest.strip_prefix('[') {
                let close = word.find(']').ok_or("a [ in a pattern is never closed")?;
                pieces.push(Piece::Literal(word[..close].to_owned()));
                rest = &word[close + 1..];
                continue;
            }
            if let Some((t, p)) = PIECES.iter().find(|(t, _)| rest.starts_with(t)) {
                pieces.push(p.clone());
                rest = &rest[t.len()..];
                continue;
            }
            if c.is_alphabetic() {
                return Err(format!(
                    "\"{c}\" is no piece of a pattern: YYYY, MMMM, MMM, MM, DD, D, dddd, HH, hh, mm, ss, A; a word between [brackets]"
                ));
            }
            match pieces.last_mut() {
                Some(Piece::Literal(l)) => l.push(c),
                _ => pieces.push(Piece::Literal(c.to_string())),
            }
            rest = &rest[c.len_utf8()..];
        }
        if pieces.iter().all(|p| matches!(p, Piece::Literal(_))) {
            return Err("a pattern holds at least one piece: YYYY, MMM, D, HH, mm".to_owned());
        }
        Ok(Pattern { spelled: spelled.to_owned(), pieces })
    }

    pub fn has_date(&self) -> bool {
        self.pieces.iter().any(Piece::is_date)
    }

    pub fn has_time(&self) -> bool {
        self.pieces.iter().any(Piece::is_time)
    }

    /// A day (days since 1970-01-01) and a second of the day, as this pattern shows them.
    pub fn show(&self, days: i32, seconds: i32, locale: &Locale) -> String {
        let (y, m, d) = civil_from_days(days);
        // 1970-01-01 was a Thursday: Monday is 0.
        let weekday = (days.rem_euclid(7) + 3) % 7;
        let (h, min, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
        let mut out = String::new();
        for p in &self.pieces {
            match p {
                Piece::Year(4) => out.push_str(&format!("{y:04}")),
                Piece::Year(_) => out.push_str(&format!("{:02}", y.rem_euclid(100))),
                Piece::Month(4) => out.push_str(locale.months[m as usize - 1]),
                Piece::Month(3) => out.push_str(locale.months_short[m as usize - 1]),
                Piece::Month(2) => out.push_str(&format!("{m:02}")),
                Piece::Month(_) => out.push_str(&m.to_string()),
                Piece::Day(2) => out.push_str(&format!("{d:02}")),
                Piece::Day(_) => out.push_str(&d.to_string()),
                Piece::Weekday(4) => out.push_str(locale.days[weekday as usize]),
                Piece::Weekday(_) => out.push_str(locale.days_short[weekday as usize]),
                Piece::Hour { twelve, width } => {
                    let shown = if *twelve { (h + 11) % 12 + 1 } else { h };
                    out.push_str(&if *width == 2 { format!("{shown:02}") } else { shown.to_string() });
                }
                Piece::Minute => out.push_str(&format!("{min:02}")),
                Piece::Second => out.push_str(&format!("{s:02}")),
                Piece::Meridiem => out.push_str(if h < 12 { locale.am } else { locale.pm }),
                Piece::Literal(l) => out.push_str(l),
            }
        }
        out
    }
}

/// What a reader's locale says of how values read: its grouping and decimal marks, and the
/// names of its months and days (Monday first). A reader hands its own; `ENGLISH` is a
/// reader's that has none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Locale<'a> {
    pub group: &'a str,
    pub decimal: &'a str,
    pub months: [&'a str; 12],
    pub months_short: [&'a str; 12],
    pub days: [&'a str; 7],
    pub days_short: [&'a str; 7],
    pub am: &'a str,
    pub pm: &'a str,
}

pub const ENGLISH: Locale<'static> = Locale {
    group: ",",
    decimal: ".",
    months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
    months_short: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
    days: ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"],
    days_short: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
    am: "AM",
    pm: "PM",
};

impl<'a> Locale<'a> {
    /// A locale handed across a boundary (wasm, the C ABI) as its 42 lines in order: the
    /// grouping mark, the decimal mark, the 12 months' names, their 12 short names, the 7
    /// weekdays' names from Monday, their 7 short names, the words for before and after
    /// noon. An empty text is `ENGLISH`.
    pub fn lines(text: &'a str) -> Result<Locale<'a>, String> {
        if text.is_empty() {
            return Ok(ENGLISH);
        }
        let l: Vec<&str> = text.split('\n').collect();
        if l.len() != 42 {
            return Err(format!("a locale is 42 lines, and this is {}", l.len()));
        }
        fn take<'a, const N: usize>(l: &[&'a str], from: usize) -> [&'a str; N] {
            core::array::from_fn(|i| l[from + i])
        }
        Ok(Locale {
            group: l[0],
            decimal: l[1],
            months: take(&l, 2),
            months_short: take(&l, 14),
            days: take(&l, 26),
            days_short: take(&l, 33),
            am: l[40],
            pm: l[41],
        })
    }
}

/// `d` at `scale` digits after its point, half to even, for display.
fn rounded(d: Decimal, scale: u8) -> Decimal {
    if d.scale <= scale {
        // more digits than 38 hold: the number as it is
        return d.at_scale(scale).map_or(d, |mantissa| Decimal { mantissa, scale });
    }
    let div = pow10((d.scale - scale) as u32);
    let (q, r) = (d.mantissa / div, d.mantissa % div);
    let twice = r.unsigned_abs() * 2;
    let up = twice > div as u128 || (twice == div as u128 && q % 2 != 0);
    let q = if up { q + d.mantissa.signum() } else { q };
    Decimal { mantissa: q, scale }
}

/// A number as its rules show it: at their `decimals`, `grouped` in the locale, negative in
/// parentheses; the locale's decimal mark when any rule says how it reads, else written as
/// the file writes it.
pub fn number(d: Decimal, rules: &[&Rule], locale: &Locale) -> String {
    if !rules.iter().any(|r| r.formats_numbers()) {
        return d.to_string();
    }
    let mut d = d;
    if let Some(n) = rules.iter().rev().find_map(|r| match r {
        Rule::Decimals(n) => Some(*n),
        _ => None,
    }) {
        d = rounded(d, n);
    }
    let negative = d.mantissa < 0;
    let digits = Decimal { mantissa: d.mantissa.abs(), scale: d.scale }.to_string();
    let (whole, fraction) = match digits.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (digits.as_str(), None),
    };
    let mut out = String::new();
    if rules.iter().any(|r| matches!(r, Rule::Grouped)) {
        let n = whole.len();
        for (i, c) in whole.chars().enumerate() {
            if i > 0 && (n - i) % 3 == 0 {
                out.push_str(locale.group);
            }
            out.push(c);
        }
    } else {
        out.push_str(whole);
    }
    if let Some(f) = fraction {
        out.push_str(locale.decimal);
        out.push_str(f);
    }
    match negative {
        true if rules.iter().any(|r| matches!(r, Rule::Parentheses)) => format!("({out})"),
        true => format!("-{out}"),
        false => out,
    }
}

/// An instant's day and second of the day, in UTC.
pub fn instant_parts(t: Instant) -> (i32, i32) {
    (t.seconds.div_euclid(86_400) as i32, t.seconds.rem_euclid(86_400) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        Decimal::parse(s.as_bytes()).unwrap()
    }

    #[test]
    fn tints() {
        assert_eq!(Tint::after_hash("green").unwrap().spelled(), "green");
        assert_eq!(Tint::after_hash("3FA34D").unwrap().spelled(), "#3fa34d");
        assert_eq!(Tint::parse("#3fa34d").unwrap(), Tint::Hex(0x3fa34d));
        assert!(Tint::after_hash("grene").is_err() && Tint::parse("#3fa").is_err() && Tint::parse("3fa34d").is_err());
    }

    #[test]
    fn rules() {
        let (t, r) = statement("balance: red when balance < budget, green when COALESCE(balance, 0) >= budget").unwrap();
        assert_eq!(t, Target::Name("balance".into()));
        assert_eq!(r, ["red when balance < budget", "green when COALESCE(balance, 0) >= budget"]);
        assert_eq!(rule(&r[1]).unwrap(), Rule::When { tint: Tint::Named(3), condition: "COALESCE(balance, 0) >= budget".into() });
        assert_eq!(statement("row: gray when paid").unwrap().0, Target::Row);
        assert_eq!(statement("\"row\": grouped").unwrap().0, Target::Name("row".into()));
        assert_eq!(rule("scale red..green").unwrap(), Rule::Scale { from: Tint::Named(0), to: Tint::Named(3) });
        assert_eq!(rule("negative in parentheses").unwrap(), Rule::Parentheses);
        for bad in ["negative grene", "decimals 40", "scale red", "as D MMM", "blue when", "bold", "as 'Q'"] {
            assert!(rule(bad).is_err(), "{bad}");
        }
        assert!(statement("amount: grouped,").is_err() && statement("amount:").is_err());
    }

    #[test]
    fn numbers() {
        let g = Rule::Grouped;
        let p = Rule::Parentheses;
        let two = Rule::Decimals(2);
        assert_eq!(number(d("-1234567.5"), &[&g], &ENGLISH), "-1,234,567.5");
        assert_eq!(number(d("-950.00"), &[&p], &ENGLISH), "(950.00)");
        assert_eq!(number(d("2.345"), &[&two], &ENGLISH), "2.34");
        assert_eq!(number(d("2.355"), &[&two], &ENGLISH), "2.36");
        assert_eq!(number(d("7"), &[&two], &ENGLISH), "7.00");
        let es = Locale { group: ".", decimal: ",", ..ENGLISH };
        assert_eq!(number(d("1234.5"), &[&g], &es), "1.234,5");
        assert_eq!(number(d("1234.5"), &[&Rule::Negative(Tint::Named(0))], &es), "1234.5", "a colour says nothing of how it reads");
    }

    #[test]
    fn a_locale_by_its_lines() {
        let e = ENGLISH;
        let mut l: Vec<&str> = Vec::from([e.group, e.decimal]);
        l.extend(e.months.iter().chain(&e.months_short).chain(&e.days).chain(&e.days_short));
        l.extend([e.am, e.pm]);
        assert_eq!(Locale::lines(&l.join("\n")), Ok(ENGLISH));
        assert_eq!(Locale::lines(""), Ok(ENGLISH));
        assert!(Locale::lines(".\n,").is_err());
    }

    #[test]
    fn patterns() {
        let p = Pattern::parse("dddd, D MMM YYYY").unwrap();
        // 2026-11-03 was a Tuesday
        let days = crate::value::days_from_civil(2026, 11, 3);
        assert_eq!(p.show(days, 0, &ENGLISH), "Tuesday, 3 Nov 2026");
        assert_eq!(Pattern::parse("D [de] MMMM").unwrap().show(days, 0, &ENGLISH), "3 de November");
        assert_eq!(Pattern::parse("h:mm A").unwrap().show(0, 13 * 3600 + 5 * 60, &ENGLISH), "1:05 PM");
        assert!(Pattern::parse("Q").is_err() && Pattern::parse("[x]").is_err() && Pattern::parse("[open").is_err());
    }
}
