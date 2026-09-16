//! Fortran namelist input -- `&group key = values ... /`.
//!
//! Used for `namelist.input` (the BMI `init_config`) and `MPTABLE.TBL`.
//!
//! Semantics that matter for bit-identity and for matching gfortran's behaviour:
//!
//! - A group runs from `&name` to a terminating `/` (or `&end`).
//! - Keys absent from the file leave their target **untouched**, which is how the `-1.E36`
//!   sentinels in `ParametersRead` survive to flag unset parameters.
//! - Values are comma and/or whitespace separated and may span lines.
//! - `n*value` repeats.
//! - `!` starts a comment.
//! - Array element and slice syntax (`a(3) = ...`, `a(2:4) = ...`) sets a sub-range.

use super::value::{parse_int, parse_logical, parse_real32, parse_string, ParseError};
use std::collections::HashMap;
use core::fmt;

#[derive(Debug)]
pub enum NamelistError {
    UnterminatedGroup(String),
    MalformedEntry { group: String, line: usize },
    Value { group: String, key: String, source: ParseError },
}

impl fmt::Display for NamelistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NamelistError::UnterminatedGroup(g) => write!(f, "namelist group &{g} is not terminated by '/'"),
            NamelistError::MalformedEntry { group, line } => {
                write!(f, "malformed entry in &{group} at line {line}")
            }
            NamelistError::Value { group, key, source } => {
                write!(f, "&{group} {key}: {source}")
            }
        }
    }
}

impl std::error::Error for NamelistError {}

/// One `key = values` entry, with the raw value tokens kept in file order.
#[derive(Debug, Clone, Default)]
pub struct Entry {
    /// Tokens as they appeared, with `n*v` already expanded.
    pub tokens: Vec<String>,
    /// Lower bound from `key(lo)` or `key(lo:hi)`, if the entry targeted a sub-range.
    pub start_index: Option<i32>,
}

/// A parsed namelist group: keys lowercased, values unparsed.
#[derive(Debug, Clone, Default)]
pub struct Group {
    pub name: String,
    entries: HashMap<String, Entry>,
}

impl Group {
    pub fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(&key.to_ascii_lowercase())
    }

    pub fn entry(&self, key: &str) -> Option<&Entry> {
        self.entries.get(&key.to_ascii_lowercase())
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }

    /// Assign into `target` if the key is present; otherwise leave it untouched.
    ///
    /// This is the behaviour the `-1.E36` sentinels depend on -- an absent key must not
    /// overwrite the sentinel with a default.
    pub fn get_real(&self, key: &str, target: &mut f32) -> Result<(), NamelistError> {
        if let Some(e) = self.entry(key) {
            if let Some(tok) = e.tokens.first() {
                *target = parse_real32(tok).map_err(|s| self.err(key, s))?;
            }
        }
        Ok(())
    }

    pub fn get_int(&self, key: &str, target: &mut i32) -> Result<(), NamelistError> {
        if let Some(e) = self.entry(key) {
            if let Some(tok) = e.tokens.first() {
                *target = parse_int(tok).map_err(|s| self.err(key, s))?;
            }
        }
        Ok(())
    }

    pub fn get_logical(&self, key: &str, target: &mut bool) -> Result<(), NamelistError> {
        if let Some(e) = self.entry(key) {
            if let Some(tok) = e.tokens.first() {
                *target = parse_logical(tok).map_err(|s| self.err(key, s))?;
            }
        }
        Ok(())
    }

    pub fn get_string(&self, key: &str, target: &mut String) -> Result<(), NamelistError> {
        if let Some(e) = self.entry(key) {
            if let Some(tok) = e.tokens.first() {
                *target = parse_string(tok);
            }
        }
        Ok(())
    }

    /// Fill a 1-based array. Only as many elements as the file supplies are written; the rest
    /// keep their prior values. Honours `key(n) = ...` offsets.
    pub fn get_real_array(&self, key: &str, target: &mut [f32]) -> Result<(), NamelistError> {
        if let Some(e) = self.entry(key) {
            let start = (e.start_index.unwrap_or(1) - 1).max(0) as usize;
            for (i, tok) in e.tokens.iter().enumerate() {
                match target.get_mut(start + i) {
                    Some(slot) => *slot = parse_real32(tok).map_err(|s| self.err(key, s))?,
                    None => break, // extra values are ignored, as gfortran would error -- see note
                }
            }
        }
        Ok(())
    }

    pub fn get_int_array(&self, key: &str, target: &mut [i32]) -> Result<(), NamelistError> {
        if let Some(e) = self.entry(key) {
            let start = (e.start_index.unwrap_or(1) - 1).max(0) as usize;
            for (i, tok) in e.tokens.iter().enumerate() {
                match target.get_mut(start + i) {
                    Some(slot) => *slot = parse_int(tok).map_err(|s| self.err(key, s))?,
                    None => break,
                }
            }
        }
        Ok(())
    }

    /// Fill a 2-D array stored column-major, as Fortran declares `SAIM_TABLE(MVT,12)`.
    ///
    /// Namelist values for such an array arrive in Fortran storage order: the first index
    /// varies fastest.
    pub fn get_real_array_2d(
        &self,
        key: &str,
        target: &mut [f32],
        _n_rows: usize,
    ) -> Result<(), NamelistError> {
        self.get_real_array(key, target)
    }

    fn err(&self, key: &str, source: ParseError) -> NamelistError {
        NamelistError::Value {
            group: self.name.clone(),
            key: key.to_string(),
            source,
        }
    }
}

