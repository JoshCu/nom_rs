//! Binding between the fixture records and the model structs, field by field.
//!
//! One list of fields, walked in two directions: [`Load`] copies a recorded state into a
//! [`NoahOwp`], [`Compare`] reports the fields where the model disagrees with a recorded state.
//! The list covers every field the fixtures record for `domain`, `forcing`, `energy`, `water`
//! and `paramstate` (except `domain%sim_datetimes`, which the fixtures omit), so a physics
//! test can replay a call from the complete pre-state and hold the complete post-state to
//! account -- including the fields the port did not know the Fortran touched.
//!
//! Generated from `manifest.rs` by hand-run script; regenerate if the manifest changes.

use crate::difftest::{Record, State};
use crate::layers::Shifted;
use crate::run::NoahOwp;

/// One direction of the walk.
pub trait Binder {
    fn f32(&mut self, s: &State, name: &str, v: &mut f32);
    fn f64(&mut self, s: &State, name: &str, v: &mut f64);
    fn i32(&mut self, s: &State, name: &str, v: &mut i32);
    fn bool(&mut self, s: &State, name: &str, v: &mut bool);
    fn string(&mut self, s: &State, name: &str, v: &mut String);
    fn f32s(&mut self, s: &State, name: &str, v: &mut Shifted<f32>);
    fn i32s(&mut self, s: &State, name: &str, v: &mut Shifted<i32>);
}

/// Copies the record into the model.
pub struct Load;

impl Binder for Load {
    fn f32(&mut self, s: &State, name: &str, v: &mut f32) {
        *v = s.f32(name);
    }
    fn f64(&mut self, s: &State, name: &str, v: &mut f64) {
        *v = s.f64(name);
    }
    fn i32(&mut self, s: &State, name: &str, v: &mut i32) {
        *v = s.i32(name);
    }
    fn bool(&mut self, s: &State, name: &str, v: &mut bool) {
        *v = s.bool(name);
    }
    fn string(&mut self, s: &State, name: &str, v: &mut String) {
        *v = s.str(name);
    }
    fn f32s(&mut self, s: &State, name: &str, v: &mut Shifted<f32>) {
        let want = s.f32s(name);
        assert_eq!(v.len(), want.len(), "{}%{name}: length", s.type_name);
        v.as_mut_slice().copy_from_slice(&want);
    }
    fn i32s(&mut self, s: &State, name: &str, v: &mut Shifted<i32>) {
        let want = s.i32s(name);
        assert_eq!(v.len(), want.len(), "{}%{name}: length", s.type_name);
        v.as_mut_slice().copy_from_slice(&want);
    }
}

/// Collects the fields where the model and the record disagree, bit for bit.
#[derive(Default)]
pub struct Compare {
    pub failures: Vec<String>,
    /// `type%field` names to leave out of the comparison.
    pub ignore: Vec<String>,
}

impl Compare {
    pub fn ignoring<'a>(names: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            failures: Vec::new(),
            ignore: names.into_iter().map(str::to_string).collect(),
        }
    }

    fn ignored(&self, s: &State, name: &str) -> bool {
        self.ignore
            .iter()
            .any(|n| n.eq_ignore_ascii_case(&format!("{}%{name}", s.type_name)))
    }
}

