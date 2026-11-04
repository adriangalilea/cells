//! Cell values: exact decimals, whole numbers, dates, times, instants, and the checks
//! for URLs and email addresses. Every function reads the cell's text (unquoted) and
//! answers the value or nothing; the caller says why.

use core::cmp::Ordering;
use core::fmt;

/// The most digits a decimal holds, Arrow's `Decimal128` precision.
pub const PRECISION: u32 = 38;

const POW10: [i128; 39] = {
    let mut t = [1i128; 39];
    let mut i = 1;
    while i < 39 {
        t[i] = t[i - 1] * 10;
        i += 1;
    }
    t
};

pub fn pow10(n: u32) -> i128 {
    POW10[n as usize]
}

/// An exact decimal: `mantissa / 10^scale`, at most 38 digits. Equal by value: `84.20`
/// is `84.2`.
#[derive(Clone, Copy, Debug)]
pub struct Decimal {
    pub mantissa: i128,
    pub scale: u8,
}

impl PartialEq for Decimal {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Decimal {}

impl core::hash::Hash for Decimal {
    fn hash<H: core::hash::Hasher>(&self, h: &mut H) {
        let n = self.normal();
        n.mantissa.hash(h);
        n.scale.hash(h);
    }
}

/// Why a number's text is no decimal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberError {
    /// Not written as a number.
    Syntax,
    /// A number, but more than 38 significant digits.
    Range,
}

impl Decimal {
    pub const ZERO: Decimal = Decimal { mantissa: 0, scale: 0 };

    /// `-1.5`, `84.20`, `3e8`, `.5`, `+2`. The scale is the digits after the point
    /// less the exponent, never below 0.
    pub fn parse(s: &[u8]) -> Result<Decimal, NumberError> {
        let n = s.len();
        let mut i = 0;
        let negative = match s.first() {
            Some(b'-') => {
                i = 1;
                true
            }
            Some(b'+') => {
                i = 1;
                false
            }
            _ => false,
        };
        let mut m: u128 = 0;
        let mut digits = 0u32; // significant digits accumulated into m
        let mut seen = 0usize; // digits before and after the point
        let mut frac = 0i32;
        let mut point = false;
        while i < n {
            let c = s[i];
            if c.is_ascii_digit() {
                seen += 1;
                if m != 0 || c != b'0' {
                    digits += 1;
                    if digits > PRECISION {
                        return Err(trailing_syntax(s, i).unwrap_or(NumberError::Range));
                    }
                }
                m = m * 10 + (c - b'0') as u128;
                if point {
                    frac += 1;
                }
            } else if c == b'.' && !point {
                point = true;
            } else {
                break;
            }
            i += 1;
        }
        if seen == 0 {
            return Err(NumberError::Syntax);
        }
        let mut exp: i32 = 0;
        if i < n {
            if s[i] != b'e' && s[i] != b'E' {
                return Err(NumberError::Syntax);
            }
            i += 1;
            let eneg = match s.get(i) {
                Some(b'-') => {
                    i += 1;
                    true
                }
                Some(b'+') => {
                    i += 1;
                    false
                }
                _ => false,
            };
            let start = i;
            while i < n && s[i].is_ascii_digit() {
                if exp < 100_000 {
                    exp = exp * 10 + (s[i] - b'0') as i32;
                }
                i += 1;
            }
            if i == start || i != n {
                return Err(NumberError::Syntax);
            }
            if eneg {
                exp = -exp;
            }
        }
        let mut scale = frac - exp;
        if m == 0 {
            return Ok(Decimal { mantissa: 0, scale: scale.clamp(0, PRECISION as i32) as u8 });
        }
        while scale > PRECISION as i32 && m.is_multiple_of(10) {
            m /= 10;
            scale -= 1;
            digits -= 1;
        }
        if scale > PRECISION as i32 {
            return Err(NumberError::Range);
        }
        if scale < 0 {
            if digits as i32 - scale > PRECISION as i32 {
                return Err(NumberError::Range);
            }
            m *= pow10((-scale) as u32) as u128;
            scale = 0;
        }
        let v = m as i128;
        Ok(Decimal { mantissa: if negative { -v } else { v }, scale: scale as u8 })
    }

