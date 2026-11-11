# ecalc

Terminal RPN calculator in the shape of GNU Emacs Calc. Algebra, calculus,
vectors, and matrices are out of scope (use a CAS for those).

## Run and test

```sh
cargo test
cargo run
```

`cargo test` does not open a terminal. `cargo run` is the full-screen UI.
The binary is `ec`. Quit with `q`.

`ec --version` prints the version. `ec -e` (`--eval`) evaluates one RPN
string and prints the top of the stack. No arguments still opens the screen.

## Emacs Calc

To check an exact result against GNU Emacs Calc, ask Calc in batch mode.
This is a reference. `cargo test` does not need Emacs. `calc-eval` reads
algebraic syntax, not these RPN keys. A fraction is `n:d`. Fraction mode
is off, so `/` produces a float.

```sh
emacs -Q --batch -l calc -eval '(progn (princ (calc-eval "10 \\ 3")) (terpri) (princ (calc-eval "10 % 3")) (terpri))' 2>&1
```

That prints `3`, then `1`. `12 \ 12:7` is `7`. `12 \ (12/7)` is `6`, because
Calc divides the float and then floors. A large power prints `Working...`
lines on stderr. The value is on stdout. Do not match transcendentals
digit for digit.

The Calc source ships with Emacs, gzipped (`calc-comb.el.gz`,
`calc-arith.el.gz`, key bindings in `calc-ext.el.gz`). Read it with `zcat`
or `zgrep`. The path changes with each Emacs upgrade, so look it up:

```sh
emacs -Q --batch -eval '(princ (locate-library "calc-comb.el"))'
```

## PARI/GP

PARI/GP is the reference for the digits of a float result. Emacs Calc is
the reference for what a key does and for exact results. At 12 digits Calc
is itself off in the last digits of a large power (`1.0000001^2000000`,
`2.^10000000`) and of an inverse hyperbolic, so do not copy a float from
Calc into a test. Ask PARI at 30 digits and round to 12.

```sh
command gp -q <<'EOF'
default(realprecision, 30);
print(1.000001^1000000);
print(acos(0.999999999999) * 180 / Pi);
EOF
```

