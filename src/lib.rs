//! RPN calculator in the shape of GNU Emacs Calc.
//!
//! Numbers are exact integers, exact rationals, decimal floats, or
//! rectangular complex values built from those.

pub mod app;
mod comb;
mod consts;
mod docs;
pub mod engine;
pub mod error;
pub mod float;
pub mod parse;
mod radix;
pub mod ui;
pub mod value;

pub use app::{App, eval};
pub use engine::Engine;
pub use error::CalcError;