    /// The same number with no zero after the point that changes nothing: `0.50` is
    /// `0.5`, `-0` is `0`.
    pub fn normal(self) -> Decimal {
        let mut d = self;
        while d.scale > 0 && d.mantissa % 10 == 0 {
            d.mantissa /= 10;
            d.scale -= 1;
        }
        d
    }

    /// The mantissa at a larger or equal scale, or nothing when it would pass 38 digits.
    pub fn at_scale(self, scale: u8) -> Option<i128> {
        debug_assert!(scale >= self.scale);
        let m = self.mantissa.checked_mul(pow10((scale - self.scale) as u32))?;
        if m.unsigned_abs() >= pow10(PRECISION) as u128 { None } else { Some(m) }
    }

    /// The nearest binary64: correctly rounded, as every implementation's number parser.
    pub fn to_f64(self) -> f64 {
        const EXACT: i128 = 1 << 53;
        if self.mantissa.abs() < EXACT && self.scale <= 22 {
            // Both exact, so one correctly rounded division.
            return self.mantissa as f64 / pow10(self.scale as u32) as f64;
        }
        let mut buf = [0u8; 48];
        let s = self.write(&mut buf);
        // A decimal's own digits always parse.
        s.parse::<f64>().unwrap()
    }

    /// Its digits into `buf`, `-84.20`, no exponent.
    pub fn write(self, buf: &mut [u8; 48]) -> &str {
        let mut digits = [0u8; 40];
        let mut n = 0;
        let mut m = self.mantissa.unsigned_abs();
        loop {
            digits[n] = b'0' + (m % 10) as u8;
            n += 1;
            m /= 10;
            if m == 0 {
                break;
            }
        }
        let scale = self.scale as usize;
        while n <= scale {
            digits[n] = b'0';
            n += 1;
        }
        let mut o = 0;
        if self.mantissa < 0 {
            buf[o] = b'-';
            o += 1;
        }
        for k in (0..n).rev() {
            buf[o] = digits[k];
            o += 1;
            if k == scale && k != 0 {
                buf[o] = b'.';
                o += 1;
            }
        }
        // ASCII digits, a sign and a point.
        core::str::from_utf8(&buf[..o]).unwrap()
    }
}

fn trailing_syntax(s: &[u8], from: usize) -> Option<NumberError> {
    // Too many digits, but only a number at all if the rest parses as one.
    let rest = &s[from..];
    let ok = rest.iter().all(|c| c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-'));
    if ok { None } else { Some(NumberError::Syntax) }
}

impl Ord for Decimal {
    fn cmp(&self, other: &Self) -> Ordering {
        let (a, b) = (*self, *other);
        if a.scale == b.scale {
            return a.mantissa.cmp(&b.mantissa);
        }
        let (lo, hi, flip) = if a.scale < b.scale { (a, b, false) } else { (b, a, true) };
        let o = match lo.mantissa.checked_mul(pow10((hi.scale - lo.scale) as u32)) {
            Some(m) => m.cmp(&hi.mantissa),
            // Past i128, so past anything the other holds: the sign decides.
            None => {
                if lo.mantissa < 0 {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            }
        };
        if flip { o.reverse() } else { o }
    }
}

impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; 48];
        f.write_str(self.write(&mut buf))
    }
}

/// `-12`, `0`, `+4200`, within `i64`.
#[inline]
pub fn parse_int(s: &[u8]) -> Option<i64> {
    let (negative, digits) = match s.first()? {
        b'-' => (true, &s[1..]),
        b'+' => (false, &s[1..]),
        _ => (false, s),
    };
    if digits.is_empty() {
        return None;
    }
    let mut v: u64 = 0;
    for &c in digits {
        let d = c.wrapping_sub(b'0');
        if d > 9 {
            return None;
        }
        v = v.checked_mul(10)?.checked_add(d as u64)?;
    }
    if negative { if v > i64::MAX as u64 + 1 { None } else { Some((v as i64).wrapping_neg()) } } else { i64::try_from(v).ok() }
}

