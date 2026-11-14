//! Random comparison with PARI/GP and GNU Emacs Calc.
//!
//! These tests need `gp` and `emacs` on the `PATH`, so they are ignored by
//! default and `cargo test` stays self-contained. Run them with
//!
//! ```sh
//! cargo test --release --test reference -- --ignored --nocapture
//! ```
//!
//! `EC_REF_SEED` picks another random sequence and `EC_REF_SCALE` multiplies
//! the number of cases (`EC_REF_SCALE=10` for a long hunt). A failure prints
//! the RPN program, so it can be replayed with `ec -e`.
//!
//! PARI/GP is the reference for digits. Emacs Calc is checked on exact
//! results only. The differences that are deliberate are written down
//! where they are skipped, not hidden in a tolerance.

use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use std::fmt::Write as _;
use std::io::Write as _;
use std::process::{Command, Stdio};

/// Largest error of a float result, in units of its last digit. A correctly
/// rounded result is within 0.5.
const MAX_ULP: f64 = 0.75;

// ---------------------------------------------------------------------
// Random numbers.

/// A seeded generator, so a run can be repeated. The sequence for a seed
/// belongs to the `rand` version in `Cargo.lock`.
struct Rng(StdRng);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(StdRng::seed_from_u64(seed))
    }

    /// Uniform in `lo..=hi`.
    fn range(&mut self, lo: i128, hi: i128) -> i128 {
        self.0.random_range(lo..=hi)
    }

    fn chance(&mut self, p: f64) -> bool {
        self.0.random_bool(p)
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.0.random_range(0..items.len())]
    }

    /// `count` decimal digits with no leading zero.
    fn digits(&mut self, count: u32) -> String {
        let mut out = String::new();
        out.push(char::from(b'0' + self.range(1, 9) as u8));
        for _ in 1..count {
            out.push(char::from(b'0' + self.range(0, 9) as u8));
        }
        out
    }
}

fn seed() -> u64 {
    env_number("EC_REF_SEED").unwrap_or(1)
}

fn scaled(cases: usize) -> usize {
    cases * env_number("EC_REF_SCALE").unwrap_or(1).max(1) as usize
}

fn env_number(name: &str) -> Option<u64> {
    let text = std::env::var(name).ok()?;
    Some(
        text.parse()
            .unwrap_or_else(|_| panic!("{name} must be a whole number, not {text:?}")),
    )
}

// ---------------------------------------------------------------------
// The three calculators.

/// Evaluate every program, in parallel. An error becomes `Err(message)`.
fn run_ec(programs: &[String]) -> Vec<Result<String, String>> {
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = programs.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = programs
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .map(|program| ecalc::eval(program).map_err(|err| err.to_string()))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("an evaluation panicked"))
            .collect()
    })
}

/// Run a PARI/GP script and return what it printed.
fn run_gp(script: &str) -> String {
    let mut child = Command::new("gp")
        .arg("-q")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|err| {
            panic!("PARI/GP is needed: could not run `gp` from the PATH ({err})")
        });
    let mut stdin = child.stdin.take().expect("gp stdin");
    let script = script.to_owned();
    // Feed the script from another thread: gp prints while it reads.
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(script.as_bytes());
    });
    let output = child.wait_with_output().expect("gp did not finish");
    writer.join().expect("the gp script was not written");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Evaluate algebraic expressions in Emacs Calc, one result per expression.
fn run_calc(expressions: &[String]) -> Vec<String> {
    let mut lisp = String::from("(require 'calc)\n(require 'calc-ext)\n(require 'calc-comb)\n");
    lisp.push_str("(dolist (e (list");
    for expression in expressions {
        let quoted = expression.replace('\\', "\\\\").replace('"', "\\\"");
        write!(lisp, " \"{quoted}\"").unwrap();
    }
    lisp.push_str(")) (princ (format \"%s\\n\" (calc-eval e))))\n");
    let path = std::env::temp_dir().join(format!("ecalc-reference-{}.el", std::process::id()));
    std::fs::write(&path, lisp).expect("could not write the Emacs script");
    let output = Command::new("emacs")
        .args(["-Q", "--batch", "-l"])
        .arg(&path)
        .stderr(Stdio::null())
        .output();
    let _ = std::fs::remove_file(&path);
    let output = output.unwrap_or_else(|err| {
        panic!("Emacs is needed: could not run `emacs` from the PATH ({err})")
    });
    let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        lines.len(),
        expressions.len(),
        "Emacs Calc answered {} of {} expressions",
        lines.len(),
        expressions.len()
    );
    lines
}

/// An integer as the entry keys type it: `_` is the minus sign.
fn keys(n: i128) -> String {
    if n < 0 {
        format!("_{}", n.unsigned_abs())
    } else {
        n.to_string()
    }
}