That prints `2.71828046931937688381979970845`, then
`8.10284684541462986323391069816 E-5`. PARI works in radians. Run it as
`command gp`, which skips aliases: some shells alias `gp` to `git push`.
Where the two disagree on meaning, the key follows Calc: `_8 3 I ^` is
`-2` (Calc's `nroot`), where PARI's `sqrtn(-8, 3)` is complex.

## Layout

- `src/float.rs` — decimal floats (`BigInt` mantissa, base-10 exponent). Precision is significant digits, default 12. `+ - * /`, sqrt, exp, ln, sin/cos/tan, inverse trig.
- `src/value.rs` — `Int` / `Rat` / `Float` / rectangular complex / integer modulo forms. Promotion, powers, roots, trig in degrees or radians.
- `src/cplx.rs` — complex kernels on decimal floats: `ln`, `ln1p`, `expm1`, `tan`, `tanh`, and the six inverse functions, with `refine`, which widens the working precision until both parts settle.
- `src/consts.rs` — physics constants (HP 35s / CODATA 1998), plus Earth, the Sun, the Moon, and the astronomical unit. `K` lists them.
- `src/parse.rs` — entry syntax.
- `src/engine.rs` — stack, undo, modes, operations.
- `src/app.rs` — key mapping. Test this without a tty.
- `src/ui.rs`, `src/main.rs` — arguments (`clap`) and the ratatui screen.

## Number rules

- Integers stay exact (`num-bigint`). `:` entry and fraction mode use rationals (`num-rational`), shown as `n:d`.
- Default fraction mode is off, matching Calc. `/` of integers that divide evenly stays an integer; otherwise it is a float at the current precision. `m f` turns fraction mode on. `:` outside entry is fraction division.
- Floats are decimal, so `0.1 + 0.2` is `0.3`. A whole-number float keeps a trailing point (`6378137.`), including after `c f`. Notation follows the precision, as in Calc's normal format: a float is written in full while its order runs from -2 through one less than the precision, and in scientific form outside that. At precision 12, `4.2e10` is `42000000000.`, `1e11` is `100000000000.`, `1e12` and `1e20` stay scientific, `0.01` stays fixed, and `0.001` is `1e-3`. An exact integer result, such as `sin(90)`, has no point.
- An integer base greater than 1 takes an exact integer log when the value is a positive rational whose numerator and denominator are both integer powers of that base, including a reciprocal and a decimal that is that rational. `1:8 2 B` and `0.125 2 B` are `-3`. `1:1000 H L` is `-3`. A result that is not an integer, such as `8 4 B` (`log4(8) = 3/2`), stays a float.
- A negative square root, and non-real powers, become rectangular complex `(re, im)`. A zero imaginary part collapses back to a real. A fractional power of a negative number is the principal value, as in Calc and PARI/GP: `_8 1:3 ^` is `(1., 1.73205080757)`. The real odd root is `I ^`: `_8 3 I ^` is `-2`.
- A modulo form `a mod m` holds integers, with `m > 0` and `0 <= a < m`, as in Calc. Only integer moduli are supported; Calc's real moduli (`10.5 mod 360`) are not.
- Angle mode defaults to degrees. It applies to real trig and to both parts of complex trig, same idea as Calc. A degree angle is reduced modulo 360 before conversion, including a non-integer (`12345678901.5` is `181.5` degrees) and an integer float whose exponent is past 100000. `P` pushes pi. `H P` pushes e.
- Inverse (`I`) and hyperbolic (`H`) are one-shot prefixes.

## Keys

```
digits . e E _ - + : ( ) ,     entry
RET SPC   push, or duplicate the top if the entry is empty
DEL       backspace, or pop
TAB       swap the top two
S-Tab     roll the top three upward (level 3 becomes the top)
+ - * / ^ \ %                  arithmetic, power, idiv, mod
F         floor toward minus infinity (`45.3` is `45`, `-45.3` is `-46`)
n & Q A   negate, reciprocal, sqrt, abs
I Q       square        I ^     nth root
S C T     sin cos tan    I … inverse    H … hyperbolic
L E       ln / exp       H L log10      H E 10^x
B         log (base on top)     I B antilog
f T f h f Q f A f s f I f E f L
          atan2, hypot, isqrt, abs^2, sign, ilog, expm1, lnp1
k g k l   gcd, lcm of the top two (integers or fractions)
k E       extended gcd: y on top, x under it; pushes a, b, g with g on top and a*x + b*y = g
k i       modular inverse: m on top, result b with 0 <= b < m and a*b ≡ 1 (mod m)
k p       prime test; the number stays on the stack
k f       prime factors, smallest first. `2023 k f` leaves `7`, `17`, `17`. `* *` rebuilds `2023`
k t       Euler's totient: `12 k t` is `4`
k n I k n next prime above, previous prime below
H k n     nth prime: `1000000 H k n` is `15485863`
k c       choose, M on top: N! / (M! (N-M)!)
H k c     permutations: N! / (N-M)!
k s       Stirling number of the first kind, signed: s(N, M), M on top
H k s     Stirling number of the second kind: S(N, M), M on top
!         factorial (exact for integers, gamma otherwise)
J G       conjugate, argument
P         pi             H P  e
K         physics constants; 0-F inserts, Tab pages
p         precision from the top, then pop (3 to 1000)
m d m r m f                degrees, radians, fraction mode
d 2 d 8 d 6 d 0            binary, octal, hex, decimal
d r       radix 2–36       d z   leading zeros
d g       group integer digits (3, or 4 in binary and hex)
b w       word size, default 32, may be negative
O d …     two's complement before d 2, d 8, d 6, d 0, or d r
#digits   current radix; 16#FF is hex in any mode
M         while typing an integer, start its modulus: 3M1000 is 3 mod 1000
c f c F   to float, to fraction
s s       store the top, leaving it on the stack
s t       pop the top into a variable
s r       recall a variable onto the stack
s u       unstore a variable
s l       list stored variables; letter or digit recalls, Tab pages
s 0-9     store into q0–q9; t 0-9 pops into one; r 0-9 recalls one
t y       trail: pick a past value; type to filter, RET pushes it
u         undo           C-r redo
          undo restores the stack and the variables
q ? Esc   quit, shortcut list, cancel entry or prefixes
?         shortcut pages. Tab and S-Tab turn the page and wrap. q quits
? ?       command pages. Type to search. Esc closes. ? returns to the list.
```

`L` follows Calc: it flips the inverse flag and then uses the exp/ln family, so `L` is ln and `I L` is exp.

## Tests worth keeping green

Tutorial shapes live in `src/app.rs`. Parser cases are in `src/parse.rs`. The screen test in `src/ui.rs` uses ratatui's `TestBackend` (the top value sits above the `.`).

- `1:8 2 B` = `-3`, `1:1000 H L` = `-3`, and `8 4 B` stays a float
- `1 2 3 4 + * -` = `-13`
- `2*4 + 7*9.5 + 5/4` = `75.75`
- `2^10` = `1024`, and a big exact `2^100`
- `125` `3` `I ^` = `5`
- magnitude of `(3, 4)` = `5`
- `i * i` = `-1`
- `sin(90 deg)` = `1`, `atan(1)` = `45` in degrees
- `10 \ 3` = `3`, `10 % 3` = `1`, `-5 \ 3` = `-2`, `-5 % 3` = `1`
- `12 \ 12:7` = `7`, `_12:1 _12:7 \` = `7`, `3:2 4:3 \` = `1`, `_3:2 4:3 \` = `-2`, `1e20 \ 3` = `33333333333300000000`, `1e20 % 3` = `100000000.`
- `36#q` = `26`, `36#n` = `23`, `36#u` = `30` (and `q` does not quit there)
- `45.3 F` = `45`, `-45.3 F` = `-46`
- `4 2 k s` = `11`, `4 2 H k s` = `7`, and `10 5 H k s` = `42525`
- `2023 k f` leaves `7`, `17`, `17`, and `* *` is `2023`
- `43920492304 k f` leaves `2`, `2`, `2`, `2`, `13`, `5903`, `35771`
- `7 k n` = `11`, `11 I k n` = `7`, `1000000 H k n` = `15485863`
- `12 k t` = `4`, `1000000007 k t` = `1000000006`
- `K` then `0` = `299792458` (speed of light)
- `ec -e '2 20 ^ d g'` = `1,048,576`
- `3M1000 567 ^` = `587 mod 1000`, `3M7 2 /` = `5 mod 7`, `2M7 _1 ^` = `4 mod 7`, and `3M7 4M8 +` is an error
- `1.000001 1000000 ^` = `2.71828046932`, `2. 10000000 ^` = `9.04981730636e3010299`, `(1000000, 1) Q` = `(1000., 5e-4)`, `_1000 H I S` = `-7.60090270954`, `0.999999999999 I C` = `8.10284684541e-5`, `1.0000000001 I S` = `(90., -8.10284684535e-4)`, `1e-3000 E` = `1.`, `m r 1 I S` = `1.57079632679`, `m r 2 I C` = `(0, 1.31695789692)`, `3 p 3 H T` = `0.995`, `_1.0000000001 H I C` = `(1.41421356236e-5, 3.14159265359)`, `20 H I T` = `(0.0500417292785, -1.57079632679)`, `_0.26 H I C` = `(0, 1.8338185297)`, `5.5001 !` = `287.936897959`, `m r (_3461328.16571, 17) H I S` = `(-15.7503101169, 4.91140948965e-6)`, `(19898.92, _0.323) S` = `(0.987921499546, 8.7411513021e-4)`, `(_0.01, 9e-11) H T` = `(-9.99966668e-3, 8.99910006e-11)`, `_12 f L` = `(2.3978952728, 3.14159265359)`, `30 p 1234.567 !` = `2.89217946521305892228567224505e3282`, `20 p 0.9999999999999999995 L` = `-5.0000000000000000013e-19` (`numeric_results_match_pari` in `src/app.rs`)
- `t y` lists `3`, `4`, `+ 7` for `3 4 +`, RET pushes the selection, and undo leaves the trail (`trail_records_numbers_and_results`, `trail_dialog_picks_and_filters`)
- last `K` page, `8` = `5.9722e24` (Earth mass), `A` = `7.346e22` (Moon mass), and `F` = `149597870700` (AU)

Transcendentals are checked by known prefixes (pi, e, ln 2) or against PARI/GP, not by matching Emacs Calc digit for digit. Exact values can be checked with the Emacs Calc command above.

## Not in this version

Algebraic entry (`'`), formulas, vectors, matrices, units, dates, HMS, the trail window and its other `t` commands, numeric prefixes (`C-u`), polar display, infinities.

## Status

Working RPN calculator. `cargo test` is green (float, value, parser, keys, and a headless screen test). `cargo build` produces the ratatui UI. That UI was checked with `TestBackend`, not by typing in a live terminal.

The screen keeps the terminal's own background. A dark terminal uses the GrokNight foregrounds; a light one uses dark text. On Unix, startup asks for the background with OSC 11 and treats no answer as dark. Windows keeps the dark foregrounds. The headless screen test still paints the GrokNight canvas (`#141414`). The live screen has a rounded `❯` prompt with the mode on its bottom border, and a shortcut bar (`ret:push`, bold key, muted label). Content sits two columns in from each edge. Under the prompt is a blank row, then the shortcuts, then a blank row. The typed text sits one column inside the prompt border. `?` opens a Shortcuts box centered on the input's width, in the space above the prompt: the same number of rows sits above the frame as between the frame and the input, and an odd leftover row sits between the frame and the input. Gray `┌┐` frame, bold title on the top edge, `[x]` at the top right, and `1/2` on the note row when another page exists. Bindings fill down up to three columns of up to 15 rows (a narrow or short terminal uses fewer), bold key and muted label. Tab and Shift-Tab turn the page and wrap. The footer shows `tab next`, `S-Tab prev`, `? docs`, and `esc close`. `q quit` stays on the window bar. `?` again opens Commands, in the same frame. The top row is a search field. Several matches show a short list and the selected page under a rule. One match shows only that page. Up and Down, or Tab and Shift-Tab, move and wrap. Backspace edits the search. Esc closes. `?` returns to the shortcut list. `q` is a search letter there.

Entry and stack details that are easy to get wrong:

- Keep `num-bigint` at 0.4.8 (`num-rational` 0.4 needs that major).
- `DecFloat::from_decimal(mantissa, exponent, prec)` builds a typed decimal. The stored precision is the number of significant digits typed, capped at the current precision.
- A typed exponent, decimal or radix, outside ±1000000 is overflow. `1e1000000` and `1e-1000000` still parse. An exponent that does not fit in a 64-bit integer is a bad exponent.
- `q` quits when nothing is being typed. While radix or word size is being typed, `q` cancels that prompt and leaves the stack. A variable name can contain `q`. `q` does not quit while `K`, `s l`, or the command reference is open. The shortcut list pages with Tab and Shift-Tab, and `q` still quits there. `p` is not a prompt: it pops an integer and sets the precision.
- Do not polish add, sub, mul, or div. `(1/3)*3` can display as `0.999…`, and `sqrt(2)^2` as `1.999…`.
- `snap_close_integer` in `float.rs` snaps only a guard-digit speck: the unrounded value must be within three digits past the last one shown. Judging the rounded value would snap a true result one unit below a power of ten. `0.999999999998 Q` is `0.999999999999`, `_1e-12 E` is `0.999999999999`, and `10 L E` is `9.99999999996`, as in Emacs Calc and PARI/GP. At precision 16, `1 E L` is `0.9999999999999999`, because 16-digit e rounds down.
- A float exponent saturates inside `DecFloat` and never wraps. `value::in_range`, called from `Engine::unary_mod` and `Engine::binop_mod`, turns a result whose order is past ±10^18 into `overflow`. Squaring `2.5e999999` 45 times is an overflow, not a wrapped exponent.
- `pow_int` in `value.rs` sends a float base to `DecFloat::powi`, which carries one guard digit per exponent bit. Do not multiply a float at the display precision in a loop: the error grows with the exponent. An exact base is limited by the size of the result (`e * log2` of the larger of numerator and denominator against `MAX_POWER_BITS`, about 2.5 million digits), not by the exponent, so `2. 10000000 ^` and `1 2000000 ^` answer and `10 100000 ^ 100000 ^` is `overflow`. The numerator and denominator are raised separately with `BigInt::pow` and joined with `new_raw`: they share no factor, and multiplying fractions in a loop ran a gcd on million-digit numbers (`1:3 4194303 ^` took 96 s). A negative power is a float unless fraction mode is on (`3:2 _2 ^` is `0.444444444444`). A float of magnitude 1 is `±1.` for any exponent, and an exponent past `i64` goes through exp/ln. A complex base still caps the exponent at 20 bits.
- `pow_exp_ln` adds one guard digit per integer digit of `exp * ln(base)` (up to 40). Without them `_1.23e600000 1000001:3 ^` was wrong from the eighth digit.
- `pi / n` is `DecFloat::pi_over(n, prec)`, rounded once. `pi(prec) / 2` gave `1.5707963268` for `m r 1 I S`.
- Avoid a difference that cancels. Complex `sqrt` takes the larger part from `radius + |x|` and the other as `y / (2 * larger)`. `acos` is `atan2(sqrt((1 - x)(1 + x)), x)`. Real `asinh`, `acosh`, and `atanh` use `ln_1p` forms (`asinh_float`, `acosh_float`) and the odd symmetry. Real `atanh` past 1 is `(atanh(1/x), -pi/2)`, odd in `x`, with `atanh(1/a) = ln1p(2 / (a - 1)) / 2`: `20 H I T` is `(0.0500417292785, -1.57079632679)`. Real `acosh` inside `[-1, 1)` is `(0, acos x)`: `_0.26 H I C` is `(0, 1.8338185297)`. `asin` of a real past 1 is `(±90, ∓acosh|x|)`. `acos` past 1 is `(0, acosh x)`, and `(180, -acosh|x|)` below -1. Real `acosh` below -1 is `(acosh|x|, pi)`. Real `tanh` is `-t / (2 + t)` with `t = expm1(-2|x|)`, rounded once (`sinh / cosh` missed the last digit half the time), and past `2 * precision + 10` it is `±1.`.
- In degrees, an inverse trig result stays at `prec + 8` (`angle_prec`) until `from_radians` has scaled it. Scaling a rounded angle gave `0.5 I C` = `60.0000000002`.
- `reduction_prec` in `float.rs` adds digits only for a large argument. A tiny one needs none: `1e-3000 E` is `1.` and `1e-3000 S` is `1.74532925199e-3002`.
- Complex transcendentals go through `src/cplx.rs`, not through `ln`, `sqrt`, and `*` at the display precision. Composed that way `m r (_3461328.16571, 17) H I S` came out as `(-11.5, pi)`; it is `(-15.7503101169, 4.91140948965e-6)`. `asin`, `acos`, `asinh`, and `acosh` use the Hull, Fairgrieve, and Tang forms. `atanh` is `(ln1p(4a / ((1 - a)^2 + b^2)) / 4, atan2(2b, (1 - a)(1 + a) - b^2) / 2)`, and `atan(z)` is `-i atanh(iz)`. `ln|z|` near the unit circle is `ln1p(x^2 + y^2 - 1) / 2` with the sum taken exactly. `tan` is `(sin x cos x, sinh y cosh y) / (cos^2 x + sinh^2 y)`. `refine` runs each kernel at `prec + 10` and `prec + 22` and widens (up to `4 prec + 80`) until both parts agree; that covers the cancellation a formula cannot avoid, such as `expm1` with `x` near `y^2 / 2`. On the imaginary axis `asinh` and `atan` take the sign of `y` for the real part, as PARI/GP does. PARI at 12 digits is itself wrong for some of these, so take the reference at 30 or more.
- Complex trig in degrees converts each part on its own (`mode_sin_cos`, `mode_radians`). The real part is reduced modulo 360 exactly and a multiple of 90 is exact, so `(_7200000, _12) T` is `(0, -0.206429956437)`. A fraction of a degree is divided at the working precision (`reduce_degree_rational`): `30 p 1:3 S` was wrong from the seventeenth digit.
- `finish_complex` runs once, on a finished result. Dropping the tiny imaginary part of `cosh` before the division made the imaginary part of `(_0.01, 9e-11) H T` wrong in the fifth digit. `B`, `H L`, and `^` take their logs from `ln_keep`, which skips it. Emacs Calc drops such a part too (`exp((0, 3.14159265359))` is `-1.`), and PARI/GP keeps it.
- A complex product keeps the four products in full and rounds each sum once (`mul`). `f E` is one rounding of `expm1` at guard precision, and `-1.` for a large negative argument. `f L` below -1 is `(ln|1 + x|, pi)`. Zero to a positive real power is `0`.
- `+ - * /` keep a tiny complex part: `(1, 1e-20) (0, 1) *` is `(-1e-20, 1.)`. Only transcendental results drop a part more than `prec` orders below the other (`finish_complex`).
- `c F` runs the continued fraction until the half-ulp tolerance is met. It has no term cap: 80 terms stopped short past about 80 digits.
- `ln` leaves a value in `[0.7071, 1)` unscaled. Scaling it to `9.99…` and subtracting `ln(10)` cancelled: `20 p 0.9999999999999999995 L` kept one digit, and is `-5.0000000000000000013e-19`.
- `half_index` in `comb.rs` doubles a float with one extra digit before asking whether it is a half-integer. At the typed precision `5.5001 !` rounded to `11` and answered `5.5!`. It is `287.936897959`, and `500.3 !` is `7.87533108633e1134`.
- A non-integer factorial is Spouge's formula (`factorial_float` in `comb.rs`). Its coefficients alternate in sign and reach about `10^(0.555 a)` for `a` terms, and the sum they leave can be as small as `sqrt(2 pi)`, so `spouge_work` carries `0.56 a` guard digits, plus one per integer digit of the exponent `(z + 1/2) ln(z + a)`. A third of `a` was enough only for a small argument: `100 p 972.32 !` had ten wrong digits. `spouge_coefficients` builds `e^1 .. e^(a-1)` by repeated multiplication. A separate `exp` per coefficient made `1000 p 0.3 !` run for minutes; it takes about 1.6 s.
- `DecFloat::sqrt` is the integer square root (`BigInt::sqrt`) of the mantissa scaled to twice the working digits with an even exponent. Newton on decimal floats started from one digit and ran every step at full precision.
- `pow10` in `float.rs` keeps powers of ten below `10^4096` in a per-thread table. Every rounding asks for a few, and raising 10 each time was about 60% of a 1000-digit `exp`.
- `digit_count` and `strip_trailing_zeros` in `float.rs` run on every rounding. Do not print the mantissa to count or strip digits. That is quadratic and made `exp` at precision 1000 take about 2s instead of 0.3s.
- An unparsable radix literal names the whole literal: `16#` says `bad entry: 16#`.
- Modulo forms follow Calc. `M` while typing an integer inserts ` mod ` (`3M1000` is `3 mod 1000`), and a second `M` fills in the previous modulus. `M` is still a digit in a radix literal above 22 (`36#mM7` is one number). DEL removes ` mod ` in one piece, a space waits for the modulus, and in `ec -e` a word ending in `M` continues into the next (`3M 1000`). A bare `M` says `type an integer before M`, and `3.5M` says `modulo forms need integers` and keeps the entry. The value is `Value::Mod(a, m)`, always reduced. `+ - * / ^`, `n`, `&`, and `I Q` take a form with an integer (an exact integer float is accepted; a whole float past the integer bound, such as `1e100001`, is `overflow`) or a form of the same modulus. After ` mod `, `(`, `,` and `)` are not entry keys. Different moduli say `modulo forms need the same modulus`. `/` and `&` multiply by the modular inverse, so `3M6 2 /` says `no modular inverse` and a zero divisor is `division by zero`. `^` uses `BigInt::modpow`, so an exponent such as `10^30` is fine. A negative power inverts first. A form used as the exponent contributes its residue, as in Calc. `0M7 0 ^` is `1 mod 7`, where Calc prints a plain `1`; PARI/GP agrees with `1 mod 7`. Display formats both sides in the current radix (`16#3 mod 16#7`).
- `Engine::unary` and `Engine::binop` reject a modulo form with `not for modulo forms`. Only the operations above use `unary_mod` and `binop_mod`, so a new operation cannot take a form by accident. Operations that read the stack themselves (`k p`, `k f`, `k E`, `p`) reject it through `exact_int`.
- `L` flips inverse in `app.rs`, then uses the exp/ln family. `E` does not flip. `push` and a successful duplicate do not clear `I` or `H`.
- `n` while typing toggles a leading `_`. `+` and `-` while typing are exponent signs, or digits inside an unclosed `(…)`. Otherwise they add and subtract. Space inside an unparsed `(…` inserts a space. `:` while typing is a fraction; on an empty entry it is fraction division.
- `ec -e` feeds words through the same keys. Whitespace, including newlines, is a separator, and a finished number is pushed at the end of each word. `2 20 ^ d g` prints `1,048,576`. Both `d g` and `dg` group. A space on the screen still pushes or duplicates. An unclosed `(…` stays open across a space, so `(3, 4)` is one value. `12 p` sets the precision and pops 12. A prompt finishes when its answer is complete (`d r 16`, `b w 64`, `s s name`). The command prints the top value. An error goes to stderr and the process exits 1. `q` and `?` stay on the screen.
- Stack index 0 is the oldest value. The top of the stack is the last element, drawn at the bottom, with a bold `.` on the last row. The screen does not print level numbers. Values and the `.` start on the same column as the character after `❯ `. A value wider than that row is split across lines: every line but the last ends with `\`, and each later line is indented one column. When that split value is taller than the rows above `.`, the leading lines stay and still end in `\`. Older values that do not fit are left off the top.
- `F` is floor toward minus infinity and returns an exact integer. An integer is unchanged. A fraction or a decimal float is exact (`-4:3` is `-2`). A complex number is an error. `f` is still the function prefix.
- `k s` is the signed Stirling number of the first kind `s(n, m)`, and `H k s` is the second kind `S(n, m)`, with `m` on top. Both arguments are nonnegative integers, and an exact integer float is accepted. `m > n` is `0`, and a negative or fractional argument is an error. The recurrence table is capped: `n` above 2000 is `overflow`, though `m` of `0`, `1`, or `n` is answered directly.
- `k i` is the modular inverse of `a` modulo `m`, with `m` on top. The result is the integer `b` with `0 <= b < m` and `a*b ≡ 1 (mod m)`. `3 7 k i` is `5`. Both values are integers, and an exact integer float is accepted. A non-positive modulus says `modulus must be positive`. If the gcd is not 1 the stack stays and the message is `no modular inverse`. Emacs Calc gets the same `b` as the reciprocal of a modulo form.
- `k f` replaces the top integer with its prime factors, smallest first, so the largest is on top. `2023 k f` leaves `7`, then `17`, then `17`, and `* *` is `2023`. A negative number leads with `-1`. `0` and `1` stay as they are. An exact integer float is accepted. Trial division uses Calc's prime table through `5003`, then Pollard rho (Brent) splits a composite cofactor up to 1024 bits. `43920492304 k f` leaves `2`, `2`, `2`, `2`, `13`, `5903`, `35771`. A cofactor that cannot be proven prime or split stays in one piece, the stack still changes, and the message is `may not be fully factored`.
- `k t` is Euler's totient, as Calc's `calcFunc-totient`. `0` and `1` stay, a negative number is an error (PARI/GP would take the absolute value), and an exact integer float is accepted. It uses the `k f` factorization, so it answers past Calc's trial-division limit (`1000000007 k t` is `1000000006`, which Calc leaves unevaluated). If a cofactor cannot be proven prime or split, it says `number too big to factor` and leaves the stack.
- `k n` and `I k n` follow Calc's `nextprime` and `prevprime`. `k n` truncates a non-integer and gives `2` for anything below 2. `I k n` rounds a non-integer up and gives `2` for anything below 4, so `2 I k n` is `2`, as in Calc. Past the proven bound of the prime test the answer replaces the top with the message `probably prime`. `H k n` is the nth prime, PARI/GP's `prime(n)`, for `1 <= n <= 10^8` (above is `overflow`). It sieves odd numbers in segments up to the Rosser bound `n (ln n + ln ln n)`; `10^8` takes a few seconds in a release build.
- `K` opens the constants, sixteen to a page, eight down each column. `0`–`9` and `a`–`f` insert that slot and close. Tab and Shift-Tab turn the page and wrap around. The last page has the remaining physics constants, then Earth mass (`Me`), Earth equatorial radius (`Re`, `6378137` m), Moon mass (`Mm`, `7.346e22` kg), Moon equatorial radius (`Rm`, `1738100` m), the mean Earth-Moon distance (`dm`, `384400000` m), Sun mass (`Ms`), Sun radius (`Rs`, `695700` km written in metres), and the astronomical unit (`AU`, `149597870700` m). Every slot is filled. A missing slot would say "no constant" and stay open. Esc closes. `q` does not quit while it is open. The physics numbers are the HP 35s CODATA 1998 values. The planetary numbers are the NASA Earth, Sun, and Moon fact sheets, in kilograms and metres. An integer literal stays an integer (`299792458`, `101325`, `6378137`, `149597870700`, `1738100`). The unit is only a label in the menu and the message (`c  m/s`, `Me  kg`). `e` is not listed; `H P` pushes it. `K` is still a digit while a radix literal above 20 is being typed.
- `atan2` of a value on an axis is an exact right angle (`0`, `±90`, `180` in degrees), including `atan2(0, 0) = 0` and `atan2(0, negative) = 180`.
- The trail (`t y`) replaces Calc's trail window with a dialog. `Engine::record` logs every pushed number and every result: `push`, `unary_mod`, `binop_mod`, `recall`, `k E`, `k n`, and each factor of `k f`. Duplicate, swap, roll, and pop are not logged, and neither is a value pushed back from the trail. The label is the command's keys (`*`, `I S`, `H k n`, `K c`, `rcl x`), set by `App::label_command` before the command runs, with `H` before `I` whichever was typed first; a typed number has an empty label. The full `Value` is kept and shown in the current display mode. Undo does not remove entries. The caps are 1000 entries (`TRAIL_CAP`) and 64 MB of digits (`TRAIL_BYTES`, by `Value::approx_bytes`), oldest dropped first, for the session only. In the dialog, newest at the bottom, typing filters by label or value text (grouping commas ignored on both sides, so `31415` finds `31,415,926…` under `d g`), Up and Down (or Tab and S-Tab) move and wrap, RET pushes (undoable), Esc closes, and `q` is a search letter. Opening the dialog formats every entry once into `TrailView` (display text cut at 512 characters, plus an ungrouped search text); typing and moving only compare those strings, and the draw reads them. A label longer than its column (8 to 16 wide) ends in `..`. `ec -e` rejects `t y` with `the trail is for the screen`.
- Undo stores the stack and the variables, cap 100. Mode changes are not undoable. A new stack change clears redo. `s s` stores without popping, `s t` pops into the variable, `s r` recalls, and `s u` unstores. A digit selects `q0`–`q9` (`s 3`, `t 3`, `r 3`). A name is finished with RET. `s l` lists them, five per column. Tab and Shift-Tab page and wrap around. A letter recalls that one-letter name, and a digit recalls `q0`–`q9`. Longer names are visible there and still recalled with `s r`.
- Radix is display and entry only. A number typed without `#` is decimal. `d 2`, `d 8`, `d 6`, `d 0`, and `d r` set the radix (2–36). `#` uses the current radix; `16#FF` forces one. Letters are digits only after `#` when the radix is above 10. For radix 14 or less, `e` is a power of that radix (`8#1.2e3`). From 15 up, `e` is a digit and the power is `16#1.2*16^3`. `d z` pads integers to the word size. `b w` sets that width (default 32, negative allowed, display uses the absolute value). `O` before a radix command turns on two's complement for radices 2, 8, and 16 (`16##FD` is an 8-bit −3). Setting the radix without `O` turns it off. `radix##digits` reads a two's-complement word. `d g` groups integer digits with commas: 4 in binary and hex, 3 otherwise. The fractional part and the exponent stay plain, so at precision 13 `1234567.123454` displays as `1,234,567.123454`. At the default 12 the entry rounds to `1,234,567.12345`. Grouping is display only.