/// All groups found in a namelist file, keyed by lowercased group name.
#[derive(Debug, Clone, Default)]
pub struct Namelist {
    groups: HashMap<String, Group>,
}

impl Namelist {
    pub fn parse(contents: &str) -> Result<Self, NamelistError> {
        let mut groups = HashMap::new();
        let mut chars = Scanner::new(contents);

        while let Some(name) = chars.seek_group_header()? {
            let group = chars.parse_group(&name)?;
            groups.insert(name.to_ascii_lowercase(), group);
        }

        Ok(Self { groups })
    }

    pub fn group(&self, name: &str) -> Option<&Group> {
        self.groups.get(&name.to_ascii_lowercase())
    }

    /// An empty group stands in for one that is absent, so every key falls back to its prior
    /// value -- matching a Fortran read of an optional namelist group.
    pub fn group_or_empty(&self, name: &str) -> Group {
        self.group(name).cloned().unwrap_or_else(|| Group {
            name: name.to_string(),
            entries: HashMap::new(),
        })
    }

    pub fn group_names(&self) -> impl Iterator<Item = &String> {
        self.groups.keys()
    }

    pub fn has_group(&self, name: &str) -> bool {
        self.groups.contains_key(&name.to_ascii_lowercase())
    }
}

/// Character scanner over the whole file, tracking line numbers for errors.
struct Scanner {
    chars: Vec<char>,
    pos: usize,
    line: usize,
}

impl Scanner {
    fn new(s: &str) -> Self {
        Self { chars: s.chars().collect(), pos: 0, line: 1 }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c == Some('\n') {
            self.line += 1;
        }
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_to_end_of_line(&mut self) {
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.bump();
        }
    }

    /// Skip whitespace and `!` comments.
    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                Some('!') => self.skip_to_end_of_line(),
                _ => return,
            }
        }
    }

    /// Advance to the next `&group` / `$group` header and consume it.
    fn seek_group_header(&mut self) -> Result<Option<String>, NamelistError> {
        loop {
            self.skip_trivia();
            match self.peek() {
                None => return Ok(None),
                Some('&') | Some('$') => {
                    self.bump();
                    let mut name = String::new();
                    while let Some(c) = self.peek() {
                        if c.is_alphanumeric() || c == '_' {
                            name.push(c);
                            self.bump();
                        } else {
                            break;
                        }
                    }
                    if name.is_empty() {
                        continue;
                    }
                    return Ok(Some(name));
                }
                // Text outside any group is ignored, as Fortran does.
                Some(_) => {
                    self.skip_to_end_of_line();
                }
            }
        }
    }

    fn parse_group(&mut self, name: &str) -> Result<Group, NamelistError> {
        let mut entries: HashMap<String, Entry> = HashMap::new();
        let mut current: Option<String> = None;

        loop {
            self.skip_trivia();

            let c = match self.peek() {
                None => return Err(NamelistError::UnterminatedGroup(name.to_string())),
                Some(c) => c,
            };

            // Group terminator.
            if c == '/' {
                self.bump();
                break;
            }
            if c == '&' || c == '$' {
                // `&end` / `$end`, or an unterminated group running into the next one.
                let save = self.pos;
                self.bump();
                let mut word = String::new();
                while let Some(ch) = self.peek() {
                    if ch.is_alphanumeric() {
                        word.push(ch);
                        self.bump();
                    } else {
                        break;
                    }
                }
                if word.eq_ignore_ascii_case("end") {
                    break;
                }
                self.pos = save;
                return Err(NamelistError::UnterminatedGroup(name.to_string()));
            }

            if c == ',' {
                self.bump();
                continue;
            }

            // Either `key = ...` or another value for the current key.
            let start_line = self.line;
            let token = self.read_token();
            if token.is_empty() {
                self.bump();
                continue;
            }

            self.skip_trivia();
            if self.peek() == Some('=') {
                self.bump();
                let (key, start_index) = split_index(&token);
                let key = key.to_ascii_lowercase();
                entries.insert(key.clone(), Entry { tokens: Vec::new(), start_index });
                current = Some(key);
            } else {
                match &current {
                    Some(key) => {
                        let entry = entries.get_mut(key).expect("current key exists");
                        push_value(&mut entry.tokens, &token);
                    }
                    None => {
                        return Err(NamelistError::MalformedEntry {
                            group: name.to_string(),
                            line: start_line,
                        })
                    }
                }
            }
        }

        Ok(Group { name: name.to_string(), entries })
    }

    /// Read one token: a quoted string, or a run up to whitespace/`,`/`=`/`!`/`/`.
    fn read_token(&mut self) -> String {
        let mut out = String::new();

        match self.peek() {
            Some(q @ ('\'' | '"')) => {
                out.push(q);
                self.bump();
                while let Some(c) = self.bump() {
                    out.push(c);
                    if c == q {
                        if self.peek() == Some(q) {
                            out.push(q);
                            self.bump();
                            continue;
                        }
                        break;
                    }
                }
            }
            _ => {
                let mut depth = 0usize;
                while let Some(c) = self.peek() {
                    if c == '(' {
                        depth += 1;
                    } else if c == ')' {
                        depth = depth.saturating_sub(1);
                    } else if depth == 0
                        && (c.is_whitespace() || c == ',' || c == '=' || c == '!' || c == '/')
                    {
                        break;
                    }
                    out.push(c);
                    self.bump();
                }
            }
        }

        out
    }
}

