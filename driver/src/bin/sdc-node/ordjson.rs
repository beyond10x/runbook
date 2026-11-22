//! JSON the way `jq` keeps it: object keys in the order the text gave them (serde_json without
//! `preserve_order` sorts them), and number text for the caps a binding states.

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Error as _, Serialize, SerializeMap, SerializeSeq, Serializer};
use serde_json::value::RawValue;
use std::fmt;

/// A JSON value with its object keys in text order.
#[derive(Clone, Debug, PartialEq)]
pub enum Ov {
    /// `null`
    Null,
    /// `true` or `false`
    Bool(bool),
    /// A number
    Num(serde_json::Number),
    /// A number given as text, printed as it stands
    Lit(NumLit),
    /// A string
    Str(String),
    /// An array
    Arr(Vec<Ov>),
    /// An object; a repeated key keeps its first place and its last value
    Obj(Vec<(String, Ov)>),
}

impl Ov {
    /// The value of a JSON text; `Err` with the parser's reason.
    pub fn parse(text: &str) -> Result<Ov, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    /// The member `key` of an object.
    pub fn get(&self, key: &str) -> Option<&Ov> {
        match self {
            Ov::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// jq's truth: everything but `null` and `false`.
    pub fn truthy(&self) -> bool {
        !matches!(self, Ov::Null | Ov::Bool(false))
    }

    /// The member `key` when it is present and truthy (`.key // empty`).
    pub fn field(&self, key: &str) -> Option<&Ov> {
        self.get(key).filter(|v| v.truthy())
    }

    /// The member `key` as text when it is a string.
    pub fn str_of(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(Ov::Str(s)) => Some(s),
            _ => None,
        }
    }

    /// The members of an array, or the values of an object (`.[]`).
    pub fn items(&self) -> Vec<&Ov> {
        match self {
            Ov::Arr(a) => a.iter().collect(),
            Ov::Obj(m) => m.iter().map(|(_, v)| v).collect(),
            _ => Vec::new(),
        }
    }

    /// `jq -c`: compact, DEL escaped as jq does.
    pub fn compact(&self) -> String {
        serde_json::to_string(self).unwrap_or_default().replace('\u{7f}', "\\u007f")
    }

    /// `jq -r` of a value: a string without its quotes, anything else compact.
    pub fn raw(&self) -> String {
        match self {
            Ov::Str(s) => s.clone(),
            other => other.compact(),
        }
    }
}

impl Serialize for Ov {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Ov::Null => s.serialize_unit(),
            Ov::Bool(b) => s.serialize_bool(*b),
            Ov::Num(n) => n.serialize(s),
            Ov::Lit(n) => n.serialize(s),
            Ov::Str(t) => s.serialize_str(t),
            Ov::Arr(a) => {
                let mut q = s.serialize_seq(Some(a.len()))?;
                for v in a {
                    q.serialize_element(v)?;
                }
                q.end()
            }
            Ov::Obj(m) => {
                let mut q = s.serialize_map(Some(m.len()))?;
                for (k, v) in m {
                    q.serialize_entry(k, v)?;
                }
                q.end()
            }
        }
    }
}

struct OvVisitor;

impl<'de> Visitor<'de> for OvVisitor {
    type Value = Ov;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON value")
    }
    fn visit_unit<E>(self) -> Result<Ov, E> {
        Ok(Ov::Null)
    }
    fn visit_none<E>(self) -> Result<Ov, E> {
        Ok(Ov::Null)
    }
    fn visit_bool<E>(self, b: bool) -> Result<Ov, E> {
        Ok(Ov::Bool(b))
    }
    fn visit_i64<E>(self, n: i64) -> Result<Ov, E> {
        Ok(Ov::Num(n.into()))
    }
    fn visit_u64<E>(self, n: u64) -> Result<Ov, E> {
        Ok(Ov::Num(n.into()))
    }
    fn visit_f64<E>(self, n: f64) -> Result<Ov, E> {
        Ok(serde_json::Number::from_f64(n).map_or(Ov::Null, Ov::Num))
    }
    fn visit_str<E>(self, s: &str) -> Result<Ov, E> {
        Ok(Ov::Str(s.to_string()))
    }
    fn visit_string<E>(self, s: String) -> Result<Ov, E> {
        Ok(Ov::Str(s))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Ov, A::Error> {
        let mut v = Vec::new();
        while let Some(x) = a.next_element()? {
            v.push(x);
        }
        Ok(Ov::Arr(v))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Ov, A::Error> {
        let mut m: Vec<(String, Ov)> = Vec::new();
        while let Some((k, v)) = a.next_entry::<String, Ov>()? {
            match m.iter_mut().find(|(have, _)| *have == k) {
                Some(slot) => slot.1 = v,
                None => m.push((k, v)),
            }
        }
        Ok(Ov::Obj(m))
    }
}

impl<'de> Deserialize<'de> for Ov {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Ov, D::Error> {
        d.deserialize_any(OvVisitor)
    }
}

/// A number written out as the text `jq` prints for `tonumber` of the argument: a plain decimal
/// keeps its digits (`0.50` stays `0.50`), leading zeros go.
#[derive(Clone, Debug, PartialEq)]
pub struct NumLit(pub String);

impl NumLit {
    /// `$v | tonumber`; `None` where jq refuses the text.
    pub fn parse(v: &str) -> Option<NumLit> {
        let plain = {
            let (sign, rest) = v.strip_prefix('-').map_or(("", v), |r| ("-", r));
            let (int, frac) = rest.split_once('.').map_or((rest, None), |(i, f)| (i, Some(f)));
            let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
            (digits(int) && frac.is_none_or(digits)).then(|| {
                let trimmed = int.trim_start_matches('0');
                let int = if trimmed.is_empty() { "0" } else { trimmed };
                match frac {
                    Some(f) => format!("{sign}{int}.{f}"),
                    None => format!("{sign}{int}"),
                }
            })
        };
        if let Some(p) = plain {
            return Some(NumLit(p));
        }
        let f: f64 = v.trim().parse().ok().filter(|f: &f64| f.is_finite())?;
        if v.trim() != v {
            return None;
        }
        Some(NumLit(serde_json::Number::from_f64(f)?.to_string()))
    }
}

impl Serialize for NumLit {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        RawValue::from_string(self.0.clone()).map_err(S::Error::custom)?.serialize(s)
    }
}
