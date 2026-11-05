//! Exact decimal arithmetic for computed columns (SPEC.md, the prelude): `+ - *` exact,
//! `/` rounded to 34 significant digits, half to even. Values that fit 38 digits stay in
//! `i128`; past that the magnitude is a base-10^9 big integer.

use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::value::{self, Decimal};

/// Significant digits a quotient keeps.
pub const QUOTIENT_DIGITS: u32 = 34;

const BASE: u64 = 1_000_000_000;

/// `±magnitude / 10^scale`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exact {
    pub negative: bool,
    /// Base 10^9, least significant first, no zero limb on top; empty is zero.
    pub magnitude: Vec<u32>,
    pub scale: u32,
}

fn trim(m: &mut Vec<u32>) {
    while m.last() == Some(&0) {
        m.pop();
    }
}

fn from_u128(mut v: u128) -> Vec<u32> {
    let mut m = Vec::new();
    while v != 0 {
        m.push((v % BASE as u128) as u32);
        v /= BASE as u128;
    }
    m
}

fn to_u128(m: &[u32]) -> Option<u128> {
    let mut v: u128 = 0;
    for &l in m.iter().rev() {
        v = v.checked_mul(BASE as u128)?.checked_add(l as u128)?;
    }
    Some(v)
}

fn cmp_mag(a: &[u32], b: &[u32]) -> Ordering {
    if a.len() != b.len() {
        return a.len().cmp(&b.len());
    }
    for k in (0..a.len()).rev() {
        if a[k] != b[k] {
            return a[k].cmp(&b[k]);
        }
    }
    Ordering::Equal
}

fn add_mag(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(a.len().max(b.len()) + 1);
    let mut carry = 0u64;
    for k in 0..a.len().max(b.len()) {
        let s = *a.get(k).unwrap_or(&0) as u64 + *b.get(k).unwrap_or(&0) as u64 + carry;
        out.push((s % BASE) as u32);
        carry = s / BASE;
    }
    if carry != 0 {
        out.push(carry as u32);
    }
    out
}

/// `a - b`, `a >= b`.
fn sub_mag(a: &[u32], b: &[u32]) -> Vec<u32> {
    debug_assert!(cmp_mag(a, b) != Ordering::Less);
    let mut out = Vec::with_capacity(a.len());
    let mut borrow = 0i64;
    for (k, &x) in a.iter().enumerate() {
        let mut d = x as i64 - borrow - *b.get(k).unwrap_or(&0) as i64;
        borrow = 0;
        if d < 0 {
            d += BASE as i64;
            borrow = 1;
        }
        out.push(d as u32);
    }
    trim(&mut out);
    out
}

fn mul_mag(a: &[u32], b: &[u32]) -> Vec<u32> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let mut out = alloc::vec![0u64; a.len() + b.len()];
    for (i, &x) in a.iter().enumerate() {
        let mut carry = 0u64;
        for (j, &y) in b.iter().enumerate() {
            let t = out[i + j] + x as u64 * y as u64 + carry;
            out[i + j] = t % BASE;
            carry = t / BASE;
        }
        let mut k = i + b.len();
        while carry != 0 {
            let t = out[k] + carry;
            out[k] = t % BASE;
            carry = t / BASE;
            k += 1;
        }
    }
    let mut m: Vec<u32> = out.into_iter().map(|l| l as u32).collect();
    trim(&mut m);
    m
}

fn mul_small(a: &[u32], f: u32) -> Vec<u32> {
    let mut out = Vec::with_capacity(a.len() + 1);
    let mut carry = 0u64;
    for &l in a {
        let t = l as u64 * f as u64 + carry;
        out.push((t % BASE) as u32);
        carry = t / BASE;
    }
    if carry != 0 {
        out.push(carry as u32);
    }
    trim(&mut out);
    out
}

fn mul_pow10(a: &[u32], mut k: u32) -> Vec<u32> {
    let mut m = a.to_vec();
    while k >= 9 {
        if !m.is_empty() {
            m.insert(0, 0);
        }
        k -= 9;
    }
    if k > 0 { mul_small(&m, 10u32.pow(k)) } else { m }
}

/// `a / d` and the remainder, `d < BASE`.
fn div_small(a: &[u32], d: u32) -> (Vec<u32>, u32) {
    let mut q = alloc::vec![0u32; a.len()];
    let mut r = 0u64;
    for k in (0..a.len()).rev() {
        let t = r * BASE + a[k] as u64;
        q[k] = (t / d as u64) as u32;
        r = t % d as u64;
    }
    trim(&mut q);
    (q, r as u32)
}

