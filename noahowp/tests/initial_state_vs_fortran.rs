//! The model's initial state, against the Fortran's.
//!
//! `initialize_from_file` builds the types and then overwrites a long list of state that the
//! `*Type` modules had just set to `huge(1.0)`. A missed line there leaves a sentinel in place
//! and would not surface until the physics divided by it. This compares every recorded field of
//! `domain`, `forcing`, `energy` and `water` against the reference Fortran's own state at the
//! first timestep, bit for bit.
//!
//! Three groups of fields are excluded, each for a reason that is part of the port's scope:
//!
//! - the eight forcings `read_forcing_text` supplies from `bondville.dat`. On the BMI path
//!   they arrive through `set_value`, and the ASCII reader is deliberately not ported.
//! - `domain%curr_datetime`, assigned at the top of `solve_noahowp` -- after initialisation,
//!   but before the record is written.
//! - `domain%sim_datetimes`, which the fixtures omit as one f64 per timestep of the run. Its
//!   length is checked through `ntime` instead.

use noahowp::difftest::{Call, Fixture, Phase, State};
use noahowp::namelist_read::NamelistConfig;
use noahowp::parameters_read::Tables;
use noahowp::run::NoahOwp;

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!("reading {path}: {e}\nregenerate with reference/build.sh --fixtures")
    })
}

fn fixture_text(name: &str) -> String {
    String::from_utf8(fixture_bytes(name)).expect("utf-8 fixture")
}

fn model() -> NoahOwp {
    let namelist = NamelistConfig::parse(&fixture_text("namelist.input")).expect("namelist");
    let tables = Tables::parse(
        &fixture_text("MPTABLE.TBL"),
        &fixture_text("SOILPARM.TBL"),
        &fixture_text("GENPARM.TBL"),
        &namelist,
    )
    .expect("tables");
    NoahOwp::new(namelist, &tables).expect("model")
}

#[derive(Default)]
struct Report(Vec<String>);

impl Report {
    fn f32(&mut self, s: &State, name: &str, got: f32) {
        let want = s.f32(name);
        if got.to_bits() != want.to_bits() {
            self.0.push(format!("{}%{name}: fortran {want:e}, rust {got:e}", s.type_name));
        }
    }
    fn f64(&mut self, s: &State, name: &str, got: f64) {
        let want = s.f64(name);
        if got.to_bits() != want.to_bits() {
            self.0.push(format!("{}%{name}: fortran {want:e}, rust {got:e}", s.type_name));
        }
    }
    fn i32(&mut self, s: &State, name: &str, got: i32) {
        let want = s.i32(name);
        if got != want {
            self.0.push(format!("{}%{name}: fortran {want}, rust {got}", s.type_name));
        }
    }
    fn bool(&mut self, s: &State, name: &str, got: bool) {
        let want = s.bool(name);
        if got != want {
            self.0.push(format!("{}%{name}: fortran {want}, rust {got}", s.type_name));
        }
    }
    fn str(&mut self, s: &State, name: &str, got: &str) {
        let want = s.str(name);
        if got != want {
            self.0.push(format!("{}%{name}: fortran {want:?}, rust {got:?}", s.type_name));
        }
    }
    fn f32s(&mut self, s: &State, name: &str, got: &[f32]) {
        let want = s.f32s(name);
        if got.len() != want.len() {
            self.0.push(format!(
                "{}%{name}: fortran has {} elements, rust {}",
                s.type_name, want.len(), got.len()
            ));
            return;
        }
        for (i, (g, w)) in got.iter().zip(&want).enumerate() {
            if g.to_bits() != w.to_bits() {
                self.0.push(format!(
                    "{}%{name}({}): fortran {w:e}, rust {g:e}",
                    s.type_name, i + 1
                ));
            }
        }
    }
    fn i32s(&mut self, s: &State, name: &str, got: &[i32]) {
        let want = s.i32s(name);
        for (i, (g, w)) in got.iter().zip(&want).enumerate() {
            if g != w {
                self.0.push(format!("{}%{name}({}): fortran {w}, rust {g}", s.type_name, i + 1));
            }
        }
    }
    fn finish(self) {
        assert!(
            self.0.is_empty(),
            "{} field(s) differ from the Fortran at the first timestep:\n  {}",
            self.0.len(),
            self.0.join("\n  ")
        );
    }
}

