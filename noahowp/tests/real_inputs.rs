//! Exercise the Fortran readers against the actual Noah-OWP input files.
//!
//! Hand-written fixtures prove the parsers handle what we *expect*; these prove they handle
//! what upstream actually ships. See `tests/fixtures/README.md`.

use noahowp::fortran::{ListDirectedReader, Namelist};

fn fixture(name: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/");
    std::fs::read_to_string(format!("{path}{name}"))
        .unwrap_or_else(|e| panic!("reading fixture {name}: {e}"))
}

// ---------------------------------------------------------------- namelist.input

#[test]
fn namelist_input_all_groups_present() {
    let nml = Namelist::parse(&fixture("namelist.input")).unwrap();
    for g in [
        "timing",
        "parameters",
        "location",
        "forcing",
        "model_options",
        "structure",
        "initial_values",
    ] {
        assert!(nml.has_group(g), "missing group &{g}");
    }
}

#[test]
fn namelist_input_timing_values() {
    let nml = Namelist::parse(&fixture("namelist.input")).unwrap();
    let g = nml.group("timing").unwrap();

    let mut dt = 0.0f32;
    g.get_real("dt", &mut dt).unwrap();
    assert_eq!(dt, 1800.0);

    let mut startdate = String::new();
    let mut enddate = String::new();
    g.get_string("startdate", &mut startdate).unwrap();
    g.get_string("enddate", &mut enddate).unwrap();
    assert_eq!(startdate, "199801010630");
    assert_eq!(enddate, "199901010630");
}

#[test]
fn namelist_input_structure_and_initial_values() {
    let nml = Namelist::parse(&fixture("namelist.input")).unwrap();

    let s = nml.group("structure").unwrap();
    let (mut nsoil, mut nsnow, mut nveg, mut vegtyp, mut isltyp) = (0, 0, 0, 0, 0);
    s.get_int("nsoil", &mut nsoil).unwrap();
    s.get_int("nsnow", &mut nsnow).unwrap();
    s.get_int("nveg", &mut nveg).unwrap();
    s.get_int("vegtyp", &mut vegtyp).unwrap();
    s.get_int("isltyp", &mut isltyp).unwrap();
    assert_eq!((nsoil, nsnow, nveg, vegtyp, isltyp), (4, 3, 20, 1, 1));

    let iv = nml.group("initial_values").unwrap();
    // dzsnso spans nsnow + nsoil = 7 layers.
    let mut dzsnso = [0.0f32; 7];
    iv.get_real_array("dzsnso", &mut dzsnso).unwrap();
    assert_eq!(dzsnso, [0.0, 0.0, 0.0, 0.1, 0.3, 0.6, 1.0]);

    let mut sh2o = [0.0f32; 4];
    iv.get_real_array("sh2o", &mut sh2o).unwrap();
    assert_eq!(sh2o, [0.3, 0.3, 0.3, 0.3]);

    let mut sice = [9.0f32; 4];
    iv.get_real_array("sice", &mut sice).unwrap();
    assert_eq!(sice, [0.0, 0.0, 0.0, 0.0]);

    let mut zwt = 0.0f32;
    iv.get_real("zwt", &mut zwt).unwrap();
    assert_eq!(zwt, -2.0);
}

#[test]
fn namelist_input_model_options() {
    let nml = Namelist::parse(&fixture("namelist.input")).unwrap();
    let g = nml.group("model_options").unwrap();

    // Every option in the shipped namelist, so a parser regression shows up as a wrong option
    // rather than a silent default.
    for (key, want) in [
        ("precip_phase_option", 1),
        ("snow_albedo_option", 1),
        ("dynamic_veg_option", 1),
        ("runoff_option", 8),
        ("drainage_option", 8),
        ("frozen_soil_option", 1),
        ("dynamic_vic_option", 1),
        ("radiative_transfer_option", 3),
        ("sfc_drag_coeff_option", 1),
        ("canopy_stom_resist_option", 1),
        ("crop_model_option", 0),
        ("snowsoil_temp_time_option", 3),
        ("soil_temp_boundary_option", 2),
        ("supercooled_water_option", 1),
        ("stomatal_resistance_option", 1),
        ("evap_srfc_resistance_option", 1),
        ("subsurface_option", 1),
    ] {
        let mut got = -999;
        g.get_int(key, &mut got).unwrap();
        assert_eq!(got, want, "{key}");
    }
}