impl Binder for Compare {
    fn f32(&mut self, s: &State, name: &str, v: &mut f32) {
        if self.ignored(s, name) {
            return;
        }
        let want = s.f32(name);
        if v.to_bits() != want.to_bits() {
            self.failures.push(format!(
                "{}%{name}: fortran {want:e} ({:08X}), rust {:e} ({:08X})",
                s.type_name,
                want.to_bits(),
                *v,
                v.to_bits()
            ));
        }
    }
    fn f64(&mut self, s: &State, name: &str, v: &mut f64) {
        if self.ignored(s, name) {
            return;
        }
        let want = s.f64(name);
        if v.to_bits() != want.to_bits() {
            self.failures.push(format!("{}%{name}: fortran {want:e}, rust {:e}", s.type_name, *v));
        }
    }
    fn i32(&mut self, s: &State, name: &str, v: &mut i32) {
        if self.ignored(s, name) {
            return;
        }
        let want = s.i32(name);
        if *v != want {
            self.failures.push(format!("{}%{name}: fortran {want}, rust {}", s.type_name, *v));
        }
    }
    fn bool(&mut self, s: &State, name: &str, v: &mut bool) {
        if self.ignored(s, name) {
            return;
        }
        let want = s.bool(name);
        if *v != want {
            self.failures.push(format!("{}%{name}: fortran {want}, rust {}", s.type_name, *v));
        }
    }
    fn string(&mut self, s: &State, name: &str, v: &mut String) {
        if self.ignored(s, name) {
            return;
        }
        let want = s.str(name);
        if *v != want {
            self.failures.push(format!("{}%{name}: fortran {want:?}, rust {:?}", s.type_name, *v));
        }
    }
    fn f32s(&mut self, s: &State, name: &str, v: &mut Shifted<f32>) {
        if self.ignored(s, name) {
            return;
        }
        let want = s.f32s(name);
        if v.len() != want.len() {
            self.failures.push(format!(
                "{}%{name}: fortran has {} elements, rust {}",
                s.type_name,
                want.len(),
                v.len()
            ));
            return;
        }
        for (i, (g, w)) in v.iter().zip(&want).enumerate() {
            if g.to_bits() != w.to_bits() {
                self.failures.push(format!(
                    "{}%{name}({}): fortran {w:e} ({:08X}), rust {g:e} ({:08X})",
                    s.type_name,
                    v.lo() + i as i32,
                    w.to_bits(),
                    g.to_bits()
                ));
            }
        }
    }
    fn i32s(&mut self, s: &State, name: &str, v: &mut Shifted<i32>) {
        if self.ignored(s, name) {
            return;
        }
        let want = s.i32s(name);
        for (i, (g, w)) in v.iter().zip(&want).enumerate() {
            if g != w {
                self.failures.push(format!(
                    "{}%{name}({}): fortran {w}, rust {g}",
                    s.type_name,
                    v.lo() + i as i32
                ));
            }
        }
    }
}

/// Load every recorded field of `rec` into `m`.
pub fn load(m: &mut NoahOwp, rec: &Record) {
    walk(&mut Load, m, rec);
}

/// The fields of `m` that differ from `rec`, as `type%field: fortran ..., rust ...` lines.
pub fn compare(m: &mut NoahOwp, rec: &Record, ignore: &[&str]) -> Vec<String> {
    let mut c = Compare::ignoring(ignore.iter().copied());
    walk(&mut c, m, rec);
    c.failures
}

