//! Fortran list-directed input -- `READ (unit, *) a, b, c`.
//!
//! `SOILPARM.TBL` and `GENPARM.TBL` are read this way (`src/ParametersRead.f90`), not as
//! namelists. The semantics that matter here:
//!
//! - Each `READ` statement starts at a **new record** (line). Any values left on the previous
//!   record are discarded.
//! - Within a statement, values are separated by commas and/or whitespace, and the reader
//!   advances to following records until the io-list is satisfied.
//! - `n*value` repeats a value `n` times; `n*` repeats a null.
//! - A null value (`,,`) leaves the corresponding list item **unchanged**.
//! - `/` terminates the statement early, leaving remaining items unchanged.

use super::value::{parse_int, parse_logical, parse_real32, parse_string, ParseError};
use core::fmt;

#[derive(Debug)]
pub enum ReadError {
    /// Ran out of input before the io-list was satisfied.
    EndOfFile { wanted: &'static str },
    Value(ParseError),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::EndOfFile { wanted } => write!(f, "end of file while reading {wanted}"),
            ReadError::Value(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ReadError {}

impl From<ParseError> for ReadError {
    fn from(e: ParseError) -> Self {
        ReadError::Value(e)
    }
}

/// One item produced by the tokenizer.
#[derive(Debug, Clone, PartialEq)]
enum Item {
    Value(String),
    /// A null -- the target keeps its prior value.
    Null,
    /// `/` -- terminate the statement.
    Terminate,
}

/// A file being read by list-directed statements.
///
/// Mirrors a Fortran unit: it holds a record cursor, and each `read_*` call begins a new record.
pub struct ListDirectedReader {
    records: Vec<String>,
    /// Index of the next record to start a statement at.
    next_record: usize,
    /// Items pending within the current statement.
    pending: Vec<Item>,
    /// Record the current statement will continue from.
    cursor: usize,
}

impl ListDirectedReader {
    pub fn new(contents: &str) -> Self {
        Self {
            records: contents.lines().map(|l| l.to_string()).collect(),
            next_record: 0,
            pending: Vec::new(),
            cursor: 0,
        }
    }

    /// Begin a new `READ` statement: discard leftovers and advance to the next record.
    fn begin_statement(&mut self) {
        self.pending.clear();
        self.cursor = self.next_record;
    }

    /// Finish the statement: the next one starts at the record after the last one consumed.
    fn end_statement(&mut self) {
        self.next_record = self.cursor;
    }

    /// Pull the next item, pulling in further records as needed.
    fn next_item(&mut self) -> Option<Item> {
        loop {
            if !self.pending.is_empty() {
                return Some(self.pending.remove(0));
            }
            if self.cursor >= self.records.len() {
                return None;
            }
            let record = self.records[self.cursor].clone();
            self.cursor += 1;
            self.pending = tokenize_record(&record);
        }
    }

    /// `READ (u,*) s` where `s` is a character variable.
    pub fn read_string(&mut self) -> Result<String, ReadError> {
        self.begin_statement();
        let out = match self.next_item() {
            Some(Item::Value(v)) => parse_string(&v),
            Some(Item::Null) => String::new(),
            Some(Item::Terminate) | None => {
                return Err(ReadError::EndOfFile { wanted: "a string" })
            }
        };
        self.end_statement();
        Ok(out)
    }

    /// `READ (u,*) i`.
    pub fn read_int(&mut self) -> Result<i32, ReadError> {
        let mut out = [0i32; 1];
        self.read_ints(&mut out)?;
        Ok(out[0])
    }

    /// `READ (u,*) a, b, c` for integers. Null items leave the slot unchanged.
    pub fn read_ints(&mut self, out: &mut [i32]) -> Result<(), ReadError> {
        self.read_into(out, "integers", |slot, tok| {
            *slot = parse_int(tok)?;
            Ok(())
        })
    }

    /// `READ (u,*) x`.
    pub fn read_real(&mut self) -> Result<f32, ReadError> {
        let mut out = [0f32; 1];
        self.read_reals(&mut out)?;
        Ok(out[0])
    }

    /// `READ (u,*) a, b, c` for reals. Null items leave the slot unchanged.
    pub fn read_reals(&mut self, out: &mut [f32]) -> Result<(), ReadError> {
        self.read_into(out, "reals", |slot, tok| {
            *slot = parse_real32(tok)?;
            Ok(())
        })
    }

    /// `READ (u,*) i, a, b, ...` -- one statement whose first item is an integer and whose
    /// remaining items are reals.
    ///
    /// This is the shape of a `SOILPARM.TBL` row: the texture index followed by its
    /// parameters. Reading it as one statement matters, because a statement boundary is what
    /// discards the rest of the record -- the trailing `'SAND'` label here.
    pub fn read_int_then_reals(&mut self, first: &mut i32, rest: &mut [f32]) -> Result<(), ReadError> {
        self.begin_statement();
        match self.next_item() {
            Some(Item::Value(v)) => *first = parse_int(&v)?,
            Some(Item::Null) => {}
            Some(Item::Terminate) | None => {
                return Err(ReadError::EndOfFile { wanted: "an integer" })
            }
        }
        for slot in rest.iter_mut() {
            match self.next_item() {
                Some(Item::Value(v)) => *slot = parse_real32(&v)?,
                Some(Item::Null) => {}
                Some(Item::Terminate) => break,
                None => return Err(ReadError::EndOfFile { wanted: "reals" }),
            }
        }
        self.end_statement();
        Ok(())
    }

    /// `READ (u,*) l`.
    pub fn read_logical(&mut self) -> Result<bool, ReadError> {
        self.begin_statement();
        let out = match self.next_item() {
            Some(Item::Value(v)) => parse_logical(&v)?,
            Some(Item::Null) => false,
            Some(Item::Terminate) | None => {
                return Err(ReadError::EndOfFile { wanted: "a logical" })
            }
        };
        self.end_statement();
        Ok(out)
    }

    /// Skip one record, as a bare `READ (u,*)` does.
    pub fn skip_record(&mut self) {
        self.begin_statement();
        if self.cursor < self.records.len() {
            self.cursor += 1;
        }
        self.end_statement();
    }

    /// Scan forward for a record whose first token equals `name`, consuming it.
    ///
    /// This is the `do iLine = 1,100 / READ(21,*) SLTYPE / if (trim(SLTYPE) == ...) exit` idiom
    /// used to find the `STAS` or `SLOPE_DATA` block.
    pub fn find_block(&mut self, name: &str, max_records: usize) -> Option<()> {
        for _ in 0..max_records {
            let label = self.read_string().ok()?;
            if label.trim() == name {
                return Some(());
            }
        }
        None
    }

    fn read_into<T>(
        &mut self,
        out: &mut [T],
        wanted: &'static str,
        mut set: impl FnMut(&mut T, &str) -> Result<(), ParseError>,
    ) -> Result<(), ReadError> {
        self.begin_statement();
        for slot in out.iter_mut() {
            match self.next_item() {
                Some(Item::Value(v)) => set(slot, &v)?,
                Some(Item::Null) => {} // leaves the prior value in place
                Some(Item::Terminate) => break,
                None => return Err(ReadError::EndOfFile { wanted }),
            }
        }
        self.end_statement();
        Ok(())
    }
}

/// Split one record into list-directed items.
fn tokenize_record(record: &str) -> Vec<Item> {
    let mut items = Vec::new();
    let chars: Vec<char> = record.chars().collect();
    let mut i = 0;
    // True once a separator has been seen with no intervening value, so `a,,b` yields a null.
    let mut expect_value = true;

    while i < chars.len() {
        let c = chars[i];

        if c.is_whitespace() {
            i += 1;
            continue;
        }

        if c == ',' {
            if expect_value {
                items.push(Item::Null);
            }
            expect_value = true;
            i += 1;
            continue;
        }

        if c == '/' {
            items.push(Item::Terminate);
            return items;
        }

        if c == '!' {
            break;
        }

        // A value: quoted string, or a run up to the next separator.
        let start = i;
        if c == '\'' || c == '"' {
            let quote = c;
            i += 1;
            while i < chars.len() {
                if chars[i] == quote {
                    // A doubled quote is an escape, not a terminator.
                    if chars.get(i + 1) == Some(&quote) {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
        } else {
            while i < chars.len()
                && !chars[i].is_whitespace()
                && chars[i] != ','
                && chars[i] != '/'
                && chars[i] != '!'
            {
                i += 1;
            }
        }

        let token: String = chars[start..i].iter().collect();
        push_token(&mut items, &token);
        expect_value = false;
    }

    items
}

/// Push a token, expanding the `n*value` repeat form.
fn push_token(items: &mut Vec<Item>, token: &str) {
    // Only unquoted tokens can carry a repeat count.
    if !token.starts_with('\'') && !token.starts_with('"') {
        if let Some((count, rest)) = token.split_once('*') {
            if let Ok(n) = count.parse::<usize>() {
                let item = if rest.is_empty() {
                    Item::Null
                } else {
                    Item::Value(rest.to_string())
                };
                for _ in 0..n {
                    items.push(item.clone());
                }
                return;
            }
        }
    }
    items.push(Item::Value(token.to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_soilparm_row() {
        // Verbatim from parameters/SOILPARM.TBL, row 1.
        let src = "1,     2.79,    0.010,    -0.472,   0.339,   0.192,   0.069,  4.66E-5,  \
                   2.65E-5,   0.010,  0.92,  0.05,  0.009,  0.05,  0.05,  0.05,   1.000,  \
                   0.050,  'SAND'";
        let mut r = ListDirectedReader::new(src);
        let mut itmp = [0i32; 1];
        // The Fortran reads ITMP then 17 reals in one statement; do it as one record here by
        // tokenizing directly.
        let items = tokenize_record(src);
        assert_eq!(items.len(), 19);
        let _ = &mut itmp;
        let _ = &mut r;
        match &items[0] {
            Item::Value(v) => assert_eq!(v, "1"),
            other => panic!("{other:?}"),
        }
        match &items[7] {
            Item::Value(v) => assert_eq!(super::parse_real32(v).unwrap(), 4.66E-5f32),
            other => panic!("{other:?}"),
        }
        match &items[18] {
            Item::Value(v) => assert_eq!(parse_string(v), "SAND"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn statement_starts_at_a_new_record() {
        // Each READ consumes a whole record even if values remain on it.
        let mut r = ListDirectedReader::new("1 2 3\n4 5 6\n");
        assert_eq!(r.read_int().unwrap(), 1);
        // Second READ starts on the next record -- 2 and 3 are discarded.
        assert_eq!(r.read_int().unwrap(), 4);
    }

    #[test]
    fn statement_continues_across_records() {
        let mut r = ListDirectedReader::new("1.0 2.0\n3.0 4.0\n5.0\n");
        let mut out = [0f32; 4];
        r.read_reals(&mut out).unwrap();
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0]);
        // The next statement resumes on the record after the one that satisfied the list.
        assert_eq!(r.read_real().unwrap(), 5.0);
    }

    #[test]
    fn null_leaves_value_unchanged() {
        let mut r = ListDirectedReader::new("1.0,,3.0\n");
        let mut out = [9.0f32; 3];
        r.read_reals(&mut out).unwrap();
        assert_eq!(out, [1.0, 9.0, 3.0]);
    }

    #[test]
    fn slash_terminates_early() {
        let mut r = ListDirectedReader::new("1.0 2.0 / 3.0\n");
        let mut out = [9.0f32; 4];
        r.read_reals(&mut out).unwrap();
        assert_eq!(out, [1.0, 2.0, 9.0, 9.0]);
    }

    #[test]
    fn repeat_count() {
        let mut r = ListDirectedReader::new("3*1.5 2.0\n");
        let mut out = [0f32; 4];
        r.read_reals(&mut out).unwrap();
        assert_eq!(out, [1.5, 1.5, 1.5, 2.0]);
    }

    #[test]
    fn repeat_count_with_null() {
        let mut r = ListDirectedReader::new("2* 7.0\n");
        let mut out = [9.0f32; 3];
        r.read_reals(&mut out).unwrap();
        assert_eq!(out, [9.0, 9.0, 7.0]);
    }

    #[test]
    fn find_block_locates_soil_class() {
        // The header shape of SOILPARM.TBL.
        let src = "Soil Parameters\nSTAS\n19,1 'BB'\n";
        let mut r = ListDirectedReader::new(src);
        assert!(r.find_block("STAS", 100).is_some());
        // Next statement reads SLCATS from the following record.
        assert_eq!(r.read_int().unwrap(), 19);
    }

    #[test]
    fn find_block_reports_missing() {
        let mut r = ListDirectedReader::new("Soil Parameters\nSTAS\n");
        assert!(r.find_block("STAS-RUC", 10).is_none());
    }

    #[test]
    fn genparm_slope_block() {
        // GENPARM.TBL: label, count, then that many values one per record.
        let src = "General Parameters\nSLOPE_DATA\n9\n0.1 \n0.6\n1.0\n";
        let mut r = ListDirectedReader::new(src);
        r.skip_record(); // "General Parameters"
        assert_eq!(r.read_string().unwrap(), "SLOPE_DATA");
        assert_eq!(r.read_int().unwrap(), 9);
        assert_eq!(r.read_real().unwrap(), 0.1f32);
        assert_eq!(r.read_real().unwrap(), 0.6f32);
        assert_eq!(r.read_real().unwrap(), 1.0f32);
    }

    #[test]
    fn end_of_file_is_an_error() {
        let mut r = ListDirectedReader::new("1.0\n");
        let mut out = [0f32; 3];
        assert!(matches!(
            r.read_reals(&mut out),
            Err(ReadError::EndOfFile { .. })
        ));
    }

    #[test]
    fn quoted_string_with_spaces() {
        let items = tokenize_record("1, 'SANDY CLAY LOAM'");
        assert_eq!(items.len(), 2);
        match &items[1] {
            Item::Value(v) => assert_eq!(parse_string(v), "SANDY CLAY LOAM"),
            other => panic!("{other:?}"),
        }
    }
}
