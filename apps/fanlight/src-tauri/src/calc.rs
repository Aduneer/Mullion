//! The calculator and the unit converter: "2^10 / 3", "sqrt(2) * pi", "15% * 80", "10 km in mi",
//! "72 f to c", "3 GB in MB". Worked out as you type, with nothing sent anywhere.

use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Answer {
    /// What it comes to, as it's copied: "6.21371".
    pub value: String,
    /// What the panel shows under it: "10 km in miles", or the expression tidied up.
    pub detail: String,
}

/// The answer to `text`, if it's a sum or a conversion. `comma_decimal` is set where Windows
/// writes 1,5 for one and a half, so a comma there is the decimal point rather than a separator.
pub fn answer(text: &str, comma_decimal: bool) -> Option<Answer> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    convert(text, comma_decimal).or_else(|| {
        let v = evaluate(text, comma_decimal)?;
        Some(Answer { value: format(v), detail: text.to_string() })
    })
}

/// A sum worth showing: anything that parses, other than a plain number.
pub fn evaluate(text: &str, comma_decimal: bool) -> Option<f64> {
    let mut p = Parser { s: text.chars().filter(|c| !c.is_whitespace()).collect(), i: 0, comma_decimal, sum: false };
    let v = p.expr()?;
    (p.i == p.s.len() && p.sum && v.is_finite()).then_some(v)
}

/// Ten significant figures at most, without trailing zeros; very large and very small numbers in
/// scientific notation.
pub fn format(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let a = v.abs();
    if !(1e-9..1e15).contains(&a) {
        let s = format!("{v:.9e}");
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let m = m.trim_end_matches('0').trim_end_matches('.');
        return format!("{m}e{e}");
    }
    let digits = (9 - a.log10().floor() as i32).clamp(0, 12) as usize;
    let s = format!("{v:.digits$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

struct Parser {
    s: Vec<char>,
    i: usize,
    comma_decimal: bool,
    /// Set once it's more than a number: an operator, a function or a constant.
    sum: bool,
}

type Function = fn(f64) -> f64;

const FUNCTIONS: &[(&str, Function)] = &[
    ("sqrt", f64::sqrt),
    ("cbrt", f64::cbrt),
    ("abs", f64::abs),
    ("round", f64::round),
    ("floor", f64::floor),
    ("ceil", f64::ceil),
    ("ln", f64::ln),
    ("log", f64::log10),
    ("log2", f64::log2),
    ("exp", f64::exp),
    ("sin", f64::sin),
    ("cos", f64::cos),
    ("tan", f64::tan),
    ("asin", f64::asin),
    ("acos", f64::acos),
    ("atan", f64::atan),
];

impl Parser {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        loop {
            if self.eat('+') {
                v += self.term()?;
            } else if self.eat('-') || self.eat('−') {
                v -= self.term()?;
            } else {
                return Some(v);
            }
            self.sum = true;
        }
    }

    fn term(&mut self) -> Option<f64> {
        let mut v = self.unary()?;
        loop {
            if self.eat('*') || self.eat('×') || self.eat('x') {
                v *= self.unary()?;
            } else if self.eat('/') || self.eat('÷') {
                v /= self.unary()?;
            } else if matches!(self.peek(), Some('(')) || self.peek().is_some_and(|c| c.is_alphabetic()) {
                // Side by side: "2pi", "3(4+1)".
                v *= self.unary()?;
            } else {
                return Some(v);
            }
            self.sum = true;
        }
    }

    fn unary(&mut self) -> Option<f64> {
        if self.eat('-') || self.eat('−') {
            return Some(-self.unary()?);
        }
        if self.eat('+') {
            return self.unary();
        }
        self.power()
    }

    fn power(&mut self) -> Option<f64> {
        let base = self.postfix()?;
        if self.eat('^') || (self.peek() == Some('*') && self.s.get(self.i + 1) == Some(&'*') && {
            self.i += 2;
            true
        }) {
            self.sum = true;
            // Right to left, and -2^2 is -(2^2), as on paper.
            return Some(base.powf(self.unary()?));
        }
        Some(base)
    }

    fn postfix(&mut self) -> Option<f64> {
        let mut v = self.primary()?;
        loop {
            if self.eat('%') {
                v /= 100.0;
            } else if self.eat('!') {
                if v < 0.0 || v.fract() != 0.0 || v > 170.0 {
                    return None;
                }
                v = (1..=v as u64).map(|n| n as f64).product();
            } else {
                return Some(v);
            }
            self.sum = true;
        }
    }

    fn primary(&mut self) -> Option<f64> {
        if self.eat('(') {
            let v = self.expr()?;
            self.eat(')'); // a missing last bracket is forgiven while typing
            return Some(v);
        }
        let c = self.peek()?;
        if c.is_ascii_digit() || c == '.' || (c == ',' && self.comma_decimal) {
            return self.number();
        }
        if c.is_alphabetic() || c == 'π' {
            let start = self.i;
            while self.peek().is_some_and(|c| c.is_alphanumeric() || c == 'π') {
                self.i += 1;
            }
            let word: String = self.s[start..self.i].iter().collect::<String>().to_lowercase();
            self.sum = true;
            return match word.as_str() {
                "pi" | "π" => Some(std::f64::consts::PI),
                "e" => Some(std::f64::consts::E),
                "tau" => Some(std::f64::consts::TAU),
                _ => {
                    let (_, f) = FUNCTIONS.iter().find(|(n, _)| *n == word)?;
                    let arg = if self.peek() == Some('(') { self.primary()? } else { self.power()? };
                    Some(f(arg))
                }
            };
        }
        None
    }

    fn number(&mut self) -> Option<f64> {
        let mut text = String::new();
        let mut seen_point = false;
        while let Some(c) = self.peek() {
            let point = c == '.' || (c == ',' && self.comma_decimal);
            if c.is_ascii_digit() {
                text.push(c);
            } else if point && !seen_point {
                seen_point = true;
                text.push('.');
            } else if (c == ',' || c == '\'' || c == '_') && !self.comma_decimal && self.s.get(self.i + 1).is_some_and(|d| d.is_ascii_digit()) {
                // A thousands separator: 1,000,000.
            } else if c == '.' && self.comma_decimal && self.s.get(self.i + 1).is_some_and(|d| d.is_ascii_digit()) {
                // 1.000.000 where the comma is the decimal point.
            } else {
                break;
            }
            self.i += 1;
        }
        // 1e6, 2.5e-3; but "2e" alone is two times e.
        if matches!(self.peek(), Some('e' | 'E')) {
            let mut j = self.i + 1;
            if matches!(self.s.get(j), Some('+' | '-')) {
                j += 1;
            }
            if self.s.get(j).is_some_and(|d| d.is_ascii_digit()) {
                text.push('e');
                text.extend(&self.s[self.i + 1..j]);
                self.i = j;
                while let Some(d) = self.peek().filter(|d| d.is_ascii_digit()) {
                    text.push(d);
                    self.i += 1;
                }
            }
        }
        text.parse().ok()
    }
}

// --- units -----------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Length,
    Mass,
    Volume,
    Area,
    Speed,
    Time,
    Data,
    Temperature,
}