/// Days from 1970-01-01 to a proleptic Gregorian date (Hinnant's `days_from_civil`).
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i32 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i32 - 719468
}

/// The date `days` after 1970-01-01, as (year, month, day).
pub fn civil_from_days(z: i32) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i32 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        2 => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[inline]
fn two(s: &[u8], i: usize) -> Option<u32> {
    let a = s[i].wrapping_sub(b'0');
    let b = s[i + 1].wrapping_sub(b'0');
    if a > 9 || b > 9 { None } else { Some(a as u32 * 10 + b as u32) }
}

#[inline]
fn four(s: &[u8], i: usize) -> Option<u32> {
    Some(two(s, i)? * 100 + two(s, i + 2)?)
}

/// A calendar date's days, if the date exists.
pub fn date_days(y: u32, m: u32, d: u32) -> Option<i32> {
    if !(1..=12).contains(&m) || d == 0 || d > days_in_month(y as i32, m) {
        return None;
    }
    Some(days_from_civil(y as i32, m, d))
}

/// `2026-08-14`.
#[inline]
pub fn parse_date(s: &[u8]) -> Option<i32> {
    if s.len() != 10 || s[4] != b'-' || s[7] != b'-' {
        return None;
    }
    date_days(four(s, 0)?, two(s, 5)?, two(s, 8)?)
}

/// Seconds since midnight from hours, minutes and seconds, if a real time of day.
pub fn time_seconds(h: u32, m: u32, s: u32) -> Option<i32> {
    if h > 23 || m > 59 || s > 59 {
        return None;
    }
    Some((h * 3600 + m * 60 + s) as i32)
}

/// `14:30` or `14:30:05`.
pub fn parse_time(s: &[u8]) -> Option<i32> {
    match s.len() {
        5 if s[2] == b':' => time_seconds(two(s, 0)?, two(s, 3)?, 0),
        8 if s[2] == b':' && s[5] == b':' => time_seconds(two(s, 0)?, two(s, 3)?, two(s, 6)?),
        _ => None,
    }
}

/// An instant: seconds since 1970-01-01T00:00:00Z and the nanoseconds after them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant {
    pub seconds: i64,
    pub nanos: u32,
}

impl Instant {
    /// The digits after the point its fraction needs, 0 to 9.
    pub fn digits(self) -> u8 {
        if self.nanos == 0 {
            return 0;
        }
        let mut n = self.nanos;
        let mut d = 9;
        while n.is_multiple_of(10) {
            n /= 10;
            d -= 1;
        }
        d
    }

    /// In units of `10^-unit` seconds, or nothing past `i64`.
    pub fn at(self, unit: u8) -> Option<i64> {
        debug_assert!(unit <= 9 && self.digits() <= unit);
        let s = self.seconds.checked_mul(10i64.pow(unit as u32))?;
        s.checked_add((self.nanos / 10u32.pow(9 - unit as u32)) as i64)
    }

    pub fn from(v: i64, unit: u8) -> Instant {
        let per = 10i64.pow(unit as u32);
        let seconds = v.div_euclid(per);
        let frac = v.rem_euclid(per) as u32;
        Instant { seconds, nanos: frac * 10u32.pow(9 - unit as u32) }
    }
}