fn report(what: &str, failures: &[String]) {
    assert!(
        failures.is_empty(),
        "{} {what} (seed {}):\n{}",
        failures.len(),
        seed(),
        failures.join("\n")
    );
}

// ---------------------------------------------------------------------
// Exact results.

struct ExactCase {
    name: &'static str,
    program: String,
    /// PARI/GP expression, when PARI has the operation.
    pari: Option<String>,
    /// Calc expression, when Calc evaluates it in batch mode.
    calc: Option<String>,
}

/// A fraction in lowest terms with a small denominator.
fn fraction(rng: &mut Rng) -> (i128, i128) {
    loop {
        let n = rng.range(-400, 400);
        let d = rng.range(2, 60);
        if n != 0 && gcd(n.abs(), d) == 1 {
            return (n, d);
        }
    }
}

fn gcd(a: i128, b: i128) -> i128 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn any_integer(rng: &mut Rng) -> i128 {
    match rng.range(0, 2) {
        0 => rng.range(-60, 60),
        1 => rng.range(-1_000_000, 1_000_000),
        _ => rng.range(-10i128.pow(25), 10i128.pow(25)),
    }
}

fn positive(rng: &mut Rng) -> i128 {
    match rng.range(0, 2) {
        0 => rng.range(1, 100),
        1 => rng.range(1, 1_000_000),
        _ => rng.range(1, 10i128.pow(14)),
    }
}