/// The quotient and whether a remainder is left (Knuth's algorithm D in base 10^9).
fn div_mag(u: &[u32], v: &[u32]) -> (Vec<u32>, bool) {
    debug_assert!(!v.is_empty());
    if cmp_mag(u, v) == Ordering::Less {
        return (Vec::new(), !u.is_empty());
    }
    if v.len() == 1 {
        let (q, r) = div_small(u, v[0]);
        return (q, r != 0);
    }
    let n = v.len();
    let m = u.len() - n;
    let f = (BASE / (v[n - 1] as u64 + 1)) as u32;
    let mut un = mul_small(u, f);
    un.resize(u.len() + 1, 0);
    let mut vn = mul_small(v, f);
    vn.resize(n, 0);
    let mut q = alloc::vec![0u32; m + 1];
    let (vt, vs) = (vn[n - 1] as u64, vn[n - 2] as u64);
    for j in (0..=m).rev() {
        let num = un[j + n] as u64 * BASE + un[j + n - 1] as u64;
        let mut qhat = num / vt;
        let mut rhat = num % vt;
        while qhat >= BASE || qhat * vs > rhat * BASE + un[j + n - 2] as u64 {
            qhat -= 1;
            rhat += vt;
            if rhat >= BASE {
                break;
            }
        }
        let mut carry = 0u64;
        let mut borrow = 0i64;
        for i in 0..n {
            let p = qhat * vn[i] as u64 + carry;
            carry = p / BASE;
            let mut t = un[i + j] as i64 - borrow - (p % BASE) as i64;
            borrow = 0;
            if t < 0 {
                t += BASE as i64;
                borrow = 1;
            }
            un[i + j] = t as u32;
        }
        let mut t = un[j + n] as i64 - borrow - carry as i64;
        if t < 0 {
            t += BASE as i64;
            un[j + n] = t as u32;
            // qhat was one too many: add the divisor back.
            qhat -= 1;
            let mut c = 0u64;
            for i in 0..n {
                let s = un[i + j] as u64 + vn[i] as u64 + c;
                un[i + j] = (s % BASE) as u32;
                c = s / BASE;
            }
            un[j + n] = ((un[j + n] as u64 + c) % BASE) as u32;
        } else {
            un[j + n] = t as u32;
        }
        q[j] = qhat as u32;
    }
    trim(&mut q);
    (q, un[..n].iter().any(|&l| l != 0))
}

fn digits(m: &[u32]) -> u32 {
    match m.last() {
        None => 0,
        Some(&top) => {
            let mut d = 1;
            let mut t = top / 10;
            while t != 0 {
                d += 1;
                t /= 10;
            }
            d + 9 * (m.len() as u32 - 1)
        }
    }
}

impl Exact {
    pub fn zero() -> Exact {
        Exact { negative: false, magnitude: Vec::new(), scale: 0 }
    }

    pub fn from_decimal(d: Decimal) -> Exact {
        Exact { negative: d.mantissa < 0, magnitude: from_u128(d.mantissa.unsigned_abs()), scale: d.scale as u32 }
    }

    pub fn from_int(v: i64) -> Exact {
        Exact { negative: v < 0, magnitude: from_u128(v.unsigned_abs() as u128), scale: 0 }
    }

    /// Digits written: `2`, `0.5`.
    pub fn parse(s: &str) -> Exact {
        // Called on the lexer's digits, with at most one point.
        let d = Decimal::parse(s.as_bytes());
        match d {
            Ok(d) => Exact::from_decimal(d),
            Err(_) => {
                let (int, frac) = s.split_once('.').unwrap_or((s, ""));
                let mut m = Vec::new();
                for c in int.bytes().chain(frac.bytes()) {
                    m = add_mag(&mul_small(&m, 10), &from_u128((c - b'0') as u128));
                }
                trim(&mut m);
                Exact { negative: false, magnitude: m, scale: frac.len() as u32 }
            }
        }
    }

    pub fn is_zero(&self) -> bool {
        self.magnitude.is_empty()
    }

    fn at_scale(&self, scale: u32) -> Vec<u32> {
        mul_pow10(&self.magnitude, scale - self.scale)
    }

    fn signed(negative: bool, magnitude: Vec<u32>, scale: u32) -> Exact {
        Exact { negative: negative && !magnitude.is_empty(), magnitude, scale }
    }

    pub fn add(&self, b: &Exact) -> Exact {
        let s = self.scale.max(b.scale);
        let (x, y) = (self.at_scale(s), b.at_scale(s));
        if self.negative == b.negative {
            return Exact::signed(self.negative, add_mag(&x, &y), s);
        }
        match cmp_mag(&x, &y) {
            Ordering::Less => Exact::signed(b.negative, sub_mag(&y, &x), s),
            _ => Exact::signed(self.negative, sub_mag(&x, &y), s),
        }
    }