/// A unit: what it measures, how many of the base unit one of it is, its names (the first is what
/// the answer calls it).
struct Unit {
    kind: Kind,
    factor: f64,
    names: &'static [&'static str],
}

const fn u(kind: Kind, factor: f64, names: &'static [&'static str]) -> Unit {
    Unit { kind, factor, names }
}

/// Data sizes count in 1024s, as File Explorer shows them.
const KI: f64 = 1024.0;
/// Each unit's names: its short one, then one and many of it, then others it's known by.
const UNITS: &[Unit] = &[
    u(Kind::Length, 0.001, &["mm", "millimeter", "millimeters", "millimetre", "millimetres"]),
    u(Kind::Length, 0.01, &["cm", "centimeter", "centimeters", "centimetre", "centimetres"]),
    u(Kind::Length, 1.0, &["m", "meter", "meters", "metre", "metres"]),
    u(Kind::Length, 1000.0, &["km", "kilometer", "kilometers", "kilometre", "kilometres"]),
    u(Kind::Length, 0.0254, &["in", "inch", "inches", "\""]),
    u(Kind::Length, 0.3048, &["ft", "foot", "feet", "'"]),
    u(Kind::Length, 0.9144, &["yd", "yard", "yards"]),
    u(Kind::Length, 1609.344, &["mi", "mile", "miles"]),
    u(Kind::Length, 1852.0, &["nmi", "nautical mile", "nautical miles"]),
    u(Kind::Mass, 1e-6, &["mg", "milligram", "milligrams"]),
    u(Kind::Mass, 0.001, &["g", "gram", "grams"]),
    u(Kind::Mass, 1.0, &["kg", "kilogram", "kilograms", "kilo", "kilos"]),
    u(Kind::Mass, 1000.0, &["t", "tonne", "tonnes", "ton", "tons"]),
    u(Kind::Mass, 0.028349523125, &["oz", "ounce", "ounces"]),
    u(Kind::Mass, 0.45359237, &["lb", "pound", "pounds", "lbs"]),
    u(Kind::Mass, 6.35029318, &["st", "stone", "stones"]),
    u(Kind::Volume, 0.001, &["ml", "milliliter", "milliliters", "millilitre", "millilitres"]),
    u(Kind::Volume, 0.01, &["cl", "centiliter", "centiliters", "centilitre", "centilitres"]),
    u(Kind::Volume, 0.1, &["dl", "deciliter", "deciliters", "decilitre", "decilitres"]),
    u(Kind::Volume, 1.0, &["l", "liter", "liters", "litre", "litres"]),
    u(Kind::Volume, 1000.0, &["m3", "cubic meter", "cubic meters", "m³", "cubic metre", "cubic metres"]),
    u(Kind::Volume, 0.00492892159375, &["tsp", "teaspoon", "teaspoons"]),
    u(Kind::Volume, 0.01478676478125, &["tbsp", "tablespoon", "tablespoons"]),
    u(Kind::Volume, 0.0295735295625, &["fl oz", "fluid ounce", "fluid ounces", "floz"]),
    u(Kind::Volume, 0.2365882365, &["cup", "cup", "cups"]),
    u(Kind::Volume, 0.473176473, &["pt", "pint", "pints"]),
    u(Kind::Volume, 0.946352946, &["qt", "quart", "quarts"]),
    u(Kind::Volume, 3.785411784, &["gal", "gallon", "gallons"]),
    u(Kind::Area, 1e-6, &["mm2", "square millimeter", "square millimeters", "mm²"]),
    u(Kind::Area, 1e-4, &["cm2", "square centimeter", "square centimeters", "cm²"]),
    u(Kind::Area, 1.0, &["m2", "square meter", "square meters", "m²", "square metre", "square metres", "sqm"]),
    u(Kind::Area, 1e4, &["ha", "hectare", "hectares"]),
    u(Kind::Area, 1e6, &["km2", "square kilometer", "square kilometers", "km²", "square kilometre", "square kilometres"]),
    u(Kind::Area, 0.00064516, &["in2", "square inch", "square inches", "in²"]),
    u(Kind::Area, 0.09290304, &["ft2", "square foot", "square feet", "ft²", "sqft"]),
    u(Kind::Area, 4046.8564224, &["ac", "acre", "acres"]),
    u(Kind::Area, 2589988.110336, &["mi2", "square mile", "square miles", "mi²"]),
    u(Kind::Speed, 1.0, &["m/s", "meter per second", "meters per second", "mps", "metres per second"]),
    u(Kind::Speed, 1.0 / 3.6, &["km/h", "kilometer per hour", "kilometers per hour", "kmh", "kph", "kmph", "kilometres per hour"]),
    u(Kind::Speed, 0.44704, &["mph", "mile per hour", "miles per hour"]),
    u(Kind::Speed, 1852.0 / 3600.0, &["kn", "knot", "knots", "kt"]),
    u(Kind::Speed, 0.3048, &["ft/s", "foot per second", "feet per second", "fps"]),
    u(Kind::Time, 0.001, &["ms", "millisecond", "milliseconds"]),
    u(Kind::Time, 1.0, &["s", "second", "seconds", "sec", "secs"]),
    u(Kind::Time, 60.0, &["min", "minute", "minutes", "mins"]),
    u(Kind::Time, 3600.0, &["h", "hour", "hours", "hr", "hrs"]),
    u(Kind::Time, 86400.0, &["d", "day", "days"]),
    u(Kind::Time, 604800.0, &["wk", "week", "weeks"]),
    u(Kind::Time, 2629800.0, &["mo", "month", "months"]),
    u(Kind::Time, 31557600.0, &["yr", "year", "years"]),
    u(Kind::Data, 0.125, &["bit", "bit", "bits"]),
    u(Kind::Data, 1.0, &["B", "byte", "bytes"]),
    u(Kind::Data, KI, &["KB", "kilobyte", "kilobytes", "kib"]),
    u(Kind::Data, KI * KI, &["MB", "megabyte", "megabytes", "mib"]),
    u(Kind::Data, KI * KI * KI, &["GB", "gigabyte", "gigabytes", "gib"]),
    u(Kind::Data, KI * KI * KI * KI, &["TB", "terabyte", "terabytes", "tib"]),
    u(Kind::Data, KI * KI * KI * KI * KI, &["PB", "petabyte", "petabytes", "pib"]),
    // Temperatures: the factor says which scale, see `to_kelvin`.
    u(Kind::Temperature, 0.0, &["°C", "celsius", "celsius", "c", "centigrade", "degc"]),
    u(Kind::Temperature, 1.0, &["°F", "fahrenheit", "fahrenheit", "f", "degf"]),
    u(Kind::Temperature, 2.0, &["K", "kelvin", "kelvins", "k"]),
];