fn exact_case(rng: &mut Rng) -> ExactCase {
    const OPS: &[&str] = &[
        "choose",
        "perm",
        "fact",
        "gcd",
        "lcm",
        "totient",
        "next",
        "prev",
        "nth",
        "factor",
        "prime",
        "dot",
        "stirling1",
        "stirling2",
        "idiv",
        "mod",
        "ridiv",
        "rmod",
        "fidiv",
        "fmod",
        "inverse",
        "modpow",
        "floor",
        "isqrt",
        "ilog",
        "fractions",
        "ipow",
        "rpow",
        "egcd",
    ];
    let name = rng.pick(OPS);
    let case = |program: String, pari: Option<String>, calc: Option<String>| ExactCase {
        name,
        program,
        pari,
        calc,
    };
    match name {
        "choose" => {
            let n = if rng.chance(0.5) {
                rng.range(-80, 200)
            } else {
                rng.range(-10i128.pow(12), 10i128.pow(12))
            };
            // Both negative is rare by chance and has its own rule.
            let m = if n < 0 && rng.chance(0.2) {
                n - rng.range(-3, 40)
            } else {
                rng.range(-3, 90)
            };
            case(
                format!("{} {} k c", keys(n), keys(m)),
                Some(format!("binomial({n},{m})")),
                Some(format!("choose({n},{m})")),
            )
        }
        "perm" => {
            let n = if rng.chance(0.5) {
                rng.range(0, 200)
            } else {
                rng.range(0, 10i128.pow(12))
            };
            let m = rng.range(0, 60);
            case(
                format!("{n} {m} H k c"),
                Some(format!("prod(i=0,{m}-1,{n}-i)")),
                Some(format!("perm({n},{m})")),
            )
        }
        "fact" => {
            let n = rng.range(0, 400);
            case(
                format!("{n} !"),
                Some(format!("{n}!")),
                Some(format!("fact({n})")),
            )
        }
        "gcd" | "lcm" => {
            let (a, b) = (any_integer(rng), any_integer(rng));
            let key = if name == "gcd" { "k g" } else { "k l" };
            case(
                format!("{} {} {key}", keys(a), keys(b)),
                Some(format!("{name}({a},{b})")),
                Some(format!("{name}({a},{b})")),
            )
        }
        "totient" => {
            let n = positive(rng);
            // Calc factors by trial division and gives up on a large prime.
            let calc = (n < 10_000_000).then(|| format!("totient({n})"));
            case(format!("{n} k t"), Some(format!("eulerphi({n})")), calc)
        }
        "next" => {
            let n = if rng.chance(0.5) {
                rng.range(-5, 200)
            } else {
                positive(rng)
            };
            case(
                format!("{} k n", keys(n)),
                Some(format!("nextprime({n}+1)")),
                Some(format!("nextprime({n})")),
            )
        }
        "prev" => {
            let n = if rng.chance(0.5) {
                rng.range(4, 200)
            } else {
                positive(rng) + 4
            };
            case(
                format!("{n} I k n"),
                Some(format!("precprime({n}-1)")),
                Some(format!("prevprime({n})")),
            )
        }
        "nth" => {
            let n = rng.range(1, 200_000);
            case(format!("{n} H k n"), Some(format!("prime({n})")), None)
        }
        "factor" => {
            let top = rng.pick(&[6u32, 14, 22]);
            let n = rng.range(-10i128.pow(top), 10i128.pow(top));
            // Each prime as often as it divides, as Calc's `prfac` lists them.
            let pari = format!(
                "if({n}==0 || {n}==1, [{n}], my(f=factor({n})); concat(vector(#f~, i, vector(f[i,2], j, f[i,1]))))"
            );
            // Calc factors by trial division and gives up on a large prime.
            let calc = (n.abs() < 10_000_000).then(|| format!("prfac({n})"));
            case(format!("{} k f", keys(n)), Some(pari), calc)
        }
        "prime" => {
            let n = if rng.chance(0.3) {
                rng.range(-10, 200)
            } else {
                positive(rng)
            };
            case(
                format!("{} k p", keys(n)),
                Some(format!("isprime({n})")),
                Some(format!("prime({n})")),
            )
        }
        "dot" => {
            let len = rng.range(0, 6) as usize;
            let x: Vec<i128> = (0..len).map(|_| any_integer(rng)).collect();
            let y: Vec<i128> = (0..len).map(|_| any_integer(rng)).collect();
            let list = |v: &[i128]| {
                v.iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let (x, y) = (list(&x), list(&y));
            let program = format!("[{}] [{}] *", typed_vec(&x), typed_vec(&y));
            case(
                program,
                Some(format!("[{x}] * [{y}]~")),
                (len > 0).then(|| format!("[{x}] * [{y}]")),
            )
        }
        "stirling1" | "stirling2" => {
            let (n, m) = (rng.range(0, 60), rng.range(0, 65));
            let (key, kind) = if name == "stirling1" {
                ("k s", 1)
            } else {
                ("H k s", 2)
            };
            case(
                format!("{n} {m} {key}"),
                Some(format!("stirling({n},{m},{kind})")),
                None,
            )
        }
        "idiv" | "mod" => {
            let a = any_integer(rng);
            let b = match any_integer(rng) {
                0 => 7,
                b => b,
            };
            if name == "idiv" {
                case(
                    format!("{} {} \\", keys(a), keys(b)),
                    Some(format!("floor(({a})/({b}))")),
                    Some(format!("({a}) \\ ({b})")),
                )
            } else {
                // Emacs Calc is wrong here for a negative operand past 15
                // digits: `-6738377342024848068638086 % 715162` is
                // `-5152668638086` there, and `625390` in PARI/GP.
                let small = a.abs() < 10i128.pow(15) && b.abs() < 10i128.pow(15);
                case(
                    format!("{} {} %", keys(a), keys(b)),
                    Some(format!("({a})-({b})*floor(({a})/({b}))")),
                    small.then(|| format!("({a}) % ({b})")),
                )
            }
        }
        "ridiv" | "rmod" => {
            let ((a, b), (c, d)) = (fraction(rng), fraction(rng));
            let program = format!("{}:{b} {}:{d}", keys(a), keys(c));
            let (x, y) = (format!("({a}/{b})"), format!("({c}/{d})"));
            let (cx, cy) = (format!("({a}:{b})"), format!("({c}:{d})"));
            if name == "ridiv" {
                case(
                    format!("{program} \\"),
                    Some(format!("floor({x}/{y})")),
                    Some(format!("{cx} \\ {cy}")),
                )
            } else {
                case(
                    format!("{program} %"),
                    Some(format!("{x}-{y}*floor({x}/{y})")),
                    Some(format!("{cx} % {cy}")),
                )
            }
        }
        // Floats follow Calc: it divides the floats and then floors.
        "fidiv" | "fmod" => {
            let a = rng.range(-1_000_000, 1_000_000);
            let b = match rng.range(-5000, 5000) {
                0 => 37,
                b => b,
            };
            let decimal = |n: i128, places: u32| {
                let scale = 10i128.pow(places);
                let sign = if n < 0 { "-" } else { "" };
                let frac = format!("{:0width$}", n.abs() % scale, width = places as usize);
                format!("{sign}{}.{frac}", n.abs() / scale)
            };
            let (x, y) = (decimal(a, 3), decimal(b, 2));
            let op = if name == "fidiv" { "\\" } else { "%" };
            case(
                format!("{} {} {op}", x.replace('-', "_"), y.replace('-', "_")),
                None,
                Some(format!("({x}) {op} ({y})")),
            )
        }
        "inverse" => {
            let m = rng.range(2, 1_000_000_000);
            let a = rng.range(-1_000_000_000, 1_000_000_000);
            case(
                format!("{} {m} k i", keys(a)),
                Some(format!("iferr(lift(Mod({a},{m})^-1), E, \"none\")")),
                None,
            )
        }
        "modpow" => {
            let m = rng.range(2, 1_000_000_000);
            let a = rng.range(0, 1_000_000_000);
            let e = rng.range(0, 10i128.pow(30));
            case(
                format!("{a}M{m} {e} ^"),
                Some(format!("lift(Mod({a},{m})^{e})")),
                None,
            )
        }
        "floor" => {
            if rng.chance(0.5) {
                let (a, b) = fraction(rng);
                case(
                    format!("{}:{b} F", keys(a)),
                    Some(format!("floor({a}/{b})")),
                    Some(format!("floor({a}:{b})")),
                )
            } else {
                let a = rng.range(-1_000_000_000, 1_000_000_000);
                case(
                    format!("{}e-4 F", keys(a)),
                    Some(format!("floor({a}/10^4)")),
                    Some(format!("floor({a}e-4)")),
                )
            }
        }
        "isqrt" => {
            let count = rng.range(1, 40) as u32;
            let n = rng.digits(count);
            case(
                format!("{n} f Q"),
                Some(format!("sqrtint({n})")),
                Some(format!("isqrt({n})")),
            )
        }
        "ilog" => {
            let base = rng.range(2, 20);
            let n = if rng.chance(0.3) {
                // An exact power, where an off-by-one would show.
                base.pow(rng.range(0, 28) as u32).to_string()
            } else {
                let count = rng.range(1, 40) as u32;
                rng.digits(count)
            };
            case(
                format!("{n} {base} f I"),
                Some(format!("logint({n},{base})")),
                Some(format!("ilog({n},{base})")),
            )
        }
        "fractions" => {
            let ((a, b), (c, d)) = (fraction(rng), fraction(rng));
            let op = rng.pick(&['+', '-', '*', ':']);
            let pari_op = if op == ':' { '/' } else { op };
            // Calc reads `:` between two fractions as one long fraction.
            let calc = (op != ':').then(|| format!("({a}:{b}){op}({c}:{d})"));
            case(
                format!("{}:{b} {}:{d} {op}", keys(a), keys(c)),
                Some(format!("({a}/{b}){pari_op}({c}/{d})")),
                calc,
            )
        }
        "ipow" => {
            let (a, e) = (rng.range(-99, 99), rng.range(0, 120));
            case(
                format!("{} {e} ^", keys(a)),
                Some(format!("({a})^{e}")),
                Some(format!("({a})^{e}")),
            )
        }
        "rpow" => {
            let (a, b) = fraction(rng);
            let e = rng.range(0, 40);
            case(
                format!("{}:{b} {e} ^", keys(a)),
                Some(format!("({a}/{b})^{e}")),
                Some(format!("({a}:{b})^{e}")),
            )
        }
        "egcd" => {
            let (a, b) = (positive(rng), positive(rng));
            // The gcd is the one left on top.
            case(format!("{a} {b} k E"), Some(format!("gcd({a},{b})")), None)
        }
        other => unreachable!("no generator for {other}"),
    }
}

/// A vector typed for `ec -e`: `_` for a minus sign.
fn typed_vec(list: &str) -> String {
    list.replace('-', "_")
}

/// Same number, allowing for how each program writes it. A vector compares
/// without its spaces.
fn same_exact(ec: &str, reference: &str) -> bool {
    if ec.starts_with('[') {
        return ec.replace(' ', "") == reference.replace(' ', "");
    }
    // A modulo form prints as `a mod m`. PARI's `lift` gives `a`.
    let ec = ec.split(" mod ").next().unwrap_or(ec);
    let reference = reference.replace('/', ":").replace(' ', "");
    if ec == reference {
        return true;
    }
    // `12.5` and `12.50`, or `1e-3` and `0.001`.
    match (ec.parse::<f64>(), reference.parse::<f64>()) {
        (Ok(a), Ok(b)) => (a - b).abs() <= 1e-9 * b.abs().max(1.0),
        _ => false,
    }
}

#[test]
#[ignore = "needs PARI/GP and Emacs on the PATH"]
fn exact_results_match_pari_and_calc() {
    let mut rng = Rng::new(seed());
    let cases: Vec<ExactCase> = (0..scaled(6000)).map(|_| exact_case(&mut rng)).collect();
    let programs: Vec<String> = cases.iter().map(|case| case.program.clone()).collect();
    let results = run_ec(&programs);

    let mut script = String::from("default(parisizemax, \"1G\");\n");
    for (index, case) in cases.iter().enumerate() {
        if let Some(pari) = &case.pari {
            writeln!(
                script,
                "iferr(print({index}, \" \", {pari}), E, print({index}, \" error\"));"
            )
            .unwrap();
        }
    }
    let mut pari = vec![None; cases.len()];
    for line in run_gp(&script).lines() {
        if let Some((index, value)) = line.split_once(' ')
            && let Ok(index) = index.parse::<usize>()
        {
            pari[index] = Some(value.to_owned());
        }
    }

    let asked: Vec<usize> = (0..cases.len())
        .filter(|&index| cases[index].calc.is_some())
        .collect();
    let expressions: Vec<String> = asked
        .iter()
        .map(|&index| cases[index].calc.clone().unwrap())
        .collect();
    let mut calc = vec![None; cases.len()];
    for (index, answer) in asked.iter().zip(run_calc(&expressions)) {
        calc[*index] = Some(answer);
    }

    let mut failures = Vec::new();
    let (mut pari_checked, mut calc_checked) = (0, 0);
    for (index, case) in cases.iter().enumerate() {
        let shown = match &results[index] {
            Ok(value) => value.clone(),
            Err(message) => format!("error: {message}"),
        };
        if let Some(reference) = &pari[index] {
            pari_checked += 1;
            let agree = match &results[index] {
                Ok(value) => same_exact(value, reference),
                // No inverse exists, and both say so.
                Err(_) => reference == "none",
            };
            if !agree {
                failures.push(format!(
                    "{}: `{}` is {shown}, PARI/GP has {reference}",
                    case.name, case.program
                ));
            }
        }
        if let Some(reference) = &calc[index] {
            // Calc hands back what it cannot evaluate, such as
            // `perm(15, 17)`.
            let unevaluated = reference.contains('(') && reference.ends_with(')');
            if unevaluated && reference.starts_with(|c: char| c.is_ascii_lowercase()) {
                continue;
            }
            calc_checked += 1;
            if !matches!(&results[index], Ok(value) if same_exact(value, reference)) {
                failures.push(format!(
                    "{}: `{}` is {shown}, Emacs Calc has {reference}",
                    case.name, case.program
                ));
            }
        }
    }
    println!(
        "exact: {} cases, {pari_checked} checked against PARI/GP, {calc_checked} against Emacs Calc",
        cases.len()
    );
    assert!(
        pari_checked > cases.len() / 2,
        "PARI/GP answered too little"
    );
    assert!(
        calc_checked > cases.len() / 3,
        "Emacs Calc answered too little"
    );
    report("exact results differ", &failures);
}

// ---------------------------------------------------------------------
// Float results.

/// A real number, as typed and as PARI/GP reads it exactly.
struct Number {
    keys: String,
    pari: String,
    negative: bool,
}

fn decimal(negative: bool, digits: &str, exponent: i64) -> Number {
    let zero = digits.bytes().all(|b| b == b'0');
    let negative = negative && !zero;
    let sign = if negative { "_" } else { "" };
    let minus = if negative { "-" } else { "" };
    if exponent == 0 {
        Number {
            keys: format!("{sign}{digits}"),
            pari: format!("({minus}{digits})"),
            negative,
        }
    } else {
        Number {
            keys: format!("{sign}{digits}e{exponent}"),
            pari: format!("({minus}{digits}*10^({exponent}))"),
            negative,
        }
    }
}

struct Numbers<'a> {
    rng: &'a mut Rng,
    prec: u32,
    complex: f64,
}

