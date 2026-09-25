//! The C BMI: `register_bmi` and the function table it fills.
//!
//! [`Bmi`] is CSDMS `bmi.h` (BMI 2.0) field for field -- the struct ngen's `bmi_c` formulation
//! and `bmi-driver`'s `BmiC` adapter both allocate and pass to the registration function. The
//! model lives behind `data` as a boxed [`BmiNoahOwp`].
//!
//! Upstream's Fortran `register_bmi` hands back a `box` for the `iso_c_bmi` middleware; this is
//! the plain C convention instead, so no middleware is needed.
//!
//! Every entry point returns `BMI_FAILURE` rather than unwinding: a panic is caught at the
//! boundary, and every error is printed to stderr before it is flattened to a status code,
//! since the status code is all a C caller sees.
//!
//! `get_value_ptr`, `*_at_indices` and the extended grid functions stay null, as the Fortran
//! leaves them unimplemented. Callers check for null before calling.

use crate::{BmiError, BmiNoahOwp, VarType};
use std::ffi::{c_char, c_double, c_int, c_void, CStr};
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const BMI_SUCCESS: c_int = 0;
pub const BMI_FAILURE: c_int = 1;
/// `BMI_MAX_*_NAME` in `bmi.h` -- the size of every string buffer the caller passes.
pub const BMI_MAX_NAME: usize = 2048;

type Self_ = *mut Bmi;
type FnStr = Option<unsafe extern "C" fn(Self_, *mut c_char) -> c_int>;
type FnInt = Option<unsafe extern "C" fn(Self_, *mut c_int) -> c_int>;
type FnDouble = Option<unsafe extern "C" fn(Self_, *mut c_double) -> c_int>;
type FnNames = Option<unsafe extern "C" fn(Self_, *mut *mut c_char) -> c_int>;
type FnVarInt = Option<unsafe extern "C" fn(Self_, *const c_char, *mut c_int) -> c_int>;
type FnVarStr = Option<unsafe extern "C" fn(Self_, *const c_char, *mut c_char) -> c_int>;
type FnVarBuf = Option<unsafe extern "C" fn(Self_, *const c_char, *mut c_void) -> c_int>;
type FnGridInt = Option<unsafe extern "C" fn(Self_, c_int, *mut c_int) -> c_int>;
type FnGridDouble = Option<unsafe extern "C" fn(Self_, c_int, *mut c_double) -> c_int>;
type FnGridStr = Option<unsafe extern "C" fn(Self_, c_int, *mut c_char) -> c_int>;

/// `struct Bmi` from `bmi.h`.
#[repr(C)]
pub struct Bmi {
    pub data: *mut c_void,

    pub initialize: Option<unsafe extern "C" fn(Self_, *const c_char) -> c_int>,
    pub update: Option<unsafe extern "C" fn(Self_) -> c_int>,
    pub update_until: Option<unsafe extern "C" fn(Self_, c_double) -> c_int>,
    pub finalize: Option<unsafe extern "C" fn(Self_) -> c_int>,

    pub get_component_name: FnStr,
    pub get_input_item_count: FnInt,
    pub get_output_item_count: FnInt,
    pub get_input_var_names: FnNames,
    pub get_output_var_names: FnNames,

    pub get_var_grid: FnVarInt,
    pub get_var_type: FnVarStr,
    pub get_var_units: FnVarStr,
    pub get_var_itemsize: FnVarInt,
    pub get_var_nbytes: FnVarInt,
    pub get_var_location: FnVarStr,

    pub get_current_time: FnDouble,
    pub get_start_time: FnDouble,
    pub get_end_time: FnDouble,
    pub get_time_units: FnStr,
    pub get_time_step: FnDouble,

    pub get_value: FnVarBuf,
    pub get_value_ptr: Option<unsafe extern "C" fn(Self_, *const c_char, *mut *mut c_void) -> c_int>,
    pub get_value_at_indices:
        Option<unsafe extern "C" fn(Self_, *const c_char, *mut c_void, *mut c_int, c_int) -> c_int>,

    pub set_value: FnVarBuf,
    pub set_value_at_indices:
        Option<unsafe extern "C" fn(Self_, *const c_char, *mut c_int, c_int, *mut c_void) -> c_int>,