fn unit(name: &str) -> Option<&'static Unit> {
    let name = name.trim().trim_start_matches("degrees ").trim_start_matches("degree ");
    let n = name.trim_start_matches('°').to_lowercase();
    UNITS.iter().find(|u| u.names.iter().any(|x| x.trim_start_matches('°').to_lowercase() == n))
}

fn to_kelvin(v: f64, u: &Unit) -> f64 {
    match u.factor as u8 {
        0 => v + 273.15,
        1 => (v - 32.0) * 5.0 / 9.0 + 273.15,
        _ => v,
    }
}

fn from_kelvin(k: f64, u: &Unit) -> f64 {
    match u.factor as u8 {
        0 => k - 273.15,
        1 => (k - 273.15) * 9.0 / 5.0 + 32.0,
        _ => k,
    }
}

/// "10 km in mi", "72f to c", "3.5 GB = MB", "(2+3) ft as m".
fn convert(text: &str, comma_decimal: bool) -> Option<Answer> {
    let lower = text.to_lowercase();
    let (from, to) = [" to ", " into ", " as ", " -> ", "=", " in "]
        .iter()
        .find_map(|sep| lower.rfind(sep).map(|i| (text[..i].trim(), text[i + sep.len()..].trim())))?;
    let to_unit = unit(to)?;
    // The number is everything up to where the unit starts; try the longest unit name first.
    let (value, from_unit) = (0..from.len())
        .rev()
        .filter(|&i| from.is_char_boundary(i))
        .find_map(|i| {
            let (num, name) = from.split_at(i);
            let u = unit(name)?;
            let num = num.trim();
            let v = if num.is_empty() { 1.0 } else { num_or_sum(num, comma_decimal)? };
            Some((v, u))
        })?;
    if from_unit.kind != to_unit.kind {
        return None;
    }
    let result = if from_unit.kind == Kind::Temperature {
        from_kelvin(to_kelvin(value, from_unit), to_unit)
    } else {
        value * from_unit.factor / to_unit.factor
    };
    // Temperatures by their symbol, the rest by name: "1 mile", "10 kilometers".
    let name = |u: &Unit, v: f64| match u.kind {
        Kind::Temperature => u.names[0],
        _ if v == 1.0 => u.names[1],
        _ => u.names[2],
    };
    Some(Answer {
        value: format(result),
        detail: format!("{} {} in {}", format(value), name(from_unit, value), name(to_unit, result)),
    })
}