impl Numbers<'_> {
    /// Up to `max_digits` digits, with the order between `low` and `high`.
    fn spread(&mut self, max_digits: u32, low: i64, high: i64) -> Number {
        let count = self.rng.range(1, i128::from(max_digits)) as u32;
        let digits = self.rng.digits(count);
        let exponent =
            self.rng.range(i128::from(low), i128::from(high)) as i64 - i64::from(count) + 1;
        let negative = self.rng.chance(0.3);
        decimal(negative, &digits, exponent)
    }

    fn of_kind(&mut self, kind: &str) -> Number {
        let prec = self.prec;
        match kind {
            "int" => {
                let n = self.rng.range(-50, 50);
                decimal(n < 0, &n.abs().to_string(), 0)
            }
            "dec" => self.spread(prec, -6, 6),
            "tiny" => self.spread(prec, -40, -7),
            "big" => self.spread(prec, 7, 40),
            // 1 plus or minus a digit in the last place typed.
            "near1" => {
                let places = self.rng.range(1, i128::from(prec) - 1) as usize;
                let last = self.rng.range(1, 9);
                let digits = if self.rng.chance(0.5) {
                    format!("1{}{last}", "0".repeat(places - 1))
                } else {
                    format!("{}{}", "9".repeat(places - 1), 10 - last)
                };
                let negative = self.rng.chance(0.5);
                decimal(negative, &digits, -(places as i64))
            }
            // Inside (-1, 1), zero included.
            "unit" => {
                let places = self.rng.range(1, i128::from(prec)) as u32;
                let digits = if self.rng.chance(0.03) {
                    "0".to_owned()
                } else {
                    self.rng.digits(places)
                };
                let negative = self.rng.chance(0.5);
                decimal(negative, &digits, -i64::from(places))
            }
            "fraction" => {
                let d = self.rng.range(2, 999);
                let mut n = self.rng.range(-5000, 5000);
                if n == 0 || gcd(n.abs(), d) != 1 {
                    n = d + 1;
                }
                Number {
                    keys: format!("{}:{d}", keys(n)),
                    pari: format!("({n}/{d})"),
                    negative: n < 0,
                }
            }
            other => unreachable!("no number of kind {other}"),
        }
    }

    fn real(&mut self) -> Number {
        let kind = self.rng.pick(&[
            "int", "dec", "dec", "dec", "near1", "tiny", "big", "unit", "unit", "fraction",
        ]);
        self.of_kind(kind)
    }

    /// A real that is not a fraction. `+ - * /` convert a fraction at the
    /// display precision before they meet a float, as Emacs Calc does, and
    /// PARI/GP keeps it exact: `439:438 0.9991 -` is `3.18310502e-3` here
    /// and `3.18310502283e-3` there.
    fn real_for_arithmetic(&mut self) -> Number {
        let kind = self
            .rng
            .pick(&["int", "dec", "dec", "near1", "tiny", "big", "unit"]);
        self.of_kind(kind)
    }

    /// `(keys, pari)` for a real or, some of the time, a complex number.
    fn argument(&mut self, allow_complex: bool, arithmetic: bool) -> (String, String) {
        if allow_complex && self.rng.chance(self.complex) {
            let re_kind = self.rng.pick(&["int", "dec", "unit", "near1"]);
            let im_kind = self.rng.pick(&["int", "dec", "unit", "tiny"]);
            let (re, im) = (self.of_kind(re_kind), self.of_kind(im_kind));
            return (
                format!("({},{})", re.keys, im.keys),
                format!("({}+{}*I)", re.pari, im.pari),
            );
        }
        let number = if arithmetic {
            self.real_for_arithmetic()
        } else {
            self.real()
        };
        (number.keys, number.pari)
    }
}