    pub get_grid_rank: FnGridInt,
    pub get_grid_size: FnGridInt,
    pub get_grid_type: FnGridStr,
    pub get_grid_shape: FnGridInt,
    pub get_grid_spacing: FnGridDouble,
    pub get_grid_origin: FnGridDouble,

    pub get_grid_x: FnGridDouble,
    pub get_grid_y: FnGridDouble,
    pub get_grid_z: FnGridDouble,

    pub get_grid_node_count: FnGridInt,
    pub get_grid_edge_count: FnGridInt,
    pub get_grid_face_count: FnGridInt,
    pub get_grid_edge_nodes: FnGridInt,
    pub get_grid_face_edges: FnGridInt,
    pub get_grid_face_nodes: FnGridInt,
    pub get_grid_nodes_per_face: FnGridInt,
}

/// Fill `model` with this library's function table and a fresh, uninitialized instance.
///
/// # Safety
/// `model` must be null or point to a writable `struct Bmi`. The instance is freed by
/// `finalize`; a `Bmi` that is never finalized leaks it.
#[no_mangle]
pub unsafe extern "C" fn register_bmi(model: *mut Bmi) -> *mut Bmi {
    if model.is_null() {
        return model;
    }
    let instance = Box::into_raw(Box::new(BmiNoahOwp::new())) as *mut c_void;
    model.write(Bmi {
        data: instance,

        initialize: Some(initialize),
        update: Some(update),
        update_until: Some(update_until),
        finalize: Some(finalize),

        get_component_name: Some(get_component_name),
        get_input_item_count: Some(get_input_item_count),
        get_output_item_count: Some(get_output_item_count),
        get_input_var_names: Some(get_input_var_names),
        get_output_var_names: Some(get_output_var_names),

        get_var_grid: Some(get_var_grid),
        get_var_type: Some(get_var_type),
        get_var_units: Some(get_var_units),
        get_var_itemsize: Some(get_var_itemsize),
        get_var_nbytes: Some(get_var_nbytes),
        get_var_location: Some(get_var_location),

        get_current_time: Some(get_current_time),
        get_start_time: Some(get_start_time),
        get_end_time: Some(get_end_time),
        get_time_units: Some(get_time_units),
        get_time_step: Some(get_time_step),

        get_value: Some(get_value),
        get_value_ptr: None,
        get_value_at_indices: None,

        set_value: Some(set_value),
        set_value_at_indices: None,

        get_grid_rank: Some(get_grid_rank),
        get_grid_size: Some(get_grid_size),
        get_grid_type: Some(get_grid_type),
        get_grid_shape: None,
        get_grid_spacing: None,
        get_grid_origin: None,

        get_grid_x: None,
        get_grid_y: None,
        get_grid_z: None,

        get_grid_node_count: None,
        get_grid_edge_count: None,
        get_grid_face_count: None,
        get_grid_edge_nodes: None,
        get_grid_face_edges: None,
        get_grid_face_nodes: None,
        get_grid_nodes_per_face: None,
    });
    model
}

// ---------------------------------------------------------------------- plumbing

/// Why a call failed before it reached the model.
enum AbiError {
    NullPointer,
    BadString,
    Model(BmiError),
}

impl From<BmiError> for AbiError {
    fn from(e: BmiError) -> Self {
        AbiError::Model(e)
    }
}

/// Run `f` against the instance behind `this`, turning errors and panics into `BMI_FAILURE`.
unsafe fn with<F>(this: Self_, what: &str, f: F) -> c_int
where
    F: FnOnce(&mut BmiNoahOwp) -> Result<(), AbiError>,
{
    let result = catch_unwind(AssertUnwindSafe(|| {
        let bmi = this.as_mut().ok_or(AbiError::NullPointer)?;
        let inst = (bmi.data as *mut BmiNoahOwp).as_mut().ok_or(AbiError::NullPointer)?;
        f(inst)
    }));
    match result {
        Ok(Ok(())) => BMI_SUCCESS,
        Ok(Err(e)) => {
            match e {
                AbiError::NullPointer => eprintln!("noahowp {what}: null pointer"),
                AbiError::BadString => eprintln!("noahowp {what}: string is not UTF-8"),
                AbiError::Model(e) => eprintln!("noahowp {what}: {e}"),
            }
            BMI_FAILURE
        }
        Err(_) => {
            eprintln!("noahowp {what}: panicked");
            BMI_FAILURE
        }
    }
}