    pub fn neg(&self) -> Exact {
        Exact::signed(!self.negative, self.magnitude.clone(), self.scale)
    }

    pub fn sub(&self, b: &Exact) -> Exact {
        self.add(&b.neg())
    }

    pub fn mul(&self, b: &Exact) -> Exact {
        Exact::signed(self.negative != b.negative, mul_mag(&self.magnitude, &b.magnitude), self.scale + b.scale)
    }

    /// `self / b` to 34 significant digits, half to even; nothing for a zero divisor.
    pub fn div(&self, b: &Exact) -> Option<Exact> {
        if b.is_zero() {
            return None;
        }
        if self.is_zero() {
            return Some(Exact::zero());
        }
        let (da, db) = (digits(&self.magnitude), digits(&b.magnitude));
        // A * 10^k / B then has at least 35 digits, one past what is kept.
        let k = (QUOTIENT_DIGITS + 1 + db).saturating_sub(da);
        let (q, sticky) = match (to_u128(&self.magnitude), to_u128(&b.magnitude)) {
            (Some(a), Some(bb)) if k <= 38 && a.checked_mul(value::pow10(k) as u128).is_some() => {
                let a = a * value::pow10(k) as u128;
                (from_u128(a / bb), !a.is_multiple_of(bb))
            }
            _ => div_mag(&mul_pow10(&self.magnitude, k), &b.magnitude),
        };
        let dq = digits(&q);
        let drop = dq.saturating_sub(QUOTIENT_DIGITS);
        let (mut kept, rest) = (q.clone(), q);
        let mut half_cmp = Ordering::Less;
        if drop > 0 {
            let mut d = drop;
            let mut rem_digits: Vec<u32> = Vec::new();
            let mut place: Vec<u32> = from_u128(1);
            let mut r = rest;
            // Peel `drop` digits off the bottom, keeping them as a number.
            while d > 0 {
                let step = d.min(9);
                let (q2, r2) = div_small(&r, 10u32.pow(step));
                rem_digits = add_mag(&rem_digits, &mul_mag(&from_u128(r2 as u128), &place));
                place = mul_pow10(&place, step);
                r = q2;
                d -= step;
            }
            kept = r;
            let half = mul_small(&place, 5);
            let half = div_small(&half, 10).0;
            half_cmp = cmp_mag(&rem_digits, &half);
        }
        let round_up = match half_cmp {
            Ordering::Greater => true,
            Ordering::Equal => sticky || kept.first().is_some_and(|l| l % 2 == 1),
            Ordering::Less => false,
        };
        if round_up {
            kept = add_mag(&kept, &from_u128(1));
        }
        let mut scale = self.scale as i64 - b.scale as i64 + k as i64 - drop as i64;
        if scale < 0 {
            kept = mul_pow10(&kept, (-scale) as u32);
            scale = 0;
        }
        let mut out = Exact::signed(self.negative != b.negative, kept, scale as u32);
        out.normalize();
        Some(out)
    }

    /// No zero after the point that changes nothing.
    pub fn normalize(&mut self) {
        while self.scale > 0 && !self.magnitude.is_empty() {
            let (q, r) = div_small(&self.magnitude, 10);
            if r != 0 {
                break;
            }
            self.magnitude = q;
            self.scale -= 1;
        }
        if self.magnitude.is_empty() {
            self.scale = 0;
        }
    }

    /// The order of two values, whatever their scales (`1.0` and `1.00` are in no order,
    /// unlike `==`, which compares how each is held).
    pub fn order(&self, b: &Exact) -> Ordering {
        match (self.is_zero() || !self.negative, b.is_zero() || !b.negative) {
            (true, false) => return Ordering::Greater,
            (false, true) => return Ordering::Less,
            _ => {}
        }
        let s = self.scale.max(b.scale);
        let o = cmp_mag(&self.at_scale(s), &b.at_scale(s));
        if self.negative && !self.is_zero() { o.reverse() } else { o }
    }

    /// Rounded half to even to `scale` digits after the point (none fewer than it has).
    pub fn round(&self, scale: u32) -> Exact {
        if scale >= self.scale {
            return self.clone();
        }
        let mut d = self.scale - scale;
        let mut r = self.magnitude.clone();
        let mut last = 0u32;
        let mut sticky = false;
        while d > 0 {
            let (q, rem) = div_small(&r, 10);
            sticky |= d > 1 && rem != 0;
            if d == 1 {
                last = rem;
            }
            r = q;
            d -= 1;
        }
        let up = last > 5 || (last == 5 && (sticky || r.first().is_some_and(|l| l % 2 == 1)));
        let m = if up { add_mag(&r, &from_u128(1)) } else { r };
        Exact::signed(self.negative, m, scale)
    }