/// Every recorded field, in manifest order.
pub fn walk<B: Binder>(b: &mut B, m: &mut NoahOwp, rec: &Record) {
    let s = rec.state("domain");
    b.i32(s, "iloc", &mut m.domain.iloc);
    b.i32(s, "jloc", &mut m.domain.jloc);
    b.f32(s, "DT", &mut m.domain.dt);
    b.string(s, "startdate", &mut m.domain.startdate);
    b.string(s, "enddate", &mut m.domain.enddate);
    b.string(s, "nowdate", &mut m.domain.nowdate);
    b.f64(s, "start_datetime", &mut m.domain.start_datetime);
    b.f64(s, "end_datetime", &mut m.domain.end_datetime);
    b.f64(s, "curr_datetime", &mut m.domain.curr_datetime);
    b.i32(s, "itime", &mut m.domain.itime);
    b.i32(s, "ntime", &mut m.domain.ntime);
    b.f64(s, "time_dbl", &mut m.domain.time_dbl);
    b.f32(s, "lat", &mut m.domain.lat);
    b.f32(s, "lon", &mut m.domain.lon);
    b.f32(s, "ZREF", &mut m.domain.zref);
    b.f32(s, "terrain_slope", &mut m.domain.terrain_slope);
    b.f32(s, "azimuth", &mut m.domain.azimuth);
    b.i32(s, "vegtyp", &mut m.domain.vegtyp);
    b.i32(s, "croptype", &mut m.domain.croptype);
    b.i32(s, "isltyp", &mut m.domain.isltyp);
    b.i32(s, "IST", &mut m.domain.ist);
    b.f32s(s, "zsoil", &mut m.domain.zsoil);
    b.f32s(s, "dzsnso", &mut m.domain.dzsnso);
    b.f32s(s, "zsnso", &mut m.domain.zsnso);
    let s = rec.state("forcing");
    b.f32(s, "SFCPRS", &mut m.forcing.SFCPRS);
    b.f32(s, "SFCTMP", &mut m.forcing.SFCTMP);
    b.f32(s, "Q2", &mut m.forcing.Q2);
    b.f32(s, "PRCP", &mut m.forcing.PRCP);
    b.f32(s, "PRCPCONV", &mut m.forcing.PRCPCONV);
    b.f32(s, "PRCPNONC", &mut m.forcing.PRCPNONC);
    b.f32(s, "PRCPSHCV", &mut m.forcing.PRCPSHCV);
    b.f32(s, "PRCPSNOW", &mut m.forcing.PRCPSNOW);
    b.f32(s, "PRCPGRPL", &mut m.forcing.PRCPGRPL);
    b.f32(s, "PRCPHAIL", &mut m.forcing.PRCPHAIL);
    b.f32(s, "SOLDN", &mut m.forcing.SOLDN);
    b.f32(s, "LWDN", &mut m.forcing.LWDN);
    b.f32(s, "FOLN", &mut m.forcing.FOLN);
    b.f32(s, "O2PP", &mut m.forcing.O2PP);
    b.f32(s, "CO2PP", &mut m.forcing.CO2PP);
    b.f32(s, "UU", &mut m.forcing.UU);
    b.f32(s, "VV", &mut m.forcing.VV);
    b.f32(s, "TBOT", &mut m.forcing.TBOT);
    b.f32(s, "UR", &mut m.forcing.UR);
    b.f32(s, "THAIR", &mut m.forcing.THAIR);
    b.f32(s, "QAIR", &mut m.forcing.QAIR);
    b.f32(s, "EAIR", &mut m.forcing.EAIR);
    b.f32(s, "RHOAIR", &mut m.forcing.RHOAIR);
    b.f32(s, "FPICE", &mut m.forcing.FPICE);
    b.f32(s, "SWDOWN", &mut m.forcing.SWDOWN);
    b.f32(s, "JULIAN", &mut m.forcing.JULIAN);
    b.i32(s, "YEARLEN", &mut m.forcing.YEARLEN);
    b.f32s(s, "SOLAD", &mut m.forcing.SOLAD);
    b.f32s(s, "SOLAI", &mut m.forcing.SOLAI);
    let s = rec.state("energy");
    b.f32(s, "TV", &mut m.energy.TV);
    b.f32(s, "TG", &mut m.energy.TG);
    b.f32(s, "FCEV", &mut m.energy.FCEV);
    b.f32(s, "FCTR", &mut m.energy.FCTR);
    b.f32(s, "IGS", &mut m.energy.IGS);
    b.bool(s, "FROZEN_CANOPY", &mut m.energy.FROZEN_CANOPY);
    b.bool(s, "FROZEN_GROUND", &mut m.energy.FROZEN_GROUND);
    b.i32s(s, "IMELT", &mut m.energy.IMELT);
    b.f32s(s, "STC", &mut m.energy.STC);
    b.f32s(s, "DF", &mut m.energy.DF);
    b.f32s(s, "HCPCT", &mut m.energy.HCPCT);
    b.f32s(s, "FACT", &mut m.energy.FACT);
    b.f32(s, "SNOWT_AVG", &mut m.energy.SNOWT_AVG);
    b.f32(s, "PAHV", &mut m.energy.PAHV);
    b.f32(s, "PAHG", &mut m.energy.PAHG);
    b.f32(s, "PAHB", &mut m.energy.PAHB);
    b.f32(s, "PAH", &mut m.energy.PAH);
    b.f32(s, "TAUSS", &mut m.energy.TAUSS);
    b.f32(s, "FAGE", &mut m.energy.FAGE);
    b.f32(s, "ALB", &mut m.energy.ALB);
    b.f32(s, "ALBOLD", &mut m.energy.ALBOLD);
    b.f32s(s, "ALBD", &mut m.energy.ALBD);
    b.f32s(s, "ALBI", &mut m.energy.ALBI);
    b.f32s(s, "ALBGRD", &mut m.energy.ALBGRD);
    b.f32s(s, "ALBGRI", &mut m.energy.ALBGRI);
    b.f32s(s, "ALBSND", &mut m.energy.ALBSND);
    b.f32s(s, "ALBSNI", &mut m.energy.ALBSNI);
    b.f32s(s, "FABD", &mut m.energy.FABD);
    b.f32s(s, "FABI", &mut m.energy.FABI);
    b.f32s(s, "FTDD", &mut m.energy.FTDD);
    b.f32s(s, "FTDI", &mut m.energy.FTDI);
    b.f32s(s, "FTID", &mut m.energy.FTID);
    b.f32s(s, "FTII", &mut m.energy.FTII);
    b.f32s(s, "FREVD", &mut m.energy.FREVD);
    b.f32s(s, "FREVI", &mut m.energy.FREVI);
    b.f32s(s, "FREGD", &mut m.energy.FREGD);
    b.f32s(s, "FREGI", &mut m.energy.FREGI);
    b.f32s(s, "RHO", &mut m.energy.RHO);
    b.f32s(s, "TAU", &mut m.energy.TAU);
    b.f32(s, "COSZ", &mut m.energy.COSZ);
    b.f32(s, "COSZ_HORIZ", &mut m.energy.COSZ_HORIZ);
    b.f32(s, "BGAP", &mut m.energy.BGAP);
    b.f32(s, "WGAP", &mut m.energy.WGAP);
    b.f32(s, "FSUN", &mut m.energy.FSUN);
    b.f32(s, "FSHA", &mut m.energy.FSHA);
    b.f32(s, "LAISUN", &mut m.energy.LAISUN);
    b.f32(s, "LAISHA", &mut m.energy.LAISHA);
    b.f32(s, "PARSUN", &mut m.energy.PARSUN);
    b.f32(s, "PARSHA", &mut m.energy.PARSHA);
    b.f32(s, "SAV", &mut m.energy.SAV);
    b.f32(s, "SAG", &mut m.energy.SAG);
    b.f32(s, "FSA", &mut m.energy.FSA);
    b.f32(s, "FSR", &mut m.energy.FSR);
    b.f32(s, "FSRV", &mut m.energy.FSRV);
    b.f32(s, "FSRG", &mut m.energy.FSRG);
    b.f32(s, "TAH", &mut m.energy.TAH);
    b.f32(s, "EAH", &mut m.energy.EAH);
    b.f32(s, "ZPD", &mut m.energy.ZPD);
    b.f32(s, "Z0MG", &mut m.energy.Z0MG);
    b.f32(s, "Z0M", &mut m.energy.Z0M);
    b.f32(s, "ZLVL", &mut m.energy.ZLVL);
    b.f32(s, "CMV", &mut m.energy.CMV);
    b.f32(s, "CMB", &mut m.energy.CMB);
    b.f32(s, "CM", &mut m.energy.CM);
    b.f32(s, "CH", &mut m.energy.CH);
    b.f32(s, "TGB", &mut m.energy.TGB);
    b.f32(s, "QSFC", &mut m.energy.QSFC);
    b.f32(s, "EMV", &mut m.energy.EMV);
    b.f32(s, "EMG", &mut m.energy.EMG);
    b.f32(s, "GAMMAV", &mut m.energy.GAMMAV);
    b.f32(s, "GAMMAG", &mut m.energy.GAMMAG);
    b.f32(s, "EVC", &mut m.energy.EVC);
    b.f32(s, "IRC", &mut m.energy.IRC);
    b.f32(s, "IRG", &mut m.energy.IRG);
    b.f32(s, "SHC", &mut m.energy.SHC);
    b.f32(s, "SHG", &mut m.energy.SHG);
    b.f32(s, "SHB", &mut m.energy.SHB);
    b.f32(s, "EVG", &mut m.energy.EVG);
    b.f32(s, "EVB", &mut m.energy.EVB);
    b.f32(s, "TR", &mut m.energy.TR);
    b.f32(s, "GH", &mut m.energy.GH);
    b.f32(s, "GHB", &mut m.energy.GHB);
    b.f32(s, "GHV", &mut m.energy.GHV);
    b.f32(s, "T2MV", &mut m.energy.T2MV);
    b.f32(s, "CHLEAF", &mut m.energy.CHLEAF);
    b.f32(s, "CHUC", &mut m.energy.CHUC);
    b.f32(s, "CHV2", &mut m.energy.CHV2);
    b.f32(s, "CHB2", &mut m.energy.CHB2);
    b.f32(s, "Q2V", &mut m.energy.Q2V);
    b.f32(s, "LATHEAV", &mut m.energy.LATHEAV);
    b.f32(s, "LATHEAG", &mut m.energy.LATHEAG);
    b.f32(s, "LATHEA", &mut m.energy.LATHEA);
    b.f32(s, "RSURF", &mut m.energy.RSURF);
    b.f32(s, "RHSUR", &mut m.energy.RHSUR);
    b.f32(s, "TAUXV", &mut m.energy.TAUXV);
    b.f32(s, "TAUYV", &mut m.energy.TAUYV);
    b.f32(s, "TAUXB", &mut m.energy.TAUXB);
    b.f32(s, "TAUYB", &mut m.energy.TAUYB);
    b.f32(s, "TAUX", &mut m.energy.TAUX);
    b.f32(s, "TAUY", &mut m.energy.TAUY);
    b.f32(s, "CAH2", &mut m.energy.CAH2);
    b.f32(s, "EHB2", &mut m.energy.EHB2);
    b.f32(s, "T2MB", &mut m.energy.T2MB);
    b.f32(s, "Q2B", &mut m.energy.Q2B);
    b.f32(s, "TGV", &mut m.energy.TGV);
    b.f32(s, "CHV", &mut m.energy.CHV);
    b.f32(s, "RSSUN", &mut m.energy.RSSUN);
    b.f32(s, "RSSHA", &mut m.energy.RSSHA);
    b.f32(s, "RB", &mut m.energy.RB);
    b.f32(s, "FIRA", &mut m.energy.FIRA);
    b.f32(s, "FSH", &mut m.energy.FSH);
    b.f32(s, "FGEV", &mut m.energy.FGEV);
    b.f32(s, "TRAD", &mut m.energy.TRAD);
    b.f32(s, "IRB", &mut m.energy.IRB);
    b.f32(s, "SSOIL", &mut m.energy.SSOIL);
    b.f32(s, "T2M", &mut m.energy.T2M);
    b.f32(s, "TS", &mut m.energy.TS);
    b.f32(s, "CHB", &mut m.energy.CHB);
    b.f32(s, "Q1", &mut m.energy.Q1);
    b.f32(s, "Q2E", &mut m.energy.Q2E);
    b.f32(s, "Z0WRF", &mut m.energy.Z0WRF);
    b.f32(s, "EMISSI", &mut m.energy.EMISSI);
    b.f32(s, "PSN", &mut m.energy.PSN);
    b.f32(s, "PSNSUN", &mut m.energy.PSNSUN);
    b.f32(s, "PSNSHA", &mut m.energy.PSNSHA);
    b.f32(s, "APAR", &mut m.energy.APAR);
    b.f32(s, "QMELT", &mut m.energy.QMELT);
    b.f32(s, "LH", &mut m.energy.LH);
    b.f32(s, "TGS", &mut m.energy.TGS);
    b.i32(s, "ICE", &mut m.energy.ICE);
    let s = rec.state("water");
    b.f32(s, "qinsur", &mut m.water.qinsur);
    b.f32(s, "qseva", &mut m.water.qseva);
    b.f32(s, "EVAPOTRANS", &mut m.water.EVAPOTRANS);
    b.f32(s, "runsrf", &mut m.water.runsrf);
    b.f32(s, "runsub", &mut m.water.runsub);
    b.f32(s, "qdrain", &mut m.water.qdrain);
    b.f32(s, "zwt", &mut m.water.zwt);
    b.f32(s, "smcwtd", &mut m.water.smcwtd);
    b.f32(s, "deeprech", &mut m.water.deeprech);
    b.f32(s, "fcrmax", &mut m.water.fcrmax);
    b.f32(s, "snoflow", &mut m.water.snoflow);
    b.f32(s, "pddum", &mut m.water.pddum);
    b.f32(s, "FACC", &mut m.water.FACC);
    b.f32(s, "sicemax", &mut m.water.sicemax);
    b.f32(s, "FB_snow", &mut m.water.FB_snow);
    b.f32(s, "rain", &mut m.water.rain);
    b.f32(s, "snow", &mut m.water.snow);
    b.f32(s, "bdfall", &mut m.water.bdfall);
    b.f32(s, "FP", &mut m.water.FP);
    b.f32(s, "canliq", &mut m.water.canliq);
    b.f32(s, "canice", &mut m.water.canice);
    b.f32(s, "FWET", &mut m.water.FWET);
    b.f32(s, "CMC", &mut m.water.CMC);
    b.f32(s, "QINTR", &mut m.water.QINTR);
    b.f32(s, "QDRIPR", &mut m.water.QDRIPR);
    b.f32(s, "QTHROR", &mut m.water.QTHROR);
    b.f32(s, "QINTS", &mut m.water.QINTS);
    b.f32(s, "QDRIPS", &mut m.water.QDRIPS);
    b.f32(s, "QTHROS", &mut m.water.QTHROS);
    b.f32(s, "QRAIN", &mut m.water.QRAIN);
    b.f32(s, "QSNOW", &mut m.water.QSNOW);
    b.f32(s, "SNOWHIN", &mut m.water.SNOWHIN);
    b.f32(s, "ECAN", &mut m.water.ECAN);
    b.f32(s, "ETRAN", &mut m.water.ETRAN);
    b.f32(s, "QSNFRO", &mut m.water.QSNFRO);
    b.f32(s, "QSNSUB", &mut m.water.QSNSUB);
    b.f32(s, "SNOWH", &mut m.water.SNOWH);
    b.f32(s, "SNEQV", &mut m.water.SNEQV);
    b.f32(s, "SNEQVO", &mut m.water.SNEQVO);
    b.f32(s, "BDSNO", &mut m.water.BDSNO);
    b.f32(s, "QSNBOT", &mut m.water.QSNBOT);
    b.f32(s, "PONDING", &mut m.water.PONDING);
    b.f32(s, "PONDING1", &mut m.water.PONDING1);
    b.f32(s, "PONDING2", &mut m.water.PONDING2);
    b.f32(s, "QVAP", &mut m.water.QVAP);
    b.f32(s, "QDEW", &mut m.water.QDEW);
    b.f32(s, "QSDEW", &mut m.water.QSDEW);
    b.f32(s, "WSLAKE", &mut m.water.WSLAKE);
    b.f32(s, "runsrf_dt", &mut m.water.runsrf_dt);
    b.f32(s, "ASAT", &mut m.water.ASAT);
    b.f32(s, "ACSNOM", &mut m.water.ACSNOM);
    b.i32(s, "ISNOW", &mut m.water.ISNOW);
    b.f32s(s, "smc", &mut m.water.smc);
    b.f32s(s, "smc_init", &mut m.water.smc_init);
    b.f32s(s, "sice", &mut m.water.sice);
    b.f32s(s, "sh2o", &mut m.water.sh2o);
    b.f32s(s, "etrani", &mut m.water.etrani);
    b.f32s(s, "BTRANI", &mut m.water.BTRANI);
    b.f32s(s, "wcnd", &mut m.water.wcnd);
    b.f32s(s, "fcr", &mut m.water.fcr);
    b.f32s(s, "FICEOLD", &mut m.water.FICEOLD);
    b.f32s(s, "SNICE", &mut m.water.SNICE);
    b.f32s(s, "SNLIQ", &mut m.water.SNLIQ);
    b.f32s(s, "SNICEV", &mut m.water.SNICEV);
    b.f32s(s, "SNLIQV", &mut m.water.SNLIQV);
    b.f32s(s, "FICE", &mut m.water.FICE);
    b.f32s(s, "EPORE", &mut m.water.EPORE);
    b.f32(s, "FSNO", &mut m.water.FSNO);
    b.f32(s, "BTRAN", &mut m.water.BTRAN);
    let s = rec.state("paramstate");
    b.f32(s, "LAI", &mut m.parameters.LAI);
    b.f32(s, "SAI", &mut m.parameters.SAI);
    b.f32(s, "ELAI", &mut m.parameters.ELAI);
    b.f32(s, "ESAI", &mut m.parameters.ESAI);
    b.f32(s, "VAI", &mut m.parameters.VAI);
    b.bool(s, "VEG", &mut m.parameters.VEG);
    b.f32(s, "FVEG", &mut m.parameters.FVEG);
}