struct FloatCase {
    name: &'static str,
    program: String,
    pari: String,
}

/// `(name, keys, PARI template, angle goes in, angle comes out)`.
const UNARY: &[(&str, &str, &str, bool, bool)] = &[
    ("sqrt", "Q", "sqrt(X)", false, false),
    ("exp", "E", "exp(X)", false, false),
    ("ln", "L", "log(X)", false, false),
    ("log10", "H L", "log(X)/log(10)", false, false),
    ("exp10", "H E", "exp(X*log(10))", false, false),
    ("sin", "S", "sin(X)", true, false),
    ("cos", "C", "cos(X)", true, false),
    ("tan", "T", "tan(X)", true, false),
    ("asin", "I S", "asin(X)", false, true),
    ("acos", "I C", "acos(X)", false, true),
    ("atan", "I T", "atan(X)", false, true),
    ("sinh", "H S", "sinh(X)", false, false),
    ("cosh", "H C", "cosh(X)", false, false),
    ("tanh", "H T", "tanh(X)", false, false),
    ("asinh", "H I S", "asinh(X)", false, false),
    ("acosh", "H I C", "acosh(X)", false, false),
    ("atanh", "H I T", "atnh(X)", false, false),
    ("expm1", "f E", "expm1(X)", false, false),
    ("lnp1", "f L", "log1p(X)", false, false),
    ("fact", "!", "gamma(X+1)", false, false),
    ("recip", "&", "1/(X)", false, false),
    ("square", "I Q", "(X)^2", false, false),
    ("abs", "A", "abs(X)", false, false),
    ("arg", "G", "arg(X)", false, true),
];

