//! Shims for the Fortran runtime behaviour the model depends on.
//!
//! Nothing here is physics. These modules exist so that ported code can read the same files and
//! evaluate the same intrinsics as the Fortran, bit for bit.

pub mod intrinsics;
pub mod list_directed;
pub mod namelist;
pub mod value;

pub use list_directed::ListDirectedReader;
pub use namelist::{Group, Namelist};