unsafe fn read_str<'a>(p: *const c_char) -> Result<&'a str, AbiError> {
    if p.is_null() {
        return Err(AbiError::NullPointer);
    }
    CStr::from_ptr(p).to_str().map_err(|_| AbiError::BadString)
}

/// Copy `s` into a caller buffer of `BMI_MAX_NAME` bytes, NUL-terminated and truncated to fit.
unsafe fn write_str(dest: *mut c_char, s: &str) -> Result<(), AbiError> {
    if dest.is_null() {
        return Err(AbiError::NullPointer);
    }
    let n = s.len().min(BMI_MAX_NAME - 1);
    std::ptr::copy_nonoverlapping(s.as_ptr(), dest as *mut u8, n);
    *dest.add(n) = 0;
    Ok(())
}

unsafe fn write<T>(dest: *mut T, v: T) -> Result<(), AbiError> {
    if dest.is_null() {
        return Err(AbiError::NullPointer);
    }
    dest.write(v);
    Ok(())
}

unsafe fn write_names(dest: *mut *mut c_char, names: &[&str]) -> Result<(), AbiError> {
    if dest.is_null() {
        return Err(AbiError::NullPointer);
    }
    for (i, name) in names.iter().enumerate() {
        write_str(*dest.add(i), name)?;
    }
    Ok(())
}

/// Number of items in `name`'s buffer, per the BMI contract that it is `nbytes` long.
fn item_count(inst: &BmiNoahOwp, name: &str) -> Result<usize, AbiError> {
    let nbytes = inst.get_var_nbytes(name)?;
    let itemsize = inst.get_var_itemsize(name)?;
    Ok((nbytes / itemsize) as usize)
}

// ---------------------------------------------------------------------- lifecycle

unsafe extern "C" fn initialize(this: Self_, config_file: *const c_char) -> c_int {
    with(this, "initialize", |m| {
        let path = read_str(config_file)?;
        Ok(m.initialize(path)?)
    })
}

unsafe extern "C" fn update(this: Self_) -> c_int {
    with(this, "update", |m| Ok(m.update()?))
}

unsafe extern "C" fn update_until(this: Self_, time: c_double) -> c_int {
    with(this, "update_until", |m| Ok(m.update_until(time)?))
}

/// `finalize` also frees the instance `register_bmi` allocated; later calls fail cleanly.
unsafe extern "C" fn finalize(this: Self_) -> c_int {
    let status = with(this, "finalize", |m| Ok(m.finalize()?));
    if let Some(bmi) = this.as_mut() {
        if !bmi.data.is_null() {
            drop(Box::from_raw(bmi.data as *mut BmiNoahOwp));
            bmi.data = std::ptr::null_mut();
        }
    }
    status
}

// ---------------------------------------------------------------------- metadata

unsafe extern "C" fn get_component_name(this: Self_, name: *mut c_char) -> c_int {
    with(this, "get_component_name", |m| write_str(name, m.get_component_name()))
}

unsafe extern "C" fn get_input_item_count(this: Self_, count: *mut c_int) -> c_int {
    with(this, "get_input_item_count", |m| write(count, m.get_input_item_count()))
}

unsafe extern "C" fn get_output_item_count(this: Self_, count: *mut c_int) -> c_int {
    with(this, "get_output_item_count", |m| write(count, m.get_output_item_count()))
}

unsafe extern "C" fn get_input_var_names(this: Self_, names: *mut *mut c_char) -> c_int {
    with(this, "get_input_var_names", |m| write_names(names, m.get_input_var_names()))
}

unsafe extern "C" fn get_output_var_names(this: Self_, names: *mut *mut c_char) -> c_int {
    with(this, "get_output_var_names", |m| write_names(names, m.get_output_var_names()))
}

unsafe extern "C" fn get_var_grid(this: Self_, name: *const c_char, grid: *mut c_int) -> c_int {
    with(this, "get_var_grid", |m| write(grid, m.get_var_grid(read_str(name)?)?))
}

unsafe extern "C" fn get_var_type(this: Self_, name: *const c_char, ty: *mut c_char) -> c_int {
    with(this, "get_var_type", |m| write_str(ty, m.get_var_type(read_str(name)?)?))
}