/// `(name, keys, PARI template)`.
const BINARY: &[(&str, &str, &str)] = &[
    ("add", "+", "(X)+(Y)"),
    ("sub", "-", "(X)-(Y)"),
    ("mul", "*", "(X)*(Y)"),
    ("div", "/", "(X)/(Y)"),
    ("pow", "^", "((X)*1.)^(Y)"),
    ("log", "B", "log(X)/log(Y)"),
    ("atan2", "f T", "atan2r(X,Y)"),
    ("hypot", "f h", "sqrt((X)^2+(Y)^2)"),
    ("root", "I ^", "nrt(X,Y)"),
];

fn float_case(numbers: &mut Numbers) -> FloatCase {
    let prec = numbers.prec;
    let degrees = numbers.rng.chance(0.4);
    let mode = if degrees { "" } else { "m r " };
    let total = UNARY.len() + BINARY.len();
    let choice = numbers.rng.range(0, total as i128 - 1) as usize;

    if let Some(&(name, key, template, angle_in, angle_out)) = UNARY.get(choice) {
        let (mut typed, mut pari) = numbers.argument(name != "fact", false);
        if matches!(name, "exp" | "sinh" | "cosh" | "exp10" | "expm1") && numbers.rng.chance(0.7) {
            // Keep most of these in a range that does not overflow.
            let number = numbers.spread(prec, -3, 3);
            (typed, pari) = (number.keys, number.pari);
        }
        if name == "fact" {
            let number = numbers.spread(prec.min(6), -2, 2);
            (typed, pari) = (number.keys, number.pari);
        }
        if angle_in && degrees {
            pari = format!("({pari}*Pi/180)");
        }
        let mut expected = template.replace('X', &pari);
        if angle_out && degrees {
            expected = format!("({expected})*180/Pi");
        }
        return FloatCase {
            name,
            program: format!("{prec} p {mode}{typed} {key}"),
            pari: expected,
        };
    }

    let (name, key, template) = BINARY[choice - UNARY.len()];
    let ((x_keys, x_pari), (y_keys, y_pari)) = match name {
        "atan2" | "hypot" => {
            let (a, b) = (numbers.real(), numbers.real());
            ((a.keys, a.pari), (b.keys, b.pari))
        }
        "root" => {
            let kind = numbers.rng.pick(&["dec", "int", "big", "tiny", "unit"]);
            let mut radicand = numbers.of_kind(kind);
            let degree = numbers.rng.range(2, 60);
            if radicand.negative && degree % 2 == 0 {
                // An even root of a negative number is complex; `I ^` of an
                // odd one is the real root, which is what `nrt` computes.
                radicand.keys = radicand.keys.replacen('_', "", 1);
                radicand.pari = radicand.pari.replacen('-', "", 1);
            }
            (
                (radicand.keys, radicand.pari),
                (degree.to_string(), degree.to_string()),
            )
        }
        "pow" => {
            let base = numbers.argument(true, false);
            let exponent = match numbers.rng.range(0, 99) {
                0..35 => {
                    let n = numbers.rng.range(-300, 300);
                    (keys(n), format!("({n})"))
                }
                35..50 => {
                    let n = numbers.rng.range(1000, 10_000_000);
                    (n.to_string(), n.to_string())
                }
                _ => {
                    let number = numbers.spread(prec, -3, 2);
                    (number.keys, number.pari)
                }
            };
            (base, exponent)
        }
        "add" | "sub" | "mul" | "div" => {
            (numbers.argument(true, true), numbers.argument(true, true))
        }
        _ => (numbers.argument(true, false), numbers.argument(true, false)),
    };
    let mut expected = template.replace('X', &x_pari).replace('Y', &y_pari);
    if name == "atan2" && degrees {
        expected = format!("({expected})*180/Pi");
    }
    FloatCase {
        name,
        program: format!("{prec} p {mode}{x_keys} {y_keys} {key}"),
        pari: expected,
    }
}