/// `2026-08-14T14:30:00Z`, RFC 3339: a date, `T`, `HH:mm:ss`, a fraction of one to nine
/// digits or none, then `Z` or `+HH:mm` / `-HH:mm`.
pub fn parse_datetime(s: &[u8]) -> Option<Instant> {
    if s.len() < 20 || s[10] != b'T' || s[13] != b':' || s[16] != b':' {
        return None;
    }
    let days = parse_date(&s[..10])?;
    let t = time_seconds(two(s, 11)?, two(s, 14)?, two(s, 17)?)? as i64;
    let mut i = 19;
    let mut nanos: u32 = 0;
    if s[i] == b'.' {
        i += 1;
        let start = i;
        while i < s.len() && s[i].is_ascii_digit() {
            if i - start == 9 {
                return None;
            }
            nanos = nanos * 10 + (s[i] - b'0') as u32;
            i += 1;
        }
        if i == start {
            return None;
        }
        nanos *= 10u32.pow(9 - (i - start) as u32);
    }
    let offset: i64 = match &s[i..] {
        [b'Z'] => 0,
        [sign @ (b'+' | b'-'), rest @ ..] if rest.len() == 5 && rest[2] == b':' => {
            let (oh, om) = (two(rest, 0)?, two(rest, 3)?);
            if oh > 23 || om > 59 {
                return None;
            }
            let o = (oh * 3600 + om * 60) as i64;
            if *sign == b'+' { o } else { -o }
        }
        _ => return None,
    };
    Some(Instant { seconds: days as i64 * 86_400 + t - offset, nanos })
}

/// `2026-08-14`.
pub fn format_date(days: i32) -> alloc::string::String {
    let (y, m, d) = civil_from_days(days);
    alloc::format!("{y:04}-{m:02}-{d:02}")
}