    /// At exactly `scale` digits after the point: rounded half to even to fewer, zeros
    /// written to more (`84.2` at 2 is `84.20`).
    pub fn rescale(&self, scale: u32) -> Exact {
        if scale < self.scale {
            return self.round(scale);
        }
        Exact::signed(self.negative, self.at_scale(scale), scale)
    }

    /// Rounded toward negative infinity to a whole number.
    pub fn floor(&self) -> Exact {
        let t = self.trunc();
        if self.negative && t.order(self) != Ordering::Equal { t.sub(&Exact::from_int(1)) } else { t }
    }

    /// The whole part, toward zero.
    pub fn trunc(&self) -> Exact {
        let mut r = self.magnitude.clone();
        for _ in 0..self.scale {
            r = div_small(&r, 10).0;
        }
        trim(&mut r);
        Exact::signed(self.negative, r, 0)
    }

    pub fn abs(&self) -> Exact {
        Exact::signed(false, self.magnitude.clone(), self.scale)
    }

    /// Digits before the point.
    pub fn int_digits(&self) -> u32 {
        digits(&self.magnitude).saturating_sub(self.scale)
    }

    /// The mantissa at `scale`, rounded half to even when the value has more digits after
    /// the point; nothing past 38 digits.
    pub fn to_i128(&self, scale: u32) -> Option<i128> {
        let m = if scale >= self.scale {
            mul_pow10(&self.magnitude, scale - self.scale)
        } else {
            let mut d = self.scale - scale;
            let mut r = self.magnitude.clone();
            let mut last = 0u32;
            let mut sticky = false;
            while d > 0 {
                let (q, rem) = div_small(&r, 10);
                sticky |= d > 1 && rem != 0;
                if d == 1 {
                    last = rem;
                }
                r = q;
                d -= 1;
            }
            let up = last > 5 || (last == 5 && (sticky || r.first().is_some_and(|l| l % 2 == 1)));
            if up { add_mag(&r, &from_u128(1)) } else { r }
        };
        let v = to_u128(&m)?;
        if v >= value::pow10(value::PRECISION) as u128 {
            return None;
        }
        Some(if self.negative { -(v as i128) } else { v as i128 })
    }
}

impl core::fmt::Display for Exact {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut s = alloc::string::String::new();
        for (k, l) in self.magnitude.iter().rev().enumerate() {
            if k == 0 {
                s.push_str(&alloc::format!("{l}"));
            } else {
                s.push_str(&alloc::format!("{l:09}"));
            }
        }
        if s.is_empty() {
            s.push('0');
        }
        let scale = self.scale as usize;
        if scale > 0 {
            while s.len() <= scale {
                s.insert(0, '0');
            }
            s.insert(s.len() - scale, '.');
        }
        if self.negative {
            s.insert(0, '-');
        }
        f.write_str(&s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    fn e(s: &str) -> Exact {
        let neg = s.starts_with('-');
        let x = Exact::parse(s.trim_start_matches('-'));
        if neg { x.neg() } else { x }
    }

    #[test]
    fn arithmetic() {
        assert_eq!(e("0.1").add(&e("0.2")).to_string(), "0.3");
        assert_eq!(e("1").div(&e("3")).unwrap().to_string(), "0.3333333333333333333333333333333333");
        assert_eq!(e("2").div(&e("3")).unwrap().to_string(), "0.6666666666666666666666666666666667");
        assert_eq!(e("-7").div(&e("3")).unwrap().to_string(), "-2.333333333333333333333333333333333");
        assert_eq!(e("10").div(&e("4")).unwrap().to_string(), "2.5");
        assert_eq!(e("12345678901234567890123456789012345").div(&e("2")).unwrap().to_string(), "6172839450617283945061728394506172");
        assert_eq!(e("12345678901234567890123456789012347").div(&e("2")).unwrap().to_string(), "6172839450617283945061728394506174");
        let vat = e("64.30").mul(&e("21")).div(&e("100").add(&e("21"))).unwrap();
        assert_eq!(vat.to_string(), "11.15950413223140495867768595041322");
        assert_eq!(e("1.10").mul(&e("3")).to_string(), "3.30");
        assert!(e("1").div(&e("0")).is_none());
        // Past u128: the big path.
        let big = e("123456789012345678901234567890123456789").mul(&e("987654321098765432109876543210"));
        let q = big.div(&e("987654321098765432109876543210")).unwrap();
        assert_eq!(q.to_string(), "123456789012345678901234567890123500000");
        assert_eq!(e("1").div(&e("7")).unwrap().to_i128(3), Some(143));
        assert_eq!(e("2.5").to_i128(0), Some(2));
        assert_eq!(e("3.5").to_i128(0), Some(4));
    }
}