/// What the screen shows, as a PARI/GP literal.
fn as_pari(shown: &str) -> String {
    let shown = shown.replace(':', "/");
    match shown
        .strip_prefix('(')
        .and_then(|rest| rest.strip_suffix(')'))
        .and_then(|inner| inner.split_once(", "))
    {
        Some((re, im)) => format!("(({re})+({im})*I)"),
        None => format!("({shown})"),
    }
}

/// PARI/GP helpers. `chk` prints the error of a result in thousandths of a
/// unit in the last place.
///
/// It prints `dropped` when a part that is about the precision below the
/// other was left out: the calculator drops such a part from a
/// transcendental result, as Emacs Calc does, and PARI/GP keeps it. A part
/// left out that is larger than that counts as a full error.
///
/// It prints `ok` for an exact `0` where PARI/GP has only round-off, which
/// is far below anything these arguments can produce: `sin(180)` in degrees
/// is `0` here and about `1e-150` there.
fn pari_prelude(prec: u32) -> String {
    format!(
        "default(realprecision, {digits}); default(parisizemax, \"2G\");\n\
         atan2r(y,x) = if(x==0&&y==0, 0, arg(x+y*I));\n\
         nrt(x,k) = if(x<0, -sqrtn(-x,k), sqrtn(x,k));\n\
         atnh(x) = if(imag(x)==0&&abs(x)>1, log1p(2/(abs(x)-1))/2*sign(x)-sign(x)*I*Pi/2, log((1+x)/(1-x))/2);\n\
         ulp(r,p) = 10.^(floor(log(abs(r))/log(10)) - p + 1);\n\
         c1(v,r,p,s) = if(abs(r) < s*10.^(-p-2), if(v==0, 0, abs(v-r)/(s*10.^(-p))), if(v==0, if(abs(r) < s*10.^(1-p), -1, 10^6), abs(v-r)/ulp(r,p)));\n\
         chk(id,v,r,p) = {{ my(a,b,s); r = r*1.; s = abs(r); if(s==0, print(id, if(v==0, \" ok\", \" nonzero\")); return);\n\
           if(v==0 && s < 10.^(-3*p-100), print(id, \" ok\"); return);\n\
           a = c1(real(v),real(r),p,s); b = c1(imag(v),imag(r),p,s);\n\
           if(a<0 || b<0, print(id, \" dropped\"), print(id, \" ulp \", round(1000*max(a,b)))); }}\n",
        digits = prec * 4 + 150
    )
}