/// `14:30:05`.
pub fn format_time(seconds: i32) -> alloc::string::String {
    alloc::format!("{:02}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60)
}

/// The instant in UTC, `2026-08-14T12:30:00.25Z`, a fraction only when it is not zero.
pub fn format_datetime(t: Instant) -> alloc::string::String {
    let days = t.seconds.div_euclid(86_400) as i32;
    let secs = t.seconds.rem_euclid(86_400) as i32;
    let mut s = alloc::format!("{}T{}", format_date(days), format_time(secs));
    if t.nanos != 0 {
        let f = alloc::format!("{:09}", t.nanos);
        s.push('.');
        s.push_str(f.trim_end_matches('0'));
    }
    s.push('Z');
    s
}

/// Crockford's base 32 in lower case: `0-9`, `a-z` but `i l o u`.
pub const ID_ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

/// `k7m2q9xa`: eight characters of `ID_ALPHABET`.
pub fn is_id(s: &[u8]) -> bool {
    s.len() == 8 && s.iter().all(|c| ID_ALPHABET.contains(c))
}

/// An id from 40 bits of randomness the caller supplies (the core keeps no clock and no
/// generator, so a seeded run gives the same ids).
pub fn id_from(bits: u64) -> [u8; 8] {
    let mut out = [0u8; 8];
    for (k, o) in out.iter_mut().enumerate() {
        *o = ID_ALPHABET[((bits >> (35 - 5 * k)) & 31) as usize];
    }
    out
}

/// An absolute URL as RFC 3986 writes it: a scheme (a letter, then letters, digits, `+`,
/// `-`, `.`), `:`, then at least one character, each a letter, a digit, one of
/// `-._~:/?#[]@!$&'()*+,;=`, or `%` and two hexadecimal digits.
pub fn is_url(s: &[u8]) -> bool {
    let Some(colon) = s.iter().position(|&c| c == b':') else { return false };
    let scheme = &s[..colon];
    if scheme.is_empty() || !scheme[0].is_ascii_alphabetic() {
        return false;
    }
    if !scheme.iter().all(|&c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.')) {
        return false;
    }
    let rest = &s[colon + 1..];
    if rest.is_empty() {
        return false;
    }
    let mut i = 0;
    while i < rest.len() {
        let c = rest[i];
        if c == b'%' {
            if !(i + 2 < rest.len() && rest[i + 1].is_ascii_hexdigit() && rest[i + 2].is_ascii_hexdigit()) {
                return false;
            }
            i += 3;
            continue;
        }
        if !(c.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&'()*+,;=".contains(&c)) {
            return false;
        }
        i += 1;
    }
    true
}

/// `local@domain`: the local part ASCII from `!` to `~` but `@`, the domain two or more
/// labels joined by `.`, each ASCII letters, digits and `-`, never opening or closing with
/// `-`.
pub fn is_email(s: &[u8]) -> bool {
    let Some(at) = s.iter().position(|&c| c == b'@') else { return false };
    let (local, domain) = (&s[..at], &s[at + 1..]);
    if local.is_empty() || !local.iter().all(|&c| (b'!'..=b'~').contains(&c)) {
        return false;
    }
    let mut labels = 0;
    for label in domain.split(|&c| c == b'.') {
        let ok = !label.is_empty()
            && label.iter().all(|&c| c.is_ascii_alphanumeric() || c == b'-')
            && label[0] != b'-'
            && label[label.len() - 1] != b'-';
        if !ok {
            return false;
        }
        labels += 1;
    }
    labels >= 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn decimals() {
        let d = |s: &str| Decimal::parse(s.as_bytes());
        assert_eq!(d("84.20"), Ok(Decimal { mantissa: 8420, scale: 2 }));
        assert_eq!(d("-1.5"), Ok(Decimal { mantissa: -15, scale: 1 }));
        assert_eq!(d("3e8"), Ok(Decimal { mantissa: 300_000_000, scale: 0 }));
        assert_eq!(d("1.5e-3"), Ok(Decimal { mantissa: 15, scale: 4 }));
        assert_eq!(d(".5"), Ok(Decimal { mantissa: 5, scale: 1 }));
        assert_eq!(d("5."), Ok(Decimal { mantissa: 5, scale: 0 }));
        assert_eq!(d("8x.50"), Err(NumberError::Syntax));
        assert_eq!(d("."), Err(NumberError::Syntax));
        assert_eq!(d("1e"), Err(NumberError::Syntax));
        assert_eq!(d("1e400"), Err(NumberError::Range));
        assert_eq!(d("-0.00").unwrap().to_string(), "0.00");
        assert_eq!(d("84.20").unwrap().to_string(), "84.20");
        assert_eq!(d("-0.05").unwrap().to_string(), "-0.05");
        assert!(d("84.2").unwrap() == d("84.2").unwrap());
        assert!(d("84.20").unwrap().cmp(&d("84.2").unwrap()).is_eq());
        assert!(d("-1").unwrap() < d("0.5").unwrap());
        assert_eq!(d("0.1").unwrap().to_f64(), 0.1);
    }

    #[test]
    fn dates() {
        assert_eq!(parse_date(b"1970-01-01"), Some(0));
        assert_eq!(parse_date(b"2026-08-14"), Some(20679));
        assert_eq!(parse_date(b"2026-02-29"), None);
        assert_eq!(parse_date(b"2024-02-29"), Some(19782));
        assert_eq!(civil_from_days(20679), (2026, 8, 14));
        assert_eq!(parse_time(b"14:30"), Some(52200));
        assert_eq!(parse_time(b"24:00"), None);
        let at = |s: i64, n: u32| Some(Instant { seconds: s, nanos: n });
        assert_eq!(parse_datetime(b"1970-01-01T00:00:00Z"), at(0, 0));
        assert_eq!(parse_datetime(b"1970-01-01T01:00:00+01:00"), at(0, 0));
        assert_eq!(parse_datetime(b"1970-01-01T00:00:00.5Z"), at(0, 500_000_000));
        assert_eq!(parse_datetime(b"2026-08-14T14:30:00"), None);
        assert_eq!(format_datetime(parse_datetime(b"2026-08-14T14:30:00.250+02:00").unwrap()), "2026-08-14T12:30:00.25Z");
        assert_eq!(Instant::from(Instant { seconds: -1, nanos: 5 }.at(9).unwrap(), 9), Instant { seconds: -1, nanos: 5 });
        assert!(is_id(&id_from(0xFF_FFFF_FFFF)));
        assert_eq!(&id_from(0), b"00000000");
        assert_eq!(parse_int(b"-9223372036854775808"), Some(i64::MIN));
        assert_eq!(parse_int(b"9223372036854775808"), None);
        assert!(is_url(b"https://example.com/a"));
        assert!(!is_url(b"example.com"));
        assert!(is_email(b"ada@example.com"));
        assert!(!is_email(b"ada@example"));
    }
}
