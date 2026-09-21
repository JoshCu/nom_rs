//! Reader for the differential-test fixtures produced by `reference/build.sh`.
//!
//! Each fixture is a recording of the reference Fortran running Bondville: for a sample of
//! timesteps, the complete model state immediately before and immediately after each of the
//! five `*Main` physics calls. A ported Rust function is correct when, handed the pre-state, it
//! reproduces the post-state bit for bit.
//!
//! The field layout lives in [`manifest`], generated from the Fortran derived types by
//! `reference/gen_serializer.py` so that both sides of the format come from one source. Nothing
//! here allocates a parser dependency: the model crate has none, and the fixtures are a flat
//! little-endian byte stream by design.
//!
//! # Format
//!
//! ```text
//! magic u32 ('NOMD')                     -- static block: the run configuration
//!   levels, options, parameters
//! repeated:
//!   magic u32, tag i32, phase i32, itime i32
//!   domain, forcing, energy, water
//! ```
//!
//! Fields are written in manifest order. An `allocatable` field carries an `i32` element count
//! immediately before its data, so a fixture generated for a different `nsoil`/`nsnow` is caught
//! on read rather than reinterpreted into garbage.

pub mod manifest;
pub mod bind;

use manifest::{Field, Kind, Shape, MAGIC, PARAM_SWEEP, PER_RECORD, STATIC};
use std::collections::HashMap;
use std::fmt;

/// Which physics call a record brackets. Values are part of the fixture format; see
/// `reference/instrument.py`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Call {
    Utilities = 1,
    Forcing = 2,
    Interception = 3,
    Energy = 4,
    Water = 5,
}

impl Call {
    fn from_tag(tag: i32) -> Result<Self, FixtureError> {
        Ok(match tag {
            1 => Call::Utilities,
            2 => Call::Forcing,
            3 => Call::Interception,
            4 => Call::Energy,
            5 => Call::Water,
            other => return Err(FixtureError::UnknownTag(other)),
        })
    }

    /// The Fortran subroutine this record brackets.
    pub fn subroutine(self) -> &'static str {
        match self {
            Call::Utilities => "UtilitiesMain",
            Call::Forcing => "ForcingMain",
            Call::Interception => "InterceptionMain",
            Call::Energy => "EnergyMain",
            Call::Water => "WaterMain",
        }
    }
}

/// Whether a record was taken before or after the call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Before,
    After,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FixtureError {
    /// The record header did not start with `'NOMD'` -- the stream is truncated or misaligned.
    BadMagic { offset: usize, found: u32 },
    /// Ran off the end of the data mid-field.
    Truncated { offset: usize, want: usize },
    UnknownTag(i32),
    UnknownPhase(i32),
    /// An `allocatable` length that cannot be right, e.g. negative or absurdly large.
    BadLength { field: &'static str, len: i32 },
    /// Trailing bytes after the last complete record.
    TrailingBytes(usize),
}

impl fmt::Display for FixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FixtureError::BadMagic { offset, found } => {
                write!(f, "bad magic {found:#010x} at offset {offset}")
            }
            FixtureError::Truncated { offset, want } => {
                write!(f, "truncated at offset {offset}, wanted {want} bytes")
            }
            FixtureError::UnknownTag(t) => write!(f, "unknown call tag {t}"),
            FixtureError::UnknownPhase(p) => write!(f, "unknown phase {p}"),
            FixtureError::BadLength { field, len } => {
                write!(f, "implausible length {len} for allocatable field {field}")
            }
            FixtureError::TrailingBytes(n) => write!(f, "{n} trailing bytes after last record"),
        }
    }
}

impl std::error::Error for FixtureError {}

/// The bytes of one derived type, indexed by field name.
///
/// Lookups are case-insensitive. Fortran is, and the derived types spell their components
/// inconsistently -- `WaterType` has both `SNOWH` and `sh2o` -- so requiring the caller to
/// match the source casing would only invite avoidable panics.
#[derive(Debug, Clone)]
pub struct State {
    /// The Fortran type this came from, e.g. `"energy"`.
    pub type_name: &'static str,
    bytes: Vec<u8>,
    fields: Vec<(&'static str, Span)>,
    order: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy)]
struct Span {
    start: usize,
    count: usize,
    kind: Kind,
}

impl State {
    /// Field names, in the order the Fortran writes them.
    pub fn field_names(&self) -> &[&'static str] {
        &self.order
    }

    /// Raw bytes of a field, or `None` if this type has no such field.
    pub fn bytes(&self, field: &str) -> Option<&[u8]> {
        let s = self.span(field)?;
        Some(&self.bytes[s.start..s.start + s.count * s.kind.width()])
    }

