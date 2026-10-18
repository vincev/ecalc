//! Physics constants from the HP 35s CONST menu (CODATA 1998), then Earth,
//! the Sun, the Moon, and the astronomical unit from the NASA planetary
//! fact sheets.
//!
//! Each value is the printed number, converted to SI for the planets. The
//! unit is a label only. `e` is not here; `H P` already pushes it.

use crate::float::MAX_PRECISION;
use crate::parse::parse_value;
use crate::value::{Mode, Value};

pub(crate) const COLUMN: usize = 8;
pub(crate) const PAGE: usize = COLUMN * 2;

struct Constant {
    symbol: &'static str,
    name: &'static str,
    unit: &'static str,
    /// Decimal literal. An integer literal stays an exact integer.
    literal: &'static str,
}

/// Manual order, with the final `e` removed. Sixteen per page. The last
/// page ends with Earth, the Moon, the Sun, and the astronomical unit.
const CATALOG: &[Constant] = &[
    c("c", "light", "m/s", "299792458"),
    c("g", "gravity", "m/s^2", "9.80665"),
    c("G", "Newton", "m^3/kg s^2", "6.673e-11"),
    c("Vm", "molar volume", "m^3/mol", "0.022413996"),
    c("NA", "Avogadro", "1/mol", "6.02214199e23"),
    c("Rinf", "Rydberg", "1/m", "10973731.5685"),
    c("qe", "charge", "C", "1.602176462e-19"),
    c("me", "electron", "kg", "9.10938188e-31"),
    c("mp", "proton", "kg", "1.67262158e-27"),
    c("mn", "neutron", "kg", "1.67492716e-27"),
    c("mmu", "muon", "kg", "1.88353109e-28"),
    c("k", "Boltzmann", "J/K", "1.3806503e-23"),
    c("h", "Planck", "J s", "6.62606876e-34"),
    c("hbar", "h/2pi", "J s", "1.054571596e-34"),
    c("phi0", "flux", "Wb", "2.067833636e-15"),
    c("a0", "Bohr radius", "m", "5.291772083e-11"),
    c("eps0", "electric", "F/m", "8.854187817e-12"),
    c("R", "gas constant", "J/mol K", "8.314472"),
    c("F", "Faraday", "C/mol", "96485.3415"),
    c("u", "atomic mass", "kg", "1.66053873e-27"),
    c("mu0", "magnetic", "N/A^2", "1.2566370614e-6"),
    c("muB", "Bohr magneton", "J/T", "9.27400899e-24"),
    c("muN", "nuclear mag.", "J/T", "5.05078317e-27"),
    c("mup", "proton moment", "J/T", "1.410606633e-26"),
    c("mue", "electron mom.", "J/T", "-9.28476362e-24"),
    c("mun", "neutron mom.", "J/T", "-9.662364e-27"),
    c("mumu", "muon moment", "J/T", "-4.49044813e-26"),
    c("re", "electron radius", "m", "2.817940285e-15"),
    c("Z0", "impedance", "ohm", "376.730313461"),
    c("lc", "Compton", "m", "2.426310215e-12"),
    c("lcn", "n Compton", "m", "1.319590898e-15"),
    c("lcp", "p Compton", "m", "1.321409847e-15"),
    c("alpha", "fine structure", "", "7.297352533e-3"),
    c("sigma", "Stefan", "W/m^2 K^4", "5.6704e-8"),
    c("t", "Celsius", "K", "273.15"),
    c("atm", "atmosphere", "Pa", "101325"),
    c("gp", "gyromagnetic", "1/s T", "267522212"),
    c("c1", "radiation 1", "W m^2", "3.74177107e-16"),
    c("c2", "radiation 2", "m K", "0.014387752"),
    c("G0", "conductance", "S", "7.748091696e-5"),
    // NASA fact sheets, in SI. Radii are equatorial. Sun radius is 695700 km.
    // The AU is the exact IAU metre. Moon distance is the mean orbit,
    // 0.3844 million km.
    c("Me", "Earth mass", "kg", "5.9722e24"),
    c("Re", "Earth radius", "m", "6378137"),
    c("Mm", "Moon mass", "kg", "7.346e22"),
    c("Rm", "Moon radius", "m", "1738100"),
    c("dm", "Moon distance", "m", "384400000"),
    c("Ms", "Sun mass", "kg", "1.9884e30"),
    c("Rs", "Sun radius", "m", "695700000"),
    c("AU", "astronomical", "m", "149597870700"),
];

const fn c(
    symbol: &'static str,
    name: &'static str,
    unit: &'static str,
    literal: &'static str,
) -> Constant {
    Constant {
        symbol,
        name,
        unit,
        literal,
    }
}

pub(crate) fn pages() -> usize {
    CATALOG.len().div_ceil(PAGE)
}

pub(crate) fn get(index: usize) -> Option<(String, Value)> {
    let item = CATALOG.get(index)?;
    Some((message(item), value(item)))
}

/// Hex key, symbol, and short name for one page.
pub(crate) fn rows(page: usize) -> Vec<(char, &'static str, &'static str)> {
    let start = page * PAGE;
    CATALOG
        .iter()
        .skip(start)
        .take(PAGE)
        .enumerate()
        .map(|(slot, item)| (hex_key(slot), item.symbol, item.name))
        .collect()
}

fn message(item: &Constant) -> String {
    if item.unit.is_empty() {
        item.symbol.to_string()
    } else {
        format!("{}  {}", item.symbol, item.unit)
    }
}

fn value(item: &Constant) -> Value {
    let mode = Mode {
        prec: MAX_PRECISION,
        ..Mode::default()
    };
    parse_value(item.literal, &mode).unwrap_or_else(|err| panic!("{}: {err}", item.symbol))
}

fn hex_key(slot: usize) -> char {
    b"0123456789ABCDEF"[slot] as char
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_three_pages_without_e() {
        assert_eq!(CATALOG.len(), 48);
        assert_eq!(pages(), 3);
        assert!(CATALOG.iter().all(|item| item.symbol != "e"));
        assert_eq!(rows(0).len(), 16);
        assert_eq!(rows(1).len(), 16);
        assert_eq!(rows(2).len(), 16);
        assert_eq!(rows(2)[0].0, '0');
        assert_eq!(rows(2)[7].0, '7');
        assert_eq!(rows(2)[8], ('8', "Me", "Earth mass"));
        assert_eq!(rows(2)[10], ('A', "Mm", "Moon mass"));
        assert_eq!(rows(2)[12], ('C', "dm", "Moon distance"));
        assert_eq!(rows(2)[15], ('F', "AU", "astronomical"));
    }

    #[test]
    fn published_digits_round_trip() {
        for item in CATALOG {
            let expect = if item.symbol == "alpha" {
                "0.007297352533"
            } else {
                item.literal
            };
            assert_eq!(value(item).to_string(), expect, "{}", item.symbol);
        }
    }
}