unsafe extern "C" fn get_var_units(this: Self_, name: *const c_char, units: *mut c_char) -> c_int {
    with(this, "get_var_units", |m| write_str(units, m.get_var_units(read_str(name)?)?))
}

unsafe extern "C" fn get_var_itemsize(this: Self_, name: *const c_char, size: *mut c_int) -> c_int {
    with(this, "get_var_itemsize", |m| write(size, m.get_var_itemsize(read_str(name)?)?))
}

unsafe extern "C" fn get_var_nbytes(this: Self_, name: *const c_char, nbytes: *mut c_int) -> c_int {
    with(this, "get_var_nbytes", |m| write(nbytes, m.get_var_nbytes(read_str(name)?)?))
}

unsafe extern "C" fn get_var_location(this: Self_, name: *const c_char, loc: *mut c_char) -> c_int {
    with(this, "get_var_location", |m| write_str(loc, m.get_var_location(read_str(name)?)))
}

// ---------------------------------------------------------------------- time

unsafe extern "C" fn get_current_time(this: Self_, time: *mut c_double) -> c_int {
    with(this, "get_current_time", |m| write(time, m.get_current_time()?))
}

unsafe extern "C" fn get_start_time(this: Self_, time: *mut c_double) -> c_int {
    with(this, "get_start_time", |m| write(time, m.get_start_time()))
}

unsafe extern "C" fn get_end_time(this: Self_, time: *mut c_double) -> c_int {
    with(this, "get_end_time", |m| write(time, m.get_end_time()?))
}

unsafe extern "C" fn get_time_units(this: Self_, units: *mut c_char) -> c_int {
    with(this, "get_time_units", |m| write_str(units, m.get_time_units()))
}

unsafe extern "C" fn get_time_step(this: Self_, dt: *mut c_double) -> c_int {
    with(this, "get_time_step", |m| write(dt, m.get_time_step()?))
}

// ---------------------------------------------------------------------- values

/// The buffer is typed by the variable: `ISNOW` is `int`, everything else `float`.
unsafe extern "C" fn get_value(this: Self_, name: *const c_char, dest: *mut c_void) -> c_int {
    with(this, "get_value", |m| {
        let name = read_str(name)?;
        let n = item_count(m, name)?;
        if dest.is_null() {
            return Err(AbiError::NullPointer);
        }
        match m.var(name)?.var_type {
            VarType::Real => {
                let buf = std::slice::from_raw_parts_mut(dest as *mut f32, n);
                Ok(m.get_value_f32(name, buf)?)
            }
            VarType::Integer => {
                let buf = std::slice::from_raw_parts_mut(dest as *mut i32, n);
                Ok(m.get_value_i32(name, buf)?)
            }
        }
    })
}

/// Reads `nbytes` from `src`, as the Fortran's `iso_c_bmi` wrapper does.
unsafe extern "C" fn set_value(this: Self_, name: *const c_char, src: *mut c_void) -> c_int {
    with(this, "set_value", |m| {
        let name = read_str(name)?;
        let n = item_count(m, name)?;
        if src.is_null() {
            return Err(AbiError::NullPointer);
        }
        match m.var(name)?.var_type {
            VarType::Real => {
                let buf = std::slice::from_raw_parts(src as *const f32, n);
                Ok(m.set_value_f32(name, buf)?)
            }
            VarType::Integer => {
                let buf = std::slice::from_raw_parts(src as *const i32, n);
                Ok(m.set_value_i32(name, buf)?)
            }
        }
    })
}

// ---------------------------------------------------------------------- grids

unsafe extern "C" fn get_grid_rank(this: Self_, grid: c_int, rank: *mut c_int) -> c_int {
    with(this, "get_grid_rank", |m| write(rank, m.get_grid_rank(grid)?))
}

unsafe extern "C" fn get_grid_size(this: Self_, grid: c_int, size: *mut c_int) -> c_int {
    with(this, "get_grid_size", |m| write(size, m.get_grid_size(grid)?))
}

unsafe extern "C" fn get_grid_type(this: Self_, grid: c_int, ty: *mut c_char) -> c_int {
    with(this, "get_grid_type", |m| write_str(ty, m.get_grid_type(grid)?))
}