    fn span(&self, field: &str) -> Option<&Span> {
        self.fields
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(field))
            .map(|(_, span)| span)
    }

    /// A scalar `real`. Panics if the field is absent or not a `real`.
    pub fn f32(&self, field: &str) -> f32 {
        let b = self.expect(field, Kind::R4);
        f32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }

    /// A `real` array.
    pub fn f32s(&self, field: &str) -> Vec<f32> {
        self.expect(field, Kind::R4)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect()
    }

    /// A scalar `real*8` or `double precision`.
    pub fn f64(&self, field: &str) -> f64 {
        let b = self.expect(field, Kind::R8);
        f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
    }

    /// A scalar `integer`.
    pub fn i32(&self, field: &str) -> i32 {
        let b = self.expect(field, Kind::I4);
        i32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }

    /// An `integer` array.
    pub fn i32s(&self, field: &str) -> Vec<i32> {
        self.expect(field, Kind::I4)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| i32::from_le_bytes(*c))
            .collect()
    }

    /// A `logical`, which the Fortran side normalises to 0 or 1.
    pub fn bool(&self, field: &str) -> bool {
        let b = self.expect(field, Kind::L4);
        i32::from_le_bytes([b[0], b[1], b[2], b[3]]) != 0
    }

    /// A `character(len=N)`, trailing spaces trimmed.
    pub fn str(&self, field: &str) -> String {
        let s = self.span(field).unwrap_or_else(|| self.missing(field));
        match s.kind {
            Kind::Char(_) => {}
            other => panic!("{}%{field} is {other:?}, not character", self.type_name),
        }
        String::from_utf8_lossy(self.bytes(field).unwrap())
            .trim_end()
            .to_string()
    }

    /// Names of the fields whose bytes differ, in manifest order.
    ///
    /// This is the assertion a ported physics function is held to: run it on the pre-state and
    /// the result must differ from the recorded post-state in nothing at all.
    pub fn diff(&self, other: &State) -> Vec<&'static str> {
        self.order
            .iter()
            .copied()
            .filter(|f| self.bytes(f) != other.bytes(f))
            .collect()
    }

    fn expect(&self, field: &str, kind: Kind) -> &[u8] {
        let s = self.span(field).unwrap_or_else(|| self.missing(field));
        assert_eq!(
            s.kind, kind,
            "{}%{field} is {:?}, not {kind:?}",
            self.type_name, s.kind
        );
        self.bytes(field).unwrap()
    }

    fn missing(&self, field: &str) -> ! {
        panic!(
            "{} has no field {field:?}; it has {:?}",
            self.type_name, self.order
        )
    }
}

/// One `(call, phase)` snapshot.
#[derive(Debug, Clone)]
pub struct Record {
    pub call: Call,
    pub phase: Phase,
    /// The Fortran's `domain%itime`, 1-based.
    pub itime: i32,
    states: HashMap<&'static str, State>,
}

impl Record {
    /// The recorded state of one derived type, e.g. `"water"`.
    pub fn state(&self, type_name: &str) -> &State {
        self.states
            .get(type_name)
            .unwrap_or_else(|| panic!("no {type_name:?} in record"))
    }
}

/// A parsed fixture file.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// Run configuration: `levels`, `options`, `parameters`. Written once.
    statics: HashMap<&'static str, State>,
    pub records: Vec<Record>,
}

impl Fixture {
    /// The run configuration, e.g. `"parameters"`.
    pub fn static_state(&self, type_name: &str) -> &State {
        self.statics
            .get(type_name)
            .unwrap_or_else(|| panic!("no static {type_name:?} in fixture"))
    }

    /// The before/after pairs for one physics call, in timestep order.
    ///
    /// Pairs are matched by position rather than by `itime` so that a missing or duplicated
    /// record shows up as a mismatched pair instead of being quietly skipped.
    pub fn pairs(&self, call: Call) -> Vec<(&Record, &Record)> {
        let mut pending: Option<&Record> = None;
        let mut out = Vec::new();
        for r in self.records.iter().filter(|r| r.call == call) {
            match (r.phase, pending.take()) {
                (Phase::Before, _) => pending = Some(r),
                (Phase::After, Some(before)) => out.push((before, r)),
                (Phase::After, None) => panic!("{:?}: after with no before", call),
            }
        }
        out
    }