#[test]
fn namelist_input_location_and_forcing() {
    let nml = Namelist::parse(&fixture("namelist.input")).unwrap();

    let loc = nml.group("location").unwrap();
    let (mut lat, mut lon) = (0.0f32, 0.0f32);
    loc.get_real("lat", &mut lat).unwrap();
    loc.get_real("lon", &mut lon).unwrap();
    assert_eq!(lat, 40.01f32);
    assert_eq!(lon, -88.37f32);

    let f = nml.group("forcing").unwrap();
    let (mut zref, mut thresh) = (0.0f32, 0.0f32);
    f.get_real("ZREF", &mut zref).unwrap(); // key lookup is case-insensitive
    f.get_real("rain_snow_thresh", &mut thresh).unwrap();
    assert_eq!(zref, 10.0);
    assert_eq!(thresh, 1.0);
}

// ---------------------------------------------------------------- MPTABLE.TBL

#[test]
fn mptable_parses_and_has_expected_groups() {
    let nml = Namelist::parse(&fixture("MPTABLE.TBL")).unwrap();
    for g in [
        "usgs_veg_categories",
        "usgs_veg_parameters",
        "modis_veg_categories",
        "modis_veg_parameters",
        "rad_parameters",
        "global_parameters",
        "crop_parameters",
        "irrigation_parameters",
        "tiledrain_parameters",
        "optional_parameters",
    ] {
        assert!(nml.has_group(g), "missing group &{g}");
    }
}

#[test]
fn mptable_veg_categories() {
    let nml = Namelist::parse(&fixture("MPTABLE.TBL")).unwrap();

    let usgs = nml.group("usgs_veg_categories").unwrap();
    let mut nveg = 0;
    let mut desc = String::new();
    usgs.get_int("NVEG", &mut nveg).unwrap();
    usgs.get_string("VEG_DATASET_DESCRIPTION", &mut desc).unwrap();
    assert_eq!(nveg, 27);
    assert_eq!(desc, "USGS");

    let modis = nml.group("modis_veg_categories").unwrap();
    let mut nveg_m = 0;
    modis.get_int("NVEG", &mut nveg_m).unwrap();
    assert_eq!(nveg_m, 20);
}

#[test]
fn mptable_usgs_index_parameters() {
    let nml = Namelist::parse(&fixture("MPTABLE.TBL")).unwrap();
    let g = nml.group("usgs_veg_parameters").unwrap();

    for (key, want) in [
        ("ISURBAN", 1),
        ("ISWATER", 16),
        ("ISBARREN", 19),
        ("ISICE", 24),
        ("ISCROP", 2),
        ("EBLFOREST", 13),
    ] {
        let mut got = -999;
        g.get_int(key, &mut got).unwrap();
        assert_eq!(got, want, "{key}");
    }
}

#[test]
fn mptable_veg_arrays_are_full_length() {
    // MVT = 27. A veg parameter row must fill all 27 slots, not stop at the first line.
    let nml = Namelist::parse(&fixture("MPTABLE.TBL")).unwrap();
    let g = nml.group("usgs_veg_parameters").unwrap();

    const SENTINEL: f32 = -1.0E36;
    for key in ["CH2OP", "DLEAF", "Z0MVT", "HVT", "HVB", "RC", "MFSNO"] {
        let mut arr = [SENTINEL; 27];
        g.get_real_array(key, &mut arr).unwrap();
        let unset = arr.iter().filter(|&&v| v == SENTINEL).count();
        assert_eq!(unset, 0, "{key} left {unset} of 27 entries unset");
    }
}