fn num_or_sum(text: &str, comma_decimal: bool) -> Option<f64> {
    let mut p = Parser { s: text.chars().filter(|c| !c.is_whitespace()).collect(), i: 0, comma_decimal, sum: false };
    let v = p.expr()?;
    (p.i == p.s.len() && v.is_finite()).then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calc(s: &str) -> Option<String> {
        evaluate(s, false).map(format)
    }

    #[test]
    fn sums() {
        assert_eq!(calc("1+2*3").as_deref(), Some("7"));
        assert_eq!(calc("(1+2)*3").as_deref(), Some("9"));
        assert_eq!(calc("2^10").as_deref(), Some("1024"));
        assert_eq!(calc("2**3**2").as_deref(), Some("512"));
        assert_eq!(calc("-2^2").as_deref(), Some("-4"));
        assert_eq!(calc("10/4").as_deref(), Some("2.5"));
        assert_eq!(calc("1/3").as_deref(), Some("0.3333333333"));
        assert_eq!(calc("15% * 80").as_deref(), Some("12"));
        assert_eq!(calc("5!").as_deref(), Some("120"));
        assert_eq!(calc("sqrt(16) + abs(-2)").as_deref(), Some("6"));
        assert_eq!(calc("2pi").as_deref(), Some("6.283185307"));
        assert_eq!(calc("3(4+1)").as_deref(), Some("15"));
        assert_eq!(calc("6 × 7 ÷ 2").as_deref(), Some("21"));
        assert_eq!(calc("1,000 * 3").as_deref(), Some("3000"));
        assert_eq!(calc("1e3 + 1").as_deref(), Some("1001"));
        assert_eq!(calc("(2+3").as_deref(), Some("5"));
        assert_eq!(calc("0.1+0.2").as_deref(), Some("0.3"));
        assert_eq!(calc("2^100").as_deref(), Some("1.2676506e30"));
        assert_eq!(evaluate("1,5 * 2", true).map(format).as_deref(), Some("3"));
    }

    #[test]
    fn not_sums() {
        // A plain number, words, a division by zero, half a sum.
        for s in ["42", "3.5", "notepad", "hello world", "1/0", "2+", "*3", "foo(2)", "", "e", "pi "] {
            if s == "e" || s == "pi " {
                assert!(calc(s).is_some(), "{s} is a constant");
            } else {
                assert!(calc(s).is_none(), "{s} isn't a sum");
            }
        }
    }

    #[test]
    fn conversions() {
        let c = |s: &str| answer(s, false).map(|a| a.value);
        assert_eq!(c("10 km in mi").as_deref(), Some("6.213711922"));
        assert_eq!(c("1 mile to km").as_deref(), Some("1.609344"));
        assert_eq!(c("72f to c").as_deref(), Some("22.22222222"));
        assert_eq!(c("100 °C in °F").as_deref(), Some("212"));
        assert_eq!(c("0 k to c").as_deref(), Some("-273.15"));
        assert_eq!(c("3 GB in MB").as_deref(), Some("3072"));
        assert_eq!(c("2 h in min").as_deref(), Some("120"));
        assert_eq!(c("5 lbs = kg").as_deref(), Some("2.26796185"));
        assert_eq!(c("12 in in cm").as_deref(), Some("30.48"));
        assert_eq!(c("(2+3) ft as m").as_deref(), Some("1.524"));
        assert_eq!(c("100 km/h to mph").as_deref(), Some("62.13711922"));
        assert_eq!(answer("10 km in mi", false).unwrap().detail, "10 kilometers in miles");
        assert_eq!(answer("1 mile to km", false).unwrap().detail, "1 mile in kilometers");
        // Different things, unknown units, words that happen to contain "in".
        assert_eq!(c("10 km in kg"), None);
        assert_eq!(c("10 parsecs in m"), None);
        assert_eq!(c("settings in windows"), None);
    }

    #[test]
    fn the_readmes_examples() {
        let v = |t: &str| answer(t, false).map(|a| a.value);
        assert_eq!(v("(1200+340)*1.2").as_deref(), Some("1848"));
        assert_eq!(v("2^10").as_deref(), Some("1024"));
        assert_eq!(v("80 * 15%").as_deref(), Some("12"));
        assert_eq!(v("sqrt(2)").as_deref(), Some("1.414213562"));
        assert_eq!(v("5!").as_deref(), Some("120"));
        assert_eq!(v("10 km in mi").as_deref(), Some("6.213711922"));
        assert_eq!(v("72 f to c").as_deref(), Some("22.22222222"));
        assert!(v("3 cups in ml").is_some());
        assert_eq!(v("1.5 gb in mb").as_deref(), Some("1536"));
    }

    #[test]
    fn formatting() {
        assert_eq!(format(0.0), "0");
        assert_eq!(format(-0.0), "0");
        assert_eq!(format(1234567.0), "1234567");
        assert_eq!(format(0.000123), "0.000123");
        assert_eq!(format(1e-12), "1e-12");
        assert_eq!(format(-2.5), "-2.5");
    }
}