    /// Parse a fixture.
    pub fn parse(data: &[u8]) -> Result<Self, FixtureError> {
        let mut p = Parser { data, at: 0 };

        p.magic()?;
        let mut statics = HashMap::new();
        for (name, fields) in STATIC {
            statics.insert(*name, p.state(name, fields)?);
        }

        let mut records = Vec::new();
        while p.at < p.data.len() {
            p.magic()?;
            let call = Call::from_tag(p.i32()?)?;
            let phase = match p.i32()? {
                0 => Phase::Before,
                1 => Phase::After,
                other => return Err(FixtureError::UnknownPhase(other)),
            };
            let itime = p.i32()?;
            let mut states = HashMap::new();
            for (name, fields) in PER_RECORD {
                states.insert(*name, p.state(name, fields)?);
            }
            records.push(Record {
                call,
                phase,
                itime,
                states,
            });
        }

        Ok(Fixture { statics, records })
    }
}

/// Which vegetation classification a sweep case used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VegDataset {
    Usgs,
    ModifiedIgbpModisNoah,
}

impl VegDataset {
    /// The string `veg_class_name` must hold to select this dataset.
    pub fn name(self) -> &'static str {
        match self {
            VegDataset::Usgs => "USGS",
            VegDataset::ModifiedIgbpModisNoah => "MODIFIED_IGBP_MODIS_NOAH",
        }
    }

    fn from_tag(tag: i32) -> Result<Self, FixtureError> {
        match tag {
            1 => Ok(VegDataset::Usgs),
            2 => Ok(VegDataset::ModifiedIgbpModisNoah),
            other => Err(FixtureError::UnknownTag(other)),
        }
    }
}

/// One case of the parameter sweep: the class indices, and the parameters they produced.
///
/// Bondville exercises a single `(vegtyp, isltyp, soilcolor)`, which leaves most of every table
/// unread. The sweep walks each index in turn -- including past the number of rows the file
/// supplies, where the Fortran legitimately returns its `-1.E36` sentinel.
#[derive(Debug, Clone)]
pub struct ParamCase {
    pub dataset: VegDataset,
    pub vegtyp: i32,
    pub isltyp: i32,
    pub soilcolor: i32,
    pub parameters: State,
}

/// Parse a parameter-sweep fixture.
pub fn parse_param_sweep(data: &[u8]) -> Result<Vec<ParamCase>, FixtureError> {
    let mut p = Parser { data, at: 0 };
    let mut cases = Vec::new();
    while p.at < p.data.len() {
        p.magic()?;
        let dataset = VegDataset::from_tag(p.i32()?)?;
        let vegtyp = p.i32()?;
        let isltyp = p.i32()?;
        let soilcolor = p.i32()?;
        let (name, fields) = PARAM_SWEEP[0];
        cases.push(ParamCase {
            dataset,
            vegtyp,
            isltyp,
            soilcolor,
            parameters: p.state(name, fields)?,
        });
    }
    Ok(cases)
}

struct Parser<'a> {
    data: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], FixtureError> {
        if self.at + n > self.data.len() {
            return Err(FixtureError::Truncated {
                offset: self.at,
                want: n,
            });
        }
        let out = &self.data[self.at..self.at + n];
        self.at += n;
        Ok(out)
    }

    fn i32(&mut self) -> Result<i32, FixtureError> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn magic(&mut self) -> Result<(), FixtureError> {
        let offset = self.at;
        let found = self.i32()? as u32;
        if found != MAGIC {
            return Err(FixtureError::BadMagic { offset, found });
        }
        Ok(())
    }

    fn state(&mut self, type_name: &'static str, fields: &[Field]) -> Result<State, FixtureError> {
        let start_of_state = self.at;
        let mut index = Vec::with_capacity(fields.len());
        let mut order = Vec::with_capacity(fields.len());

        for f in fields {
            let count = match f.shape {
                Shape::Scalar => 1,
                Shape::Fixed(n) => n,
                Shape::Alloc => {
                    let n = self.i32()?;
                    // Profile arrays are nsnow+nsoil long. A length outside this range means the
                    // stream has lost alignment, and reading on would produce plausible-looking
                    // nonsense rather than an error.
                    if !(0..=4096).contains(&n) {
                        return Err(FixtureError::BadLength {
                            field: f.name,
                            len: n,
                        });
                    }
                    n as usize
                }
            };
            let start = self.at - start_of_state;
            self.take(count * f.kind.width())?;
            index.push((
                f.name,
                Span {
                    start,
                    count,
                    kind: f.kind,
                },
            ));
            order.push(f.name);
        }

        Ok(State {
            type_name,
            bytes: self.data[start_of_state..self.at].to_vec(),
            fields: index,
            order,
        })
    }
}
