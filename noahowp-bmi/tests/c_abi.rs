//! The C BMI, driven the way a C caller drives it: allocate a `struct Bmi`, hand it to
//! `register_bmi`, and go through the function pointers with C strings and raw buffers.

use noahowp_bmi::c_abi::{register_bmi, Bmi, BMI_FAILURE, BMI_MAX_NAME, BMI_SUCCESS};
use std::ffi::{c_char, c_void, CStr, CString};
use std::mem::MaybeUninit;

fn config() -> tempfile::Path {
    let tables = concat!(env!("CARGO_MANIFEST_DIR"), "/../noahowp/tests/fixtures/");
    let text = include_str!("../../noahowp/tests/fixtures/namelist.input")
        .replace("\"../parameters/\"", &format!("\"{tables}\""));
    tempfile::write(&text)
}

/// Enough of a temp file for one config, without a dependency.
mod tempfile {
    pub struct Path(pub String);
    impl Drop for Path {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    pub fn write(text: &str) -> Path {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "noahowp-c-abi-{}-{}.input",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&p, text).unwrap();
        Path(p.to_string_lossy().into_owned())
    }
}

fn registered() -> Box<Bmi> {
    let mut slot: Box<MaybeUninit<Bmi>> = Box::new(MaybeUninit::uninit());
    unsafe {
        let p = register_bmi(slot.as_mut_ptr());
        assert_eq!(p, slot.as_mut_ptr());
        Box::from_raw(Box::into_raw(slot) as *mut Bmi)
    }
}

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}

fn buf_str(buf: &[u8]) -> String {
    CStr::from_bytes_until_nul(buf).unwrap().to_str().unwrap().to_string()
}

#[test]
fn full_lifecycle_through_the_function_table() {
    let cfg = config();
    let mut b = registered();
    let bp: *mut Bmi = &mut *b;
    unsafe {
        assert_eq!((b.initialize.unwrap())(bp, c(&cfg.0).as_ptr()), BMI_SUCCESS);

        let mut name = vec![0u8; BMI_MAX_NAME];
        assert_eq!((b.get_component_name.unwrap())(bp, name.as_mut_ptr() as *mut c_char), 0);
        assert_eq!(buf_str(&name), "Noah-OWP-Modular Surface Module");

        let mut n = 0;
        (b.get_input_item_count.unwrap())(bp, &mut n);
        assert_eq!(n, 8);
        let mut bufs: Vec<Vec<u8>> = (0..n).map(|_| vec![0u8; BMI_MAX_NAME]).collect();
        let mut ptrs: Vec<*mut c_char> =
            bufs.iter_mut().map(|b| b.as_mut_ptr() as *mut c_char).collect();
        assert_eq!((b.get_input_var_names.unwrap())(bp, ptrs.as_mut_ptr()), BMI_SUCCESS);
        assert_eq!(buf_str(&bufs[0]), "SFCPRS");
        assert_eq!(buf_str(&bufs[7]), "PRCPNONC");

        let mut ty = vec![0u8; BMI_MAX_NAME];
        (b.get_var_type.unwrap())(bp, c("ISNOW").as_ptr(), ty.as_mut_ptr() as *mut c_char);
        assert_eq!(buf_str(&ty), "integer");

        let mut nbytes = 0;
        (b.get_var_nbytes.unwrap())(bp, c("SNLIQ").as_ptr(), &mut nbytes);
        assert_eq!(nbytes, 12);

        for (name, mut v) in [
            ("SFCPRS", 101_325.0f32),
            ("SFCTMP", 285.0),
            ("SOLDN", 400.0),
            ("LWDN", 300.0),
            ("UU", 2.0),
            ("VV", 1.0),
            ("Q2", 0.006),
            ("PRCPNONC", 0.0005),
        ] {
            let p = &mut v as *mut f32 as *mut c_void;
            assert_eq!((b.set_value.unwrap())(bp, c(name).as_ptr(), p), BMI_SUCCESS, "{name}");
        }

        let mut dt = 0.0;
        (b.get_time_step.unwrap())(bp, &mut dt);
        assert_eq!((b.update.unwrap())(bp), BMI_SUCCESS);
        assert_eq!((b.update_until.unwrap())(bp, 3.0 * dt), BMI_SUCCESS);
        let mut t = 0.0;
        (b.get_current_time.unwrap())(bp, &mut t);
        assert_eq!(t, 3.0 * dt);

        let mut q = f32::NAN;
        let p = &mut q as *mut f32 as *mut c_void;
        assert_eq!((b.get_value.unwrap())(bp, c("QINSUR").as_ptr(), p), BMI_SUCCESS);
        assert!(q > 0.0);

        let mut snliq = [f32::NAN; 3];
        let p = snliq.as_mut_ptr() as *mut c_void;
        assert_eq!((b.get_value.unwrap())(bp, c("SNLIQ").as_ptr(), p), BMI_SUCCESS);
        assert!(snliq.iter().all(|x| x.is_finite()));

        let mut isnow = -7i32;
        let p = &mut isnow as *mut i32 as *mut c_void;
        assert_eq!((b.get_value.unwrap())(bp, c("ISNOW").as_ptr(), p), BMI_SUCCESS);
        assert_eq!(isnow, 0);

        // Failures come back as a status, not a crash.
        let mut x = 0.0f32;
        let p = &mut x as *mut f32 as *mut c_void;
        assert_eq!((b.get_value.unwrap())(bp, c("NOPE").as_ptr(), p), BMI_FAILURE);
        assert_eq!((b.set_value.unwrap())(bp, c("FSH").as_ptr(), p), BMI_FAILURE);
        assert_eq!((b.get_value.unwrap())(bp, std::ptr::null(), p), BMI_FAILURE);

        assert_eq!((b.finalize.unwrap())(bp), BMI_SUCCESS);
        assert!(b.data.is_null());
        assert_eq!((b.update.unwrap())(bp), BMI_FAILURE);
    }
}

#[test]
fn unimplemented_functions_are_null() {
    let mut b = registered();
    assert!(b.get_value_ptr.is_none());
    assert!(b.get_value_at_indices.is_none());
    assert!(b.set_value_at_indices.is_none());
    assert!(b.get_grid_shape.is_none());
    assert!(b.get_grid_x.is_none());
    unsafe { (b.finalize.unwrap())(&mut *b) };
}

#[test]
fn a_bad_config_fails_initialize() {
    let mut b = registered();
    let bp: *mut Bmi = &mut *b;
    unsafe {
        assert_eq!((b.initialize.unwrap())(bp, c("/nonexistent").as_ptr()), BMI_FAILURE);
        (b.finalize.unwrap())(bp);
    }
}