/// Split `name(3)` or `name(2:4)` into the bare name and its lower bound.
fn split_index(token: &str) -> (String, Option<i32>) {
    match (token.find('('), token.rfind(')')) {
        (Some(open), Some(close)) if close > open => {
            let name = token[..open].to_string();
            let inner = &token[open + 1..close];
            // For a multi-dimensional or sliced reference take the first subscript.
            let first = inner.split(',').next().unwrap_or("").split(':').next().unwrap_or("");
            (name, first.trim().parse::<i32>().ok())
        }
        _ => (token.to_string(), None),
    }
}

/// Push a value token, expanding `n*value`.
fn push_value(out: &mut Vec<String>, token: &str) {
    if !token.starts_with('\'') && !token.starts_with('"') {
        if let Some((count, rest)) = token.split_once('*') {
            if let Ok(n) = count.parse::<usize>() {
                if !rest.is_empty() {
                    for _ in 0..n {
                        out.push(rest.to_string());
                    }
                    return;
                }
            }
        }
    }
    out.push(token.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_timing_group() {
        let src = r#"
&timing
  dt                 = 1800.0
  startdate          = "199801010630"
  enddate            = "199901010630"
  forcing_filename   = "../data/bondville.dat"
/
"#;
        let nml = Namelist::parse(src).unwrap();
        let g = nml.group("timing").unwrap();

        let mut dt = 0.0f32;
        g.get_real("dt", &mut dt).unwrap();
        assert_eq!(dt, 1800.0);

        let mut start = String::new();
        g.get_string("startdate", &mut start).unwrap();
        assert_eq!(start, "199801010630");
    }

    #[test]
    fn comments_are_stripped() {
        let src = "&location\n  lat = 40.01   ! latitude [degrees]  (-90 to 90)\n/\n";
        let nml = Namelist::parse(src).unwrap();
        let mut lat = 0.0f32;
        nml.group("location").unwrap().get_real("lat", &mut lat).unwrap();
        assert_eq!(lat, 40.01f32);
    }

    #[test]
    fn absent_key_leaves_target_untouched() {
        // The sentinel behaviour ParametersRead depends on.
        let nml = Namelist::parse("&g\n a = 1.0\n/\n").unwrap();
        let g = nml.group("g").unwrap();
        let mut b = -1.0E36f32;
        g.get_real("b", &mut b).unwrap();
        assert_eq!(b, -1.0E36f32);
    }

    #[test]
    fn absent_group_leaves_everything_untouched() {
        let nml = Namelist::parse("&g\n a = 1.0\n/\n").unwrap();
        let g = nml.group_or_empty("nope");
        let mut v = 42.0f32;
        g.get_real("a", &mut v).unwrap();
        assert_eq!(v, 42.0);
    }

    #[test]
    fn arrays_span_lines_and_commas() {
        // The initial_values group from run/namelist.input.
        let src = "&initial_values\n dzsnso = 0.0,  0.0,  0.0,  0.1,  0.3,  0.6,  1.0\n \
                   sh2o   =  0.3,  0.3,  0.3,  0.3\n zwt = -2.0\n/\n";
        let nml = Namelist::parse(src).unwrap();
        let g = nml.group("initial_values").unwrap();

        let mut dzsnso = [0.0f32; 7];
        g.get_real_array("dzsnso", &mut dzsnso).unwrap();
        assert_eq!(dzsnso, [0.0, 0.0, 0.0, 0.1, 0.3, 0.6, 1.0]);

        let mut zwt = 0.0f32;
        g.get_real("zwt", &mut zwt).unwrap();
        assert_eq!(zwt, -2.0);
    }

    #[test]
    fn partial_array_keeps_the_tail() {
        let nml = Namelist::parse("&g\n a = 1.0, 2.0\n/\n").unwrap();
        let mut a = [9.0f32; 4];
        nml.group("g").unwrap().get_real_array("a", &mut a).unwrap();
        assert_eq!(a, [1.0, 2.0, 9.0, 9.0]);
    }

    #[test]
    fn repeat_syntax() {
        let nml = Namelist::parse("&g\n a = 3*1.5, 2.0\n/\n").unwrap();
        let mut a = [0.0f32; 4];
        nml.group("g").unwrap().get_real_array("a", &mut a).unwrap();
        assert_eq!(a, [1.5, 1.5, 1.5, 2.0]);
    }

    #[test]
    fn indexed_assignment_offsets() {
        let nml = Namelist::parse("&g\n a(3) = 7.0, 8.0\n/\n").unwrap();
        let mut a = [0.0f32; 5];
        nml.group("g").unwrap().get_real_array("a", &mut a).unwrap();
        assert_eq!(a, [0.0, 0.0, 7.0, 8.0, 0.0]);
    }

    #[test]
    fn sliced_assignment_uses_lower_bound() {
        let nml = Namelist::parse("&g\n a(2:3) = 7.0, 8.0\n/\n").unwrap();
        let mut a = [0.0f32; 4];
        nml.group("g").unwrap().get_real_array("a", &mut a).unwrap();
        assert_eq!(a, [0.0, 7.0, 8.0, 0.0]);
    }

    #[test]
    fn multiple_groups() {
        let src = "&a\n x = 1\n/\n&b\n y = 2\n/\n";
        let nml = Namelist::parse(src).unwrap();
        assert!(nml.has_group("a"));
        assert!(nml.has_group("b"));
        let mut y = 0;
        nml.group("b").unwrap().get_int("y", &mut y).unwrap();
        assert_eq!(y, 2);
    }

    #[test]
    fn group_names_are_case_insensitive() {
        let nml = Namelist::parse("&Timing\n DT = 1800.0\n/\n").unwrap();
        let mut dt = 0.0f32;
        nml.group("timing").unwrap().get_real("dt", &mut dt).unwrap();
        assert_eq!(dt, 1800.0);
    }

    #[test]
    fn ampersand_end_terminator() {
        let nml = Namelist::parse("&g\n x = 1\n&end\n").unwrap();
        let mut x = 0;
        nml.group("g").unwrap().get_int("x", &mut x).unwrap();
        assert_eq!(x, 1);
    }

    #[test]
    fn unterminated_group_is_an_error() {
        assert!(matches!(
            Namelist::parse("&g\n x = 1\n"),
            Err(NamelistError::UnterminatedGroup(_))
        ));
    }

    #[test]
    fn leading_prose_is_ignored() {
        // MPTABLE.TBL-style files may carry text before the first group.
        let nml = Namelist::parse("Some header text\n&g\n x = 1\n/\n").unwrap();
        assert!(nml.has_group("g"));
    }

    #[test]
    fn logicals_and_negatives() {
        let nml = Namelist::parse("&g\n flag = .true.\n v = -2.5\n/\n").unwrap();
        let g = nml.group("g").unwrap();
        let mut flag = false;
        g.get_logical("flag", &mut flag).unwrap();
        assert!(flag);
        let mut v = 0.0f32;
        g.get_real("v", &mut v).unwrap();
        assert_eq!(v, -2.5);
    }

    #[test]
    fn model_options_group() {
        let src = "&model_options\n  precip_phase_option = 1\n  runoff_option = 8\n  \
                   drainage_option = 8\n/\n";
        let nml = Namelist::parse(src).unwrap();
        let g = nml.group("model_options").unwrap();
        let mut runoff = 0;
        g.get_int("runoff_option", &mut runoff).unwrap();
        assert_eq!(runoff, 8);
    }
}