#[test]
fn initial_state_matches_the_fortran() {
    let m = model();
    let recording = Fixture::parse(&fixture_bytes("difftest/bondville.difftest")).expect("fixture");
    let first = recording
        .records
        .iter()
        .find(|r| r.call == Call::Utilities && r.phase == Phase::Before)
        .expect("a record before the first physics call");
    assert_eq!(first.itime, 1, "the first record should be the first timestep");

    let mut r = Report::default();


    let s = first.state("domain");
    r.i32(s, "iloc", m.domain.iloc);
    r.i32(s, "jloc", m.domain.jloc);
    r.f32(s, "DT", m.domain.dt);
    r.str(s, "startdate", &m.domain.startdate);
    r.str(s, "enddate", &m.domain.enddate);
    r.str(s, "nowdate", &m.domain.nowdate);
    r.f64(s, "start_datetime", m.domain.start_datetime);
    r.f64(s, "end_datetime", m.domain.end_datetime);
    r.i32(s, "itime", m.domain.itime);
    r.i32(s, "ntime", m.domain.ntime);
    r.f64(s, "time_dbl", m.domain.time_dbl);
    r.f32(s, "lat", m.domain.lat);
    r.f32(s, "lon", m.domain.lon);
    r.f32(s, "ZREF", m.domain.zref);
    r.f32(s, "terrain_slope", m.domain.terrain_slope);
    r.f32(s, "azimuth", m.domain.azimuth);
    r.i32(s, "vegtyp", m.domain.vegtyp);
    r.i32(s, "croptype", m.domain.croptype);
    r.i32(s, "isltyp", m.domain.isltyp);
    r.i32(s, "IST", m.domain.ist);
    r.f32s(s, "zsoil", m.domain.zsoil.as_slice());
    r.f32s(s, "dzsnso", m.domain.dzsnso.as_slice());
    r.f32s(s, "zsnso", m.domain.zsnso.as_slice());

    let s = first.state("forcing");
    r.f32(s, "PRCPCONV", m.forcing.PRCPCONV);
    r.f32(s, "PRCPNONC", m.forcing.PRCPNONC);
    r.f32(s, "PRCPSHCV", m.forcing.PRCPSHCV);
    r.f32(s, "PRCPSNOW", m.forcing.PRCPSNOW);
    r.f32(s, "PRCPGRPL", m.forcing.PRCPGRPL);
    r.f32(s, "PRCPHAIL", m.forcing.PRCPHAIL);
    r.f32(s, "FOLN", m.forcing.FOLN);
    r.f32(s, "O2PP", m.forcing.O2PP);
    r.f32(s, "CO2PP", m.forcing.CO2PP);
    r.f32(s, "TBOT", m.forcing.TBOT);
    r.f32(s, "UR", m.forcing.UR);
    r.f32(s, "THAIR", m.forcing.THAIR);
    r.f32(s, "QAIR", m.forcing.QAIR);
    r.f32(s, "EAIR", m.forcing.EAIR);
    r.f32(s, "RHOAIR", m.forcing.RHOAIR);
    r.f32(s, "FPICE", m.forcing.FPICE);
    r.f32(s, "SWDOWN", m.forcing.SWDOWN);
    r.f32(s, "JULIAN", m.forcing.JULIAN);
    r.i32(s, "YEARLEN", m.forcing.YEARLEN);
    r.f32s(s, "SOLAD", m.forcing.SOLAD.as_slice());
    r.f32s(s, "SOLAI", m.forcing.SOLAI.as_slice());

    let s = first.state("energy");
    r.f32(s, "TV", m.energy.TV);
    r.f32(s, "TG", m.energy.TG);
    r.f32(s, "FCEV", m.energy.FCEV);
    r.f32(s, "FCTR", m.energy.FCTR);
    r.f32(s, "IGS", m.energy.IGS);
    r.bool(s, "FROZEN_CANOPY", m.energy.FROZEN_CANOPY);
    r.bool(s, "FROZEN_GROUND", m.energy.FROZEN_GROUND);
    r.i32s(s, "IMELT", m.energy.IMELT.as_slice());
    r.f32s(s, "STC", m.energy.STC.as_slice());
    r.f32s(s, "DF", m.energy.DF.as_slice());
    r.f32s(s, "HCPCT", m.energy.HCPCT.as_slice());
    r.f32s(s, "FACT", m.energy.FACT.as_slice());
    r.f32(s, "SNOWT_AVG", m.energy.SNOWT_AVG);
    r.f32(s, "PAHV", m.energy.PAHV);
    r.f32(s, "PAHG", m.energy.PAHG);
    r.f32(s, "PAHB", m.energy.PAHB);
    r.f32(s, "PAH", m.energy.PAH);
    r.f32(s, "TAUSS", m.energy.TAUSS);
    r.f32(s, "FAGE", m.energy.FAGE);
    r.f32(s, "ALB", m.energy.ALB);
    r.f32(s, "ALBOLD", m.energy.ALBOLD);
    r.f32s(s, "ALBD", m.energy.ALBD.as_slice());
    r.f32s(s, "ALBI", m.energy.ALBI.as_slice());
    r.f32s(s, "ALBGRD", m.energy.ALBGRD.as_slice());
    r.f32s(s, "ALBGRI", m.energy.ALBGRI.as_slice());
    r.f32s(s, "ALBSND", m.energy.ALBSND.as_slice());
    r.f32s(s, "ALBSNI", m.energy.ALBSNI.as_slice());
    r.f32s(s, "FABD", m.energy.FABD.as_slice());
    r.f32s(s, "FABI", m.energy.FABI.as_slice());
    r.f32s(s, "FTDD", m.energy.FTDD.as_slice());
    r.f32s(s, "FTDI", m.energy.FTDI.as_slice());
    r.f32s(s, "FTID", m.energy.FTID.as_slice());
    r.f32s(s, "FTII", m.energy.FTII.as_slice());
    r.f32s(s, "FREVD", m.energy.FREVD.as_slice());
    r.f32s(s, "FREVI", m.energy.FREVI.as_slice());
    r.f32s(s, "FREGD", m.energy.FREGD.as_slice());
    r.f32s(s, "FREGI", m.energy.FREGI.as_slice());
    r.f32s(s, "RHO", m.energy.RHO.as_slice());
    r.f32s(s, "TAU", m.energy.TAU.as_slice());
    r.f32(s, "COSZ", m.energy.COSZ);
    r.f32(s, "COSZ_HORIZ", m.energy.COSZ_HORIZ);
    r.f32(s, "BGAP", m.energy.BGAP);
    r.f32(s, "WGAP", m.energy.WGAP);
    r.f32(s, "FSUN", m.energy.FSUN);
    r.f32(s, "FSHA", m.energy.FSHA);
    r.f32(s, "LAISUN", m.energy.LAISUN);
    r.f32(s, "LAISHA", m.energy.LAISHA);
    r.f32(s, "PARSUN", m.energy.PARSUN);
    r.f32(s, "PARSHA", m.energy.PARSHA);
    r.f32(s, "SAV", m.energy.SAV);
    r.f32(s, "SAG", m.energy.SAG);
    r.f32(s, "FSA", m.energy.FSA);
    r.f32(s, "FSR", m.energy.FSR);
    r.f32(s, "FSRV", m.energy.FSRV);
    r.f32(s, "FSRG", m.energy.FSRG);
    r.f32(s, "TAH", m.energy.TAH);
    r.f32(s, "EAH", m.energy.EAH);
    r.f32(s, "ZPD", m.energy.ZPD);
    r.f32(s, "Z0MG", m.energy.Z0MG);
    r.f32(s, "Z0M", m.energy.Z0M);
    r.f32(s, "ZLVL", m.energy.ZLVL);
    r.f32(s, "CMV", m.energy.CMV);
    r.f32(s, "CMB", m.energy.CMB);
    r.f32(s, "CM", m.energy.CM);
    r.f32(s, "CH", m.energy.CH);
    r.f32(s, "TGB", m.energy.TGB);
    r.f32(s, "QSFC", m.energy.QSFC);
    r.f32(s, "EMV", m.energy.EMV);
    r.f32(s, "EMG", m.energy.EMG);
    r.f32(s, "GAMMAV", m.energy.GAMMAV);
    r.f32(s, "GAMMAG", m.energy.GAMMAG);
    r.f32(s, "EVC", m.energy.EVC);
    r.f32(s, "IRC", m.energy.IRC);
    r.f32(s, "IRG", m.energy.IRG);
    r.f32(s, "SHC", m.energy.SHC);
    r.f32(s, "SHG", m.energy.SHG);
    r.f32(s, "SHB", m.energy.SHB);
    r.f32(s, "EVG", m.energy.EVG);
    r.f32(s, "EVB", m.energy.EVB);
    r.f32(s, "TR", m.energy.TR);
    r.f32(s, "GH", m.energy.GH);
    r.f32(s, "GHB", m.energy.GHB);
    r.f32(s, "GHV", m.energy.GHV);
    r.f32(s, "T2MV", m.energy.T2MV);
    r.f32(s, "CHLEAF", m.energy.CHLEAF);
    r.f32(s, "CHUC", m.energy.CHUC);
    r.f32(s, "CHV2", m.energy.CHV2);
    r.f32(s, "CHB2", m.energy.CHB2);
    r.f32(s, "Q2V", m.energy.Q2V);
    r.f32(s, "LATHEAV", m.energy.LATHEAV);
    r.f32(s, "LATHEAG", m.energy.LATHEAG);
    r.f32(s, "LATHEA", m.energy.LATHEA);
    r.f32(s, "RSURF", m.energy.RSURF);
    r.f32(s, "RHSUR", m.energy.RHSUR);
    r.f32(s, "TAUXV", m.energy.TAUXV);
    r.f32(s, "TAUYV", m.energy.TAUYV);
    r.f32(s, "TAUXB", m.energy.TAUXB);
    r.f32(s, "TAUYB", m.energy.TAUYB);
    r.f32(s, "TAUX", m.energy.TAUX);
    r.f32(s, "TAUY", m.energy.TAUY);
    r.f32(s, "CAH2", m.energy.CAH2);
    r.f32(s, "EHB2", m.energy.EHB2);
    r.f32(s, "T2MB", m.energy.T2MB);
    r.f32(s, "Q2B", m.energy.Q2B);
    r.f32(s, "TGV", m.energy.TGV);
    r.f32(s, "CHV", m.energy.CHV);
    r.f32(s, "RSSUN", m.energy.RSSUN);
    r.f32(s, "RSSHA", m.energy.RSSHA);
    r.f32(s, "RB", m.energy.RB);
    r.f32(s, "FIRA", m.energy.FIRA);
    r.f32(s, "FSH", m.energy.FSH);
    r.f32(s, "FGEV", m.energy.FGEV);
    r.f32(s, "TRAD", m.energy.TRAD);
    r.f32(s, "IRB", m.energy.IRB);
    r.f32(s, "SSOIL", m.energy.SSOIL);
    r.f32(s, "T2M", m.energy.T2M);
    r.f32(s, "TS", m.energy.TS);
    r.f32(s, "CHB", m.energy.CHB);
    r.f32(s, "Q1", m.energy.Q1);
    r.f32(s, "Q2E", m.energy.Q2E);
    r.f32(s, "Z0WRF", m.energy.Z0WRF);
    r.f32(s, "EMISSI", m.energy.EMISSI);
    r.f32(s, "PSN", m.energy.PSN);
    r.f32(s, "PSNSUN", m.energy.PSNSUN);
    r.f32(s, "PSNSHA", m.energy.PSNSHA);
    r.f32(s, "APAR", m.energy.APAR);
    r.f32(s, "QMELT", m.energy.QMELT);
    r.f32(s, "LH", m.energy.LH);
    r.f32(s, "TGS", m.energy.TGS);
    r.i32(s, "ICE", m.energy.ICE);

    let s = first.state("water");
    r.f32(s, "qinsur", m.water.qinsur);
    r.f32(s, "qseva", m.water.qseva);
    r.f32(s, "EVAPOTRANS", m.water.EVAPOTRANS);
    r.f32(s, "runsrf", m.water.runsrf);
    r.f32(s, "runsub", m.water.runsub);
    r.f32(s, "qdrain", m.water.qdrain);
    r.f32(s, "zwt", m.water.zwt);
    r.f32(s, "smcwtd", m.water.smcwtd);
    r.f32(s, "deeprech", m.water.deeprech);
    r.f32(s, "fcrmax", m.water.fcrmax);
    r.f32(s, "snoflow", m.water.snoflow);
    r.f32(s, "pddum", m.water.pddum);
    r.f32(s, "FACC", m.water.FACC);
    r.f32(s, "sicemax", m.water.sicemax);
    r.f32(s, "FB_snow", m.water.FB_snow);
    r.f32(s, "rain", m.water.rain);
    r.f32(s, "snow", m.water.snow);
    r.f32(s, "bdfall", m.water.bdfall);
    r.f32(s, "FP", m.water.FP);
    r.f32(s, "canliq", m.water.canliq);
    r.f32(s, "canice", m.water.canice);
    r.f32(s, "FWET", m.water.FWET);
    r.f32(s, "CMC", m.water.CMC);
    r.f32(s, "QINTR", m.water.QINTR);
    r.f32(s, "QDRIPR", m.water.QDRIPR);
    r.f32(s, "QTHROR", m.water.QTHROR);
    r.f32(s, "QINTS", m.water.QINTS);
    r.f32(s, "QDRIPS", m.water.QDRIPS);
    r.f32(s, "QTHROS", m.water.QTHROS);
    r.f32(s, "QRAIN", m.water.QRAIN);
    r.f32(s, "QSNOW", m.water.QSNOW);
    r.f32(s, "SNOWHIN", m.water.SNOWHIN);
    r.f32(s, "ECAN", m.water.ECAN);
    r.f32(s, "ETRAN", m.water.ETRAN);
    r.f32(s, "QSNFRO", m.water.QSNFRO);
    r.f32(s, "QSNSUB", m.water.QSNSUB);
    r.f32(s, "SNOWH", m.water.SNOWH);
    r.f32(s, "SNEQV", m.water.SNEQV);
    r.f32(s, "SNEQVO", m.water.SNEQVO);
    r.f32(s, "BDSNO", m.water.BDSNO);
    r.f32(s, "QSNBOT", m.water.QSNBOT);
    r.f32(s, "PONDING", m.water.PONDING);
    r.f32(s, "PONDING1", m.water.PONDING1);
    r.f32(s, "PONDING2", m.water.PONDING2);
    r.f32(s, "QVAP", m.water.QVAP);
    r.f32(s, "QDEW", m.water.QDEW);
    r.f32(s, "QSDEW", m.water.QSDEW);
    r.f32(s, "WSLAKE", m.water.WSLAKE);
    r.f32(s, "runsrf_dt", m.water.runsrf_dt);
    r.f32(s, "ASAT", m.water.ASAT);
    r.f32(s, "ACSNOM", m.water.ACSNOM);
    r.i32(s, "ISNOW", m.water.ISNOW);
    r.f32s(s, "smc", m.water.smc.as_slice());
    r.f32s(s, "smc_init", m.water.smc_init.as_slice());
    r.f32s(s, "sice", m.water.sice.as_slice());
    r.f32s(s, "sh2o", m.water.sh2o.as_slice());
    r.f32s(s, "etrani", m.water.etrani.as_slice());
    r.f32s(s, "BTRANI", m.water.BTRANI.as_slice());
    r.f32s(s, "wcnd", m.water.wcnd.as_slice());
    r.f32s(s, "fcr", m.water.fcr.as_slice());
    r.f32s(s, "FICEOLD", m.water.FICEOLD.as_slice());
    r.f32s(s, "SNICE", m.water.SNICE.as_slice());
    r.f32s(s, "SNLIQ", m.water.SNLIQ.as_slice());
    r.f32s(s, "SNICEV", m.water.SNICEV.as_slice());
    r.f32s(s, "SNLIQV", m.water.SNLIQV.as_slice());
    r.f32s(s, "FICE", m.water.FICE.as_slice());
    r.f32s(s, "EPORE", m.water.EPORE.as_slice());
    r.f32(s, "FSNO", m.water.FSNO);
    r.f32(s, "BTRAN", m.water.BTRAN);

    r.finish();
}