#[test]
fn mptable_monthly_arrays_assemble_to_27x12() {
    // SAIM_TABLE(MVT,12) is not a single namelist key. ParametersRead.f90:535-546 builds it
    // from twelve monthly vectors: SAIM_TABLE(1:NVEG, 1) = SAI_JAN(1:NVEG), and so on.
    // This test performs that assembly, which is what the port will have to do.
    const MONTHS: [&str; 12] = [
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];
    const MVT: usize = 27;
    const SENTINEL: f32 = -1.0E36;

    let nml = Namelist::parse(&fixture("MPTABLE.TBL")).unwrap();
    let g = nml.group("usgs_veg_parameters").unwrap();
    let nveg = {
        let mut n = 0;
        nml.group("usgs_veg_categories").unwrap().get_int("NVEG", &mut n).unwrap();
        n as usize
    };
    assert_eq!(nveg, 27);

    for prefix in ["SAI", "LAI"] {
        // Column-major, as Fortran declares it: index (v, m) lives at v + m * MVT.
        let mut table = [SENTINEL; MVT * 12];
        for (m, month) in MONTHS.iter().enumerate() {
            let key = format!("{prefix}_{month}");
            let mut col = [SENTINEL; MVT];
            g.get_real_array(&key, &mut col[..nveg])
                .unwrap_or_else(|e| panic!("{key}: {e}"));
            assert!(
                col[..nveg].iter().all(|&v| v != SENTINEL),
                "{key} did not fill all {nveg} veg entries"
            );
            table[m * MVT..m * MVT + MVT].copy_from_slice(&col);
        }
        assert!(
            table.iter().all(|&v| v != SENTINEL),
            "{prefix}M_TABLE has unset entries after assembly"
        );
    }

    // Spot-check against the file: SAI_JAN veg type 2 is 0.3, LAI_JUN veg type 13 is 4.5.
    let mut sai_jan = [SENTINEL; MVT];
    g.get_real_array("SAI_JAN", &mut sai_jan).unwrap();
    assert_eq!(sai_jan[0], 0.0);
    assert_eq!(sai_jan[1], 0.3);

    let mut lai_jun = [SENTINEL; MVT];
    g.get_real_array("LAI_JUN", &mut lai_jun).unwrap();
    assert_eq!(lai_jun[12], 4.5);
}

#[test]
fn mptable_trailing_commas_do_not_add_values() {
    // Every veg row in MPTABLE.TBL ends with a trailing comma. It must not produce a 28th
    // (null) entry that shifts the next key's data.
    let nml = Namelist::parse(&fixture("MPTABLE.TBL")).unwrap();
    let g = nml.group("usgs_veg_parameters").unwrap();
    let e = g.entry("SAI_JAN").unwrap();
    assert_eq!(e.tokens.len(), 27, "SAI_JAN token count");
}

#[test]
fn mptable_global_and_rad_parameters() {
    let nml = Namelist::parse(&fixture("MPTABLE.TBL")).unwrap();

    let g = nml.group("global_parameters").unwrap();
    let (mut co2, mut o2) = (0.0f32, 0.0f32);
    g.get_real("CO2", &mut co2).unwrap();
    g.get_real("O2", &mut o2).unwrap();
    assert!(co2 > 0.0, "CO2 should be set, got {co2}");
    assert!(o2 > 0.0, "O2 should be set, got {o2}");

    let r = nml.group("rad_parameters").unwrap();
    let mut albsat_vis = [-1.0E36f32; 8]; // MSC = 8
    r.get_real_array("ALBSAT_VIS", &mut albsat_vis).unwrap();
    assert!(
        albsat_vis.iter().all(|&v| v != -1.0E36),
        "ALBSAT_VIS not fully populated: {albsat_vis:?}"
    );
}

#[test]
fn mptable_sentinels_survive_for_absent_keys() {
    // The property ParametersRead relies on: a key the file does not mention keeps -1.E36.
    let nml = Namelist::parse(&fixture("MPTABLE.TBL")).unwrap();
    let g = nml.group("global_parameters").unwrap();
    let mut never_present = -1.0E36f32;
    g.get_real("THIS_KEY_DOES_NOT_EXIST", &mut never_present).unwrap();
    assert_eq!(never_present, -1.0E36f32);
}

