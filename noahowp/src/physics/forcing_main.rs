//! Port of `src/ForcingModule.f90` @ 0ff055e.
//!
//! The second of the five physics calls, and the thinnest: `ForcingMain` exists only to call
//! `ATM`. It is kept as its own function rather than folded away so the call graph still maps
//! one-to-one onto the Fortran's.

use crate::energy::Energy;
use crate::forcing::Forcing;
use crate::options::Options;
use crate::parameters::Parameters;
use crate::physics::atm_processing::{atm, PrecipInput};
use crate::water::Water;

/// `ForcingMain`.
pub fn forcing_main(
    options: &Options,
    parameters: &Parameters,
    forcing: &mut Forcing,
    energy: &mut Energy,
    water: &mut Water,
    precip_input: PrecipInput,
) {
    atm(options, parameters, forcing, energy, water, precip_input);
}