enum Verdict {
    /// Error in units of the last place.
    Ulp(f64),
    Dropped,
    /// Both programs refuse, or only PARI/GP does.
    Skipped,
    Wrong(String),
}

fn float_run(
    prec: u32,
    cases: usize,
    complex: f64,
    rng: &mut Rng,
) -> (Vec<FloatCase>, Vec<Verdict>) {
    let mut numbers = Numbers { rng, prec, complex };
    let cases: Vec<FloatCase> = (0..cases).map(|_| float_case(&mut numbers)).collect();
    let programs: Vec<String> = cases.iter().map(|case| case.program.clone()).collect();
    let results = run_ec(&programs);

    let mut script = pari_prelude(prec);
    for (index, (case, result)) in cases.iter().zip(&results).enumerate() {
        let pari = &case.pari;
        match result {
            Ok(shown) => writeln!(
                script,
                "iferr(chk({index},{},{pari},{prec}), E, print({index}, \" refused\"));",
                as_pari(shown)
            ),
            Err(_) => writeln!(
                script,
                "iferr(my(r=1.*({pari})); print({index}, \" value\"), E, print({index}, \" refused\"));"
            ),
        }
        .unwrap();
    }

    let mut verdicts: Vec<Verdict> = (0..cases.len())
        .map(|_| Verdict::Wrong("PARI/GP printed nothing".to_owned()))
        .collect();
    for line in run_gp(&script).lines() {
        let mut words = line.split(' ');
        let Some(index) = words.next().and_then(|word| word.parse::<usize>().ok()) else {
            continue;
        };
        if index >= cases.len() {
            continue;
        }
        verdicts[index] = match (words.next(), &results[index]) {
            (Some("ok"), _) => Verdict::Ulp(0.0),
            (Some("ulp"), _) => {
                let thousandths: f64 = words
                    .next()
                    .and_then(|w| w.parse().ok())
                    .unwrap_or(f64::MAX);
                Verdict::Ulp(thousandths / 1000.0)
            }
            (Some("dropped"), _) => Verdict::Dropped,
            (Some("refused"), _) => Verdict::Skipped,
            (Some("nonzero"), _) => Verdict::Wrong("PARI/GP has exactly 0".to_owned()),
            // The calculator refused and PARI/GP has a value. An exact
            // power that would not fit is an overflow by design, and an
            // exact 90 degrees has no tangent where PARI has round-off.
            (Some("value"), Err(message))
                if message.contains("overflow") || message.contains("tangent is undefined") =>
            {
                Verdict::Skipped
            }
            (Some("value"), Err(message)) => {
                Verdict::Wrong(format!("{message}, and PARI/GP has a value"))
            }
            (other, _) => Verdict::Wrong(format!("unreadable PARI/GP line {other:?}")),
        };
    }
    (cases, verdicts)
}

#[test]
#[ignore = "needs PARI/GP on the PATH"]
fn float_results_match_pari() {
    // (precision, cases, share of complex arguments)
    let runs = [
        (12, 8000, 0.3),
        (5, 1500, 0.3),
        (20, 2000, 0.3),
        (30, 2000, 0.3),
        (100, 600, 0.3),
        (200, 150, 0.3),
        (12, 4000, 1.0),
        (30, 1000, 1.0),
    ];
    let mut rng = Rng::new(seed());
    let mut failures = Vec::new();
    let (mut checked, mut dropped, mut skipped, mut worst) = (0usize, 0usize, 0usize, 0.0f64);
    for (prec, count, complex) in runs {
        let (cases, verdicts) = float_run(prec, scaled(count), complex, &mut rng);
        for (case, verdict) in cases.iter().zip(verdicts) {
            match verdict {
                Verdict::Ulp(error) => {
                    checked += 1;
                    worst = worst.max(error);
                    if error > MAX_ULP {
                        failures.push(format!(
                            "{}: `{}` is {error:.3} ulp from PARI/GP `{}`",
                            case.name, case.program, case.pari
                        ));
                    }
                }
                Verdict::Dropped => dropped += 1,
                Verdict::Skipped => skipped += 1,
                Verdict::Wrong(why) => {
                    failures.push(format!("{}: `{}`: {why}", case.name, case.program));
                }
            }
        }
    }
    println!(
        "float: {checked} results checked, worst {worst:.3} ulp, {dropped} with a tiny part \
         dropped, {skipped} refused by PARI/GP or too large"
    );
    assert!(checked > 1000, "PARI/GP answered too little");
    report("float results differ", &failures);
}