// ---------------------------------------------------------------- SOILPARM.TBL

#[test]
fn soilparm_stas_block() {
    // Mirrors read_soil_parameters: skip to the class block, read the count, then the rows.
    let src = fixture("SOILPARM.TBL");
    let mut r = ListDirectedReader::new(&src);

    assert!(r.find_block("STAS", 100).is_some(), "STAS block not found");

    let slcats = r.read_int().unwrap();
    assert_eq!(slcats, 19);

    // Row 1 is SAND; check the values the Fortran assigns, in order.
    let mut row = [0.0f32; 18]; // ITMP is read as a real here then discarded
    r.read_reals(&mut row).unwrap();
    assert_eq!(row[0], 1.0); // ITMP
    assert_eq!(row[1], 2.79); // BEXP
    assert_eq!(row[2], 0.010); // SMCDRY
    assert_eq!(row[3], -0.472); // F11
    assert_eq!(row[4], 0.339); // SMCMAX
    assert_eq!(row[5], 0.192); // SMCREF
    assert_eq!(row[6], 0.069); // PSISAT
    assert_eq!(row[7], 4.66E-5); // DKSAT
    assert_eq!(row[8], 2.65E-5); // DWSAT
    assert_eq!(row[9], 0.010); // SMCWLT
    assert_eq!(row[10], 0.92); // QTZ
}

#[test]
fn soilparm_all_rows_readable() {
    let src = fixture("SOILPARM.TBL");
    let mut r = ListDirectedReader::new(&src);
    r.find_block("STAS", 100).unwrap();
    let slcats = r.read_int().unwrap();

    for lc in 1..=slcats {
        let mut row = [0.0f32; 18];
        r.read_reals(&mut row)
            .unwrap_or_else(|e| panic!("row {lc}: {e}"));
        assert_eq!(row[0], lc as f32, "row {lc} index column");
        // SMCMAX must be a sane porosity for every class.
        assert!(
            row[4] >= 0.0 && row[4] <= 1.0,
            "row {lc} SMCMAX out of range: {}",
            row[4]
        );
    }
}

#[test]
fn soilparm_stas_ruc_block_also_present() {
    let src = fixture("SOILPARM.TBL");
    let mut r = ListDirectedReader::new(&src);
    assert!(
        r.find_block("STAS-RUC", 100).is_some(),
        "STAS-RUC block not found"
    );
    let slcats = r.read_int().unwrap();
    assert!(slcats > 0, "STAS-RUC category count: {slcats}");
}

// ---------------------------------------------------------------- GENPARM.TBL

#[test]
fn genparm_slope_and_scalars() {
    // Mirrors the second half of read_soil_parameters.
    let src = fixture("GENPARM.TBL");
    let mut r = ListDirectedReader::new(&src);

    r.skip_record(); // "General Parameters"
    assert_eq!(r.read_string().unwrap(), "SLOPE_DATA");

    let num_slope = r.read_int().unwrap();
    assert_eq!(num_slope, 9);

    let mut slope = vec![0.0f32; num_slope as usize];
    for (i, s) in slope.iter_mut().enumerate() {
        *s = r.read_real().unwrap_or_else(|e| panic!("slope {i}: {e}"));
    }
    assert_eq!(slope[0], 0.1f32);
    assert_eq!(slope[1], 0.6f32);
    assert_eq!(slope[2], 1.0f32);
    assert_eq!(slope[3], 0.35f32);

    // Then label/value pairs.
    for (label, want) in [
        ("SBETA_DATA", -2.0f32),
        ("FXEXP_DATA", 2.0),
        ("CSOIL_DATA", 2.00E+6),
        ("SALP_DATA", 2.6),
        ("REFDK_DATA", 2.0E-6),
        ("REFKDT_DATA", 3.0),
    ] {
        assert_eq!(r.read_string().unwrap(), label);
        assert_eq!(r.read_real().unwrap(), want, "{label}");
    }
}
