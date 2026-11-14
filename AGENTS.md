# ecalc

Terminal RPN calculator in the shape of GNU Emacs Calc. Vectors and
matrices are in, as in Calc. Algebra and calculus are out of scope (use a
CAS for those).

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
`calc-arith.el.gz`, `calc-mtx.el.gz`, `calc-vec.el.gz`, key bindings in
`calc-ext.el.gz`). Read it with `zcat`
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

## Reference tests

`tests/reference.rs` compares random cases with PARI/GP and Emacs Calc. It
needs `gp` and `emacs` on the `PATH`, so both tests are ignored and plain
`cargo test` (and CI) skips them. Run them after touching anything numeric:

```sh
cargo test --release --test reference -- --ignored --nocapture
```

That is about 6,000 exact results (choose, permutations, factorials, gcd,
primes, factoring as vectors, `k p`, dot products, Stirling numbers, `\` and `%`, floor, modular powers,
fraction arithmetic, and matrices of integers, fractions, and modulo forms:
products, determinants, inverses, `/`, powers, trace, transpose, cross
product, identity, `f A`, and `V J` of Gaussian integers against Calc's
`ctrn`) and 19,000 float results (every function and `+ - * / ^`, real and
complex, at 5 to 200 digits, and about 5,000 of them the matrix operations
on matrices up to 5 by 5, plus `V J`, `A`, and `f A`, and the determinant,
inverse, and `/` of 6 by 6 to 9 by 9 matrices), in about ten seconds,
most of it PARI/GP inverting the larger matrices exactly. `A` of a vector
or matrix takes the root of the sum of squares at guard precision with
`DecFloat::sqrt` directly: `value::sqrt` rounds a float argument to the
precision first, as for a typed number, and the reference tests caught `A`
a unit off. A float result
may be 0.75 of a unit in the last place from PARI/GP; the worst seen is
0.53. A vector or matrix result is judged element by element (`chkm`), each
real and imaginary part on its own, and an exact `0` has to be `0`. The
worst matrix element seen is 0.500: the determinant, the inverse, and `/`
are exact before their one rounding, so a random ill-conditioned matrix
needs no looser tolerance, and PARI/GP is given the entries as exact
rationals so that its answer is exact too. `EC_REF_SEED=7` picks another sequence and `EC_REF_SCALE=10` runs ten
times the cases. A failure prints the RPN program for `ec -e`. A missing
`gp` or `emacs` fails the test with a message that says so.

The differences that are deliberate are skipped where the case is built or
judged, with the reason: a tiny complex part dropped from a transcendental
result, an exact `0` where PARI has round-off, an exact power too large to
hold, `+ - * /` of a fraction with a float, and Calc's own wrong `%` for a
negative operand past 15 digits. Calc is asked for an exact matrix result
only where it stays exact: its determinant up to 3 by 3 (past that it
eliminates in floats), and its integer inverse up to 3 by 3, compared to
nine digits, because Calc multiplies by a rounded `1 / det`
(`inv([[4, -9], [-9, -7]])` has `0.0825688073395` where `9 / 109` is
`0.0825688073394`). Add a new operation to `UNARY`, `BINARY`, `MATRIX`,
or `exact_case` when you add its key.

## Layout

- `src/float.rs` — decimal floats (`BigInt` mantissa, base-10 exponent). Precision is significant digits, default 12. `+ - * /`, sqrt, exp, ln, sin/cos/tan, inverse trig.
- `src/value.rs` — `Int` / `Rat` / `Float` / rectangular complex / integer modulo forms / vectors. Promotion, powers, roots, trig in degrees or radians.
- `src/vector.rs` — element-by-element mapping (`map1`, `map2`) and what Calc means by `*`, `/`, `^`, `A`, `I Q`, `f A`, `&` on a whole vector.
- `src/matrix.rs` — matrices: product, exact determinant, inverse, and `/` (fraction-free elimination on integers), integer powers, transpose, trace, identity, cross product, and the work budget.
- `src/cplx.rs` — complex kernels on decimal floats: `ln`, `ln1p`, `expm1`, `tan`, `tanh`, and the six inverse functions, with `refine`, which widens the working precision until both parts settle.
- `src/consts.rs` — physics constants (HP 35s / CODATA 1998), plus Earth, the Sun, the Moon, and the astronomical unit. `K` lists them.
- `src/parse.rs` — entry syntax.
- `src/engine.rs` — stack, undo, modes, operations.
- `src/app.rs` — key mapping. Test this without a tty.
- `src/ui.rs`, `src/main.rs` — arguments (`clap`) and the ratatui screen.
- `tests/reference.rs` — random comparison with PARI/GP and Emacs Calc, ignored by default.

## Number rules

- Integers stay exact (`num-bigint`). `:` entry and fraction mode use rationals (`num-rational`), shown as `n:d`.
- Default fraction mode is off, matching Calc. `/` of integers that divide evenly stays an integer; otherwise it is a float at the current precision. `m f` turns fraction mode on. `:` outside entry is fraction division.
- Floats are decimal, so `0.1 + 0.2` is `0.3`. A whole-number float keeps a trailing point (`6378137.`), including after `c f`. Notation follows the precision, as in Calc's normal format: a float is written in full while its order runs from -2 through one less than the precision, and in scientific form outside that. At precision 12, `4.2e10` is `42000000000.`, `1e11` is `100000000000.`, `1e12` and `1e20` stay scientific, `0.01` stays fixed, and `0.001` is `1e-3`. An exact integer result, such as `sin(90)`, has no point.
- An integer base greater than 1 takes an exact integer log when the value is a positive rational whose numerator and denominator are both integer powers of that base, including a reciprocal and a decimal that is that rational. `1:8 2 B` and `0.125 2 B` are `-3`. `1:1000 H L` is `-3`. A result that is not an integer, such as `8 4 B` (`log4(8) = 3/2`), stays a float.
- A negative square root, and non-real powers, become rectangular complex `(re, im)`. A zero imaginary part collapses back to a real. A fractional power of a negative number is the principal value, as in Calc and PARI/GP: `_8 1:3 ^` is `(1., 1.73205080757)`. The real odd root is `I ^`: `_8 3 I ^` is `-2`.
- A modulo form `a mod m` holds integers, with `m > 0` and `0 <= a < m`, as in Calc. Only integer moduli are supported; Calc's real moduli (`10.5 mod 360`) are not.
- A vector `[a, b, c]` holds any values, vectors included, of any length. Where Calc computes a result the key follows it, and where Calc leaves a vector unevaluated the key goes element by element. See the vector bullet under Status.
- A matrix is a vector of rows of one length, `[[1, 2], [3, 4]]`, of integers, fractions, floats, complex numbers, or modulo forms. `*` is the matrix product, `&` the inverse, `^` an integer power, and `B A /` solves `A X = B`, as in Calc. The determinant, the inverse, and `/` are worked out exactly, floats included, and rounded once. An inverse of exact entries follows `/`: an element that is not whole is a float unless fraction mode is on. See the matrix bullet under Status.
- Angle mode defaults to degrees. It applies to real trig and to both parts of complex trig, same idea as Calc. A degree angle is reduced modulo 360 before conversion, including a non-integer (`12345678901.5` is `181.5` degrees) and an integer float whose exponent is past 100000. `P` pushes pi. `H P` pushes e.
- Inverse (`I`) and hyperbolic (`H`) are one-shot prefixes.

## Keys

```
digits . e E _ - + : ( ) ,     entry
RET SPC   push, or duplicate the top if the entry is empty
DEL       backspace, or pop
TAB       swap the top two
← → C-b C-f   scroll long stack rows sideways by a screen
C-a C-e   scroll to the start, to the end (also Home, End)
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
k E       extended gcd: y on top, x under it; `[g, a, b]` with a*x + b*y = g, as Calc's egcd
k i       modular inverse: m on top, result b with 0 <= b < m and a*b ≡ 1 (mod m)
k p       prime test: 1 or 0 replaces the number, the message says why
k f       prime factors as a vector, smallest first. `2023 k f` is `[7, 17, 17]`. `V R *` rebuilds `2023`
k t       Euler's totient: `12 k t` is `4`
k n I k n next prime above, previous prime below
H k n     nth prime: `1000000 H k n` is `15485863`
k r       random below the limit on top: an integer of [0, m), a float of [0, x), a normal sample for 0, an element of a vector
k a       another random number from the last limit, pushed
k h       count on top, limit under it: that many picks without repeats, in random order
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
[ ] ,     vector entry: `[1, 2, 3]`, or `[1 2 3]`
v p       pack: pop n, then pack the n values under it into a vector
v u       unpack the top vector onto the stack
v x       index: n becomes `[1, 2, ..., n]` (up to 10000)
v l       length of the top vector
V M …     map: the next operation goes element by element
V R …     reduce: fold the next binary operation over the top vector
* / ^ &   on matrices: product, solve (`B A /` is the X with A X = B),
          integer power, inverse (`v &` too)
v t       transpose; a flat vector becomes a matrix of one column
V J       conjugate transpose
V D       determinant of a square matrix
V T       trace of a square matrix
v i       identity: n becomes the n by n identity matrix (up to 100)
V C       cross product of two vectors of three numbers
          `v` and `V` are the same prefix, as in Calc
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
- `2023 k f` is `[7, 17, 17]`, and `V R *` is `2023`; `[6, 8, 15] k f` is `[[2, 3], [2, 2, 2], [3, 5]]`, and `v u` leaves the three lists
- `43920492304 k f` is `[2, 2, 2, 2, 13, 5903, 35771]`
- `7 k p` = `1`, `8 k p` = `0`, `[2, 4, 7] k p` = `[1, 0, 1]`
- `3M7 [1,2,3,4,5,6] ^` = `[3 mod 7, 2 mod 7, 6 mod 7, 4 mod 7, 5 mod 7, 1 mod 7]`, `[1,2,3] 2 ^` = `14`, `[1,2] [3,4] *` = `11`, `[1,2] [3,4] V M *` = `[3, 8]`, `[3,4] A` = `5` (`vectors_follow_calc`)
- `[[1,2],[3,4]] [[5,6],[7,8]] *` = `[[19, 22], [43, 50]]`, `[[1,2],[3,4]] [5,6] *` = `[17, 39]`, `[[1,2],[3,4]] &` = `[[-2, 1], [1.5, -0.5]]` and `[[-2, 1], [3:2, -1:2]]` after `m f`, `[5,6] [[1,2],[3,4]] /` = `[-4, 4.5]`, `[[1,2],[3,4]] V D` = `-2`, `[[1,2],[3,4]] _2 ^` = `[[5.5, -2.5], [-3.75, 1.75]]`, `[[1,2],[3,4]] 0 ^` = `[[1, 0], [0, 1]]`, `[[1.5,2],[3,4]] &` is `singular matrix`, `[[2M6,3M6],[3M6,2M6]] &` is itself, `[1,2,3] [4,5,6] V C` = `[-3, 6, -3]`, `[[1,2],[3,4]] 2 V M ^` = `[[1, 4], [9, 16]]`, `100 v i 12345678901234567890123 * &` is `overflow` (the `matrix_*` tests in `src/app.rs`; `matrix_results_match_pari` has the float results from PARI/GP)
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

Algebraic entry (`'`), formulas, Calc's other `v` and `V` commands (the LU decomposition `V L`, the norms `v n` and `V N`, `v d`, `v r`, `v c`, the Kronecker product, sets, sorting), a matrix shown on several rows, a real power or a function of a matrix as a matrix (`Q` and `E` go element by element), real moduli, units, dates, HMS, the trail window and its other `t` commands, numeric prefixes (`C-u`), polar display, infinities.

## Status

Working RPN calculator. `cargo test` is green (float, value, parser, keys, and a headless screen test). `cargo build` produces the ratatui UI. That UI was checked with `TestBackend`, not by typing in a live terminal.

The screen keeps the terminal's own background. A dark terminal uses the GrokNight foregrounds; a light one uses dark text. On Unix, startup asks for the background with OSC 11 and treats no answer as dark. Windows keeps the dark foregrounds. The headless screen test still paints the GrokNight canvas (`#141414`). The live screen has a rounded `❯` prompt with the mode on its bottom border, and a shortcut bar (`ret:push`, bold key, muted label). Content sits two columns in from each edge. Under the prompt is a blank row, then the shortcuts, then a blank row. The typed text sits one column inside the prompt border. `?` opens a Shortcuts box centered on the input's width, in the space above the prompt: the same number of rows sits above the frame as between the frame and the input, and an odd leftover row sits between the frame and the input. Gray `┌┐` frame, bold title on the top edge, `[x]` at the top right, and `1/2` on the note row when another page exists. Bindings fill down up to three columns of up to 15 rows (a narrow or short terminal uses fewer), bold key and muted label. The order (`help` in `docs.rs`) keeps each group and each prefix together, and at 3 by 15 every column starts a group. Page one is the stack and arithmetic, the one-argument functions (`I`, `H`, then `n` to `f I`), and constants, trig, and logs. Page two is `!` and every `k` key, then `k r`, `k a`, `k h`, entry (`:`, `M`, `c f`, `c F`), `p`, `K`, the trail, and the `s` keys, then the vector and matrix keys as one column. The last twelve, page three, are the angle and fraction modes and the display keys. The box keeps the size of the first page on every page. Tab and Shift-Tab turn the page and wrap. The footer shows `tab next`, `S-Tab prev`, `? docs`, and `esc close`. `q quit` stays on the window bar. `?` again opens Commands, in the same frame. The top row is a search field. Several matches show a short list and the selected page under a rule. One match shows only that page. Up and Down, or Tab and Shift-Tab, move and wrap. Backspace edits the search. Esc closes. `?` returns to the shortcut list. `q` is a search letter there.

Entry and stack details that are easy to get wrong:

- Keep `num-bigint` at 0.4.8 (`num-rational` 0.4 needs that major).
- `k E` is Calc's `egcd`: `[g, a, b]` with `a*x + b*y = g`, `y` on top. `35 15 k E` is `[5, 1, -2]`, and `v u` puts the three on the stack. It goes through `binop`, so it maps over vectors and rejects a modulo form. The reference tests compare it with Calc's `egcd`; PARI's `gcdext` may choose other coefficients.
- Random numbers (`random.rs`) follow Calc's `random` and `shuffle`. They come from `rand::rng()` (rand 0.10): ChaCha12, seeded from the operating system and reseeded every 64 kB. `k r` replaces the limit on top: an integer `m` gives an integer of `[0, m)`, or of `(m, 0]` when `m` is negative; a float or a fraction `x` gives `x` times a float of `[0, 1)` with every digit of the precision random (`30 p 1. k r` has 30 random digits); `0` (or `0.`) gives a standard normal sample by Box-Muller at guard precision; a vector gives one of its elements, and `[]` is `random of an empty vector`. A complex number or a modulo form is `random needs a real number or a vector`. Integers are drawn by rejection on random bits (`below`), so a range of any size, `10^50` included, is exactly uniform; Calc reduces a few extra digits modulo the limit, which leans a little towards the small values. `k a` pushes another draw from the last limit `k r` or `k h` used, `1.` before any, as in Calc. `k h` takes the count on top and the limit under it, as Calc's `shuffle(n, max)`: distinct picks in random order from `[0, m)`, `(m, 0]`, a vector's elements, or repeated `random` of a float or `0`. More picks than values is `shuffle asks for more values than there are`, and more than 10000 is `overflow`. A range up to 40000 is shuffled in place (the first `n` steps of Fisher-Yates); a larger one samples with a set of the values seen. The tests check ranges, distinctness, that every value of a small range and every order of four turn up, and the mean and spread of normal samples.
- `DecFloat::from_decimal(mantissa, exponent, prec)` builds a typed decimal. The stored precision is the number of significant digits typed, capped at the current precision.
- A typed exponent, decimal or radix, outside ±1000000 is overflow. `1e1000000` and `1e-1000000` still parse. An exponent that does not fit in a 64-bit integer is a bad exponent.
- `q` quits when nothing is being typed. While radix or word size is being typed, `q` cancels that prompt and leaves the stack. A variable name can contain `q`. `q` does not quit while `K`, `s l`, or the command reference is open. The shortcut list pages with Tab and Shift-Tab, and `q` still quits there. `p` is not a prompt: it pops an integer and sets the precision.
- Do not polish add, sub, mul, or div. `(1/3)*3` can display as `0.999…`, and `sqrt(2)^2` as `1.999…`.
- `snap_close_integer` in `float.rs` snaps only a guard-digit speck: the unrounded value must be within three digits past the last one shown. Judging the rounded value would snap a true result one unit below a power of ten. `0.999999999998 Q` is `0.999999999999`, `_1e-12 E` is `0.999999999999`, and `10 L E` is `9.99999999996`, as in Emacs Calc and PARI/GP. At precision 16, `1 E L` is `0.9999999999999999`, because 16-digit e rounds down.
- A float exponent saturates inside `DecFloat` and never wraps. `value::in_range`, called from `Engine::unary_mod` and `Engine::binop_mod`, turns a result whose order is past ±10^18 into `overflow`. Squaring `2.5e999999` 45 times is an overflow, not a wrapped exponent.
- `pow_int` in `value.rs` sends a float base to `DecFloat::powi`, which carries one guard digit per exponent bit. Do not multiply a float at the display precision in a loop: the error grows with the exponent. An exact base is limited by the size of the result (`e * log2` of the larger of numerator and denominator against `MAX_POWER_BITS`, about 2.5 million digits), not by the exponent, so `2. 10000000 ^` and `1 2000000 ^` answer and `10 100000 ^ 100000 ^` is `overflow`. The numerator and denominator are raised separately with `BigInt::pow` and joined with `new_raw`: they share no factor, and multiplying fractions in a loop ran a gcd on million-digit numbers (`1:3 4194303 ^` took 96 s). A whole-number float exponent gives a float, as in Calc: `2 3. ^` is `8.`, and `5013800. H E`, `10 5013800. ^`, and `10. 5013800 ^` are all `1e5013800`. An integer exponent keeps an exact base exact (`3 H E` is `1000`). A negative power is a float unless fraction mode is on (`3:2 _2 ^` is `0.444444444444`). A float of magnitude 1 is `±1.` for any exponent, and an exponent past `i64` goes through exp/ln. A complex base still caps the exponent at 20 bits.
- `pow_exp_ln` adds one guard digit per integer digit of `exp * ln(base)` (up to 40). Without them `_1.23e600000 1000001:3 ^` was wrong from the eighth digit.
- `pi / n` is `DecFloat::pi_over(n, prec)`, rounded once. `pi(prec) / 2` gave `1.5707963268` for `m r 1 I S`.
- Avoid a difference that cancels. Complex `sqrt` takes the larger part from `radius + |x|` and the other as `y / (2 * larger)`. `acos` is `atan2(sqrt((1 - x)(1 + x)), x)`. Real `asinh`, `acosh`, and `atanh` use `ln_1p` forms (`asinh_float`, `acosh_float`) and the odd symmetry. Real `atanh` past 1 is `(atanh(1/x), -pi/2)`, odd in `x`, with `atanh(1/a) = ln1p(2 / (a - 1)) / 2`: `20 H I T` is `(0.0500417292785, -1.57079632679)`. Real `acosh` inside `[-1, 1)` is `(0, acos x)`: `_0.26 H I C` is `(0, 1.8338185297)`. `asin` of a real past 1 is `(±90, ∓acosh|x|)`. `acos` past 1 is `(0, acosh x)`, and `(180, -acosh|x|)` below -1. Real `acosh` below -1 is `(acosh|x|, pi)`. Real `tanh` is `-t / (2 + t)` with `t = expm1(-2|x|)`, rounded once (`sinh / cosh` missed the last digit half the time), and past `2 * precision + 10` it is `±1.`.
- In degrees, an inverse trig result stays at `prec + 8` (`angle_prec`) until `from_radians` has scaled it. Scaling a rounded angle gave `0.5 I C` = `60.0000000002`.
- `reduction_prec` in `float.rs` adds digits only for a large argument. A tiny one needs none: `1e-3000 E` is `1.` and `1e-3000 S` is `1.74532925199e-3002`.
- Complex transcendentals go through `src/cplx.rs`, not through `ln`, `sqrt`, and `*` at the display precision. Composed that way `m r (_3461328.16571, 17) H I S` came out as `(-11.5, pi)`; it is `(-15.7503101169, 4.91140948965e-6)`. `asin`, `acos`, `asinh`, and `acosh` use the Hull, Fairgrieve, and Tang forms. `atanh` is `(ln1p(4a / ((1 - a)^2 + b^2)) / 4, atan2(2b, (1 - a)(1 + a) - b^2) / 2)`, and `atan(z)` is `-i atanh(iz)`. `ln|z|` near the unit circle is `ln1p(x^2 + y^2 - 1) / 2` with the sum taken exactly. `tan` is `(sin x cos x, sinh y cosh y) / (cos^2 x + sinh^2 y)`. `refine` runs each kernel at `prec + 10` and `prec + 22` and widens (up to `4 prec + 80`) until both parts agree; that covers the cancellation a formula cannot avoid, such as `expm1` with `x` near `y^2 / 2`. On the imaginary axis `asinh` and `atan` take the sign of `y` for the real part, as PARI/GP does. PARI at 12 digits is itself wrong for some of these, so take the reference at 30 or more.
- Complex trig in degrees converts each part on its own (`mode_sin_cos`, `mode_radians`). The real part is reduced modulo 360 exactly and a multiple of 90 is exact, so `(_7200000, _12) T` is `(0, -0.206429956437)`. A fraction of a degree is divided at the working precision (`reduce_degree_rational`): `30 p 1:3 S` was wrong from the seventeenth digit.
- `finish_complex` runs once, on a finished result. Dropping the tiny imaginary part of `cosh` before the division made the imaginary part of `(_0.01, 9e-11) H T` wrong in the fifth digit. `B`, `H L`, and `^` take their logs from `ln_keep`, which skips it. Emacs Calc drops such a part too (`exp((0, 3.14159265359))` is `-1.`), and PARI/GP keeps it.
- A complex product keeps the four products in full and rounds each sum once (`mul`). `f E` is one rounding of `expm1` at guard precision, and `-1.` for a large negative argument. `f L` below -1 is `(ln|1 + x|, pi)`. Zero to a positive real power is `0`.
- A function takes an exact argument (an integer or a fraction) through `Real::operand`, which converts it with the guard digits the function works in. A float is the number it is. Rounding `167:166` to 12 digits first left `L` with nine good digits (`6.00602406464e-3`, where it is `6.00602406021e-3`), and Emacs Calc keeps guard digits here too. In radians `to_radians` keeps every digit of a large exact angle and the caller no longer rounds it again (`keep_float`): `m r 123456789012345678 S` was `0.611883915853` and is `-0.998852840435`. `+ - * /` of a fraction and a float still convert the fraction at the display precision first, as Calc does; PARI/GP keeps the fraction exact, so `439:438 0.9991 -` is `3.18310502e-3` here and `3.18310502283e-3` there.
- `+ - * /` keep a tiny complex part: `(1, 1e-20) (0, 1) *` is `(-1e-20, 1.)`. Only transcendental results drop a part more than `prec` orders below the other (`finish_complex`).
- `c F` runs the continued fraction until the half-ulp tolerance is met. It has no term cap: 80 terms stopped short past about 80 digits.
- `ln` leaves a value in `[0.7071, 1)` unscaled. Scaling it to `9.99…` and subtracting `ln(10)` cancelled: `20 p 0.9999999999999999995 L` kept one digit, and is `-5.0000000000000000013e-19`.
- `half_index` in `comb.rs` doubles a float with one extra digit before asking whether it is a half-integer. At the typed precision `5.5001 !` rounded to `11` and answered `5.5!`. It is `287.936897959`, and `500.3 !` is `7.87533108633e1134`.
- An integer factorial is a product tree (`range_product` in `comb.rs`): `100000 !` takes about 0.1 s, where a running product took 0.8 s. `k c` with both arguments negative and `k <= n` is `C(n, n - k)`, as in Calc and PARI/GP: `_3 _5 k c` is `6`, where it used to be `0`. `k c` and `H k c` use the same tree: `k c` is `(n-k+1) ... n` over `k!` with one exact division, so `1000000 100000 k c` takes 0.13 s instead of 3.8 s.
- A non-integer factorial is Spouge's formula (`factorial_float` in `comb.rs`). Its coefficients alternate in sign and reach about `10^(0.555 a)` for `a` terms, and the sum they leave can be as small as `sqrt(2 pi)`, so `spouge_work` carries `0.56 a` guard digits, plus one per integer digit of the exponent `(z + 1/2) ln(z + a)`. A third of `a` was enough only for a small argument: `100 p 972.32 !` had ten wrong digits. `spouge_coefficients` builds `e^1 .. e^(a-1)` by repeated multiplication. A separate `exp` per coefficient made `1000 p 0.3 !` run for minutes; it takes about 1.6 s.
- `DecFloat::sqrt` is the integer square root (`BigInt::sqrt`) of the mantissa scaled to twice the working digits with an even exponent. Newton on decimal floats started from one digit and ran every step at full precision.
- `pow10` in `float.rs` keeps powers of ten below `10^4096` in a per-thread table. Every rounding asks for a few, and raising 10 each time was about 60% of a 1000-digit `exp`.
- `digit_count` and `strip_trailing_zeros` in `float.rs` run on every rounding. Do not print the mantissa to count or strip digits. That is quadratic and made `exp` at precision 1000 take about 2s instead of 0.3s.
- An unparsable radix literal names the whole literal: `16#` says `bad entry: 16#`.
- Modulo forms follow Calc. `M` while typing an integer inserts ` mod ` (`3M1000` is `3 mod 1000`), and a second `M` fills in the previous modulus. `M` is still a digit in a radix literal above 22 (`36#mM7` is one number). DEL removes ` mod ` in one piece, a space waits for the modulus, and in `ec -e` a word ending in `M` continues into the next (`3M 1000`). A bare `M` says `type an integer before M`, and `3.5M` says `modulo forms need integers` and keeps the entry. The value is `Value::Mod(a, m)`, always reduced. `+ - * / ^`, `n`, `&`, and `I Q` take a form with an integer (an exact integer float is accepted; a whole float past the integer bound, such as `1e100001`, is `overflow`) or a form of the same modulus. After ` mod `, `(`, `,` and `)` are not entry keys. Different moduli say `modulo forms need the same modulus`. `/` and `&` multiply by the modular inverse, so `3M6 2 /` says `no modular inverse` and a zero divisor is `division by zero`. `^` uses `BigInt::modpow`, so an exponent such as `10^30` is fine. A negative power inverts first. A form used as the exponent contributes its residue, as in Calc. `0M7 0 ^` is `1 mod 7`, where Calc prints a plain `1`; PARI/GP agrees with `1 mod 7`. Display formats both sides in the current radix (`16#3 mod 16#7`).
- `Engine::unary` and `Engine::binop` reject a modulo form with `not for modulo forms`. Only the operations above use `unary_mod` and `binop_mod`, so a new operation cannot take a form by accident. The check is made on each element, so `[3M7] S` is that error too. Operations that read the stack themselves (`p`, `v p`) reject it through `exact_int`.
- Vectors follow Calc (`vector.rs`). All four of `unary`, `unary_mod`, `binop`, and `binop_mod` go element by element through a vector at any depth (`map1`, `map2`): a number meets every element, two vectors pair their elements and need the same length (`vectors need the same length`; Calc silently drops the extra elements). So `+ -`, a number times a vector, `n`, `F`, `c f`, and every function work element by element, and a new operation does too. The keys Calc defines on a whole vector go through `unary_whole` and `binop_whole` instead: `*` of two flat vectors is the dot product, with nothing conjugated (`[1,2] [3,4] *` is `11`), its products kept in full and the sum rounded once (`matrix::sum_products`); `v^n` for an exact positive integer `n` repeats that product, so `[1,2,3] 2 ^` is `14` and `[1,2,3] 3 ^` is `[14, 28, 42]`; any other power goes element by element, because Calc leaves it unevaluated (`[1,2,3] 2. ^` is `[1., 4., 9.]`, `[4,9] 0.5 ^` is `[2, 3]`, `[1,2] 0 ^` is `[1, 1]`), except a negative integer, which is `a vector has no negative power`; `I Q` is the dot product with itself; `A` is the length (`[3,4] A` is `5`); `f A` is the sum of squared magnitudes; `&` of a flat vector and dividing by one are `not for vectors`. A vector of rows of one length is a matrix, with its own rules in the next bullet; any other vector with a vector in it is `not a matrix` to these keys. Where Calc leaves a vector unevaluated (`2^[1,2,3]`, `sqrt([4,9])`, `fact`, `prime`, `prfac`, `totient`), the key goes element by element, so `3M7 [1..6] ^` is the six powers. `V M` and `V R` are one-shot, like `I` and `H`: the key map keeps them until an operation runs (`App::run` hands them to `Engine::set_vector_mode`), the mode line shows `map` or `reduce`, the trail label starts with `V M` or `V R`, and Esc clears them. Under `V M` the whole-vector keys go element by element too (`[1,2] [3,4] V M *` is `[3, 8]`). Under `V R` a binary operation folds left over the top vector (`[1,2,3,4] V R +` is `10`). A matrix (every element a vector of numbers, all one length) folds over all its elements, as Calc's `reduce`: `[[1,2],[3,4]] V R +` is `10` and `V R *` is `24`; anything else folds row by row (`[1,[2,3]] V R +` is `[3, 4]`). An empty vector gives `0` for `+` and `-` and `1` for `*` (`reduce_identity`), and is `V R of an empty vector` for anything else. A one-argument key is `V R needs a binary operation`. `value::in_range` checks every element. Typing: `[` starts a vector, `]` ends it, and each element follows the rules for a number typed alone, so `(1, 2)`, `16#FF`, `3M7`, `_2`, `-2`, and a nested `[…]` work inside (`Entry::element`, `vector_accepts`). Commas separate the elements; without commas, spaces do, as in Calc (`[1 2 3]`; Calc multiplies in `[1, 2 3]`, which is a bad entry here). A space at the top of an open vector ends the element, so `[1 2n]` is `[1, -2]` and `[3M7 4M7]` has two forms. `Entry` keeps a `Scan` (open brackets and parens, where each element starts, whether a space just ended one) and updates it key by key, so typing or pasting a 40000-element vector is linear; only DEL, `M`, and `n` rescan. `n` while typing changes the sign of the element being typed, a finished inner vector included (`[[1,2]n]` is `[[-1, -2]]`), and after the closing `]` it negates the vector. A second `M` takes the modulus typed earlier in the same entry (`[3M7,4MM]`), else the last modulus entered, which is also taken from inside a vector. A value or `[` typed straight after a finished one starts the next entry: `[1,2][3,4]*` is `11`. In `ec -e` the space between words stays inside an open `[…` or `(…`. A vector prints as `[1, 2, 3]` in the current display mode and is one stack row, cut with `$` and scrolled like a long number. `v x` stops at 10000 elements (`MAX_INDEX`) because undo keeps up to 100 copies of the stack, and it does not map over a vector, which would get round the cap. Vectors nest at most 100 deep (`MAX_DEPTH`): entry refuses a deeper `[`, `parse_vector` and `v p` refuse one, and `value::in_range` checks every result, so parsing, printing, and dropping never recurse without bound (3000 levels overflowed the stack).
- Matrices follow Calc (`matrix.rs`). A matrix is a vector of rows: every element a vector of numbers, all of one length, at least 1 by 1 (`matrix::shape`). `[]` and a vector of numbers are flat. `+ -`, a number with `*` or `/`, and every function still go element by element (`map1`, `map2`), and `V M` makes `* / ^ &`, `A`, `I Q`, and `f A` do so too (`[[1,2],[3,4]] 2 V M ^` is `[[1, 4], [9, 16]]`). `V R` is as before. `*` is Calc's `math-mul-objects-fancy`: a matrix times a matrix needs the inner dimensions to agree; a matrix times a flat vector takes it as a column and gives a flat vector (`[[1,2],[3,4]] [5,6] *` is `[17, 39]`); a flat vector times a matrix takes it as a row (`[5,6] [[1,2],[3,4]] *` is `[23, 34]`); a one-column matrix times a flat vector of as many elements is the outer product (`[[1],[2]] [3,4] *` is `[[3, 4], [6, 8]]`), which is Calc's rule. Anything else is `dimensions do not match`. Calc's simplifier then tries the product the other way round, so `[[1,2,3],[4,5,6]] * [1,2]` is `[9, 12, 15]` there; here it stays an error. `I Q` is the matrix square. `&` (and `v &`) is the inverse of a square matrix: a flat vector is still `not for vectors`, another shape is `needs a square matrix`, and a zero determinant is `singular matrix`. `B A /` is Calc's `math-div-objects-fancy`: the `X` with `A X = B`, that is `A^-1 B`, for `B` a matrix of as many rows as `A` or a flat vector of that length (`[5,6] [[1,2],[3,4]] /` is `[-4, 4.5]`). A matrix `B` whose rows do not fit and whose columns do gives `B A^-1`, and a number gives the number times the inverse. A matrix divided by a flat vector is `not for vectors` (Calc recurses until Emacs gives up). `:` still goes element by element, because Calc leaves `fdiv` of a vector unevaluated; `m f` and `/` give the exact solution. `^` of a square matrix by an exact integer is the matrix power by repeated squaring: `0` is the identity and a negative power is a power of the inverse. Any other exponent goes element by element, as for a vector, because Calc leaves it unevaluated (`[[1,2],[3,4]] 2. ^` is `[[1., 4.], [9., 16.]]`). `V D` is the determinant, `V T` the trace, `v t` the transpose (a flat vector becomes one column, a number stays), `V J` the conjugate transpose, `v i` the identity of the order on the stack (Calc prompts for it; `0 v i` is `[]`), and `V C` the cross product of two flat vectors of three numbers. `f A` is the sum of the squared elements, as Calc's `abssqr`, and `A` is its root (Calc leaves `abs` of a matrix unevaluated). A decimal float is an exact rational, and the arithmetic uses that. An element of a product is a sum of products, and with a float in it the sum is taken exactly, as decimals, and rounded once (`sum_products`, which the dot product, `V T`, and `V C` use too): `[1e10, 1, _1e10] [1e10, 1.5, 1e10] *` is `1.5`. Only modulo forms, and exponents more than 4096 digits apart, go through `value::mul` and `value::add` at twice the precision. The determinant, the inverse, and `/` turn `[A | B]` into integers (each row times the lcm of its denominators; powers of ten taken out of each row and column first, so `[[1e-200000, 2],[3e-200000, 5]] &` costs nothing extra) and run fraction-free Gaussian elimination (Bareiss) and back substitution on them (`reduce`), on `BigInt`, or on Gaussian integers when an entry is complex. Every number it makes is a minor of the matrix and every division is exact. So no pivot is round-off, a matrix is singular only when its determinant is exactly zero (`[[1.5,2],[3,4]]` and `[[0.1,0.2],[0.3,0.6]]` are, and their `V D` is `0.`), and each result is rounded once whatever the condition number. Calc eliminates in floats: `[5,6] / [[1,2],[3,4]]` is `[-3.99999999997, 4.49999999998]` there. An exact determinant stays exact (`[[1:2,1:3],[1:4,1:5]] V D` is `1:60`). An exact inverse or quotient follows `/`: an element that is whole is an integer, and any other is a float unless fraction mode is on (`[[1,2],[3,4]] &` is `[[-2, 1], [1.5, -0.5]]`, and `[[-2, 1], [3:2, -1:2]]` after `m f`). Calc keeps a fraction that came from fractions, as its `/` does. With a float anywhere, every result is a float. A 1 by 1 inverse is a 1 by 1 matrix: `[[2]] &` is `[[0.5]]`, where Calc gives the number `0.5`. Modulo forms are lifted to integers, eliminated, and reduced, and the solution is multiplied by the inverse of the determinant. The forms need one modulus, an integer beside them takes it, and any other number is `modulo forms need integers`. A composite modulus works whenever the determinant is a unit: `[[2M6,3M6],[3M6,2M6]] &` is itself, where no entry is a unit and Calc finds no pivot. A zero determinant is `singular matrix` and another that is not a unit is `no modular inverse`. A power multiplies with `sum_products`. Floats are rounded after every product, and a later product can cancel the digits that were kept: `M^-2` squares an inverse whose entries may be far larger than the answer, and a 30-digit case was 179 ulp off. So a float power runs with guard digits (one per exponent bit and six more) at `prec + 4` and at `prec + 16`, and wider (up to `4 prec + 80`) until two runs agree to the display precision, as `cplx::refine` does (`power`, `agrees`). A negative power of exact entries inverts exactly and is settled at the end, like `/`. A matrix of forms takes any exponent (`10^30`), and its zeroth power is the identity of `1 mod m`, where Calc has a plain `1`. Limits: `v i`, the determinant, the inverse, `/`, and `^` take up to 100 by 100 (`MAX_DIM`, 10000 elements, as `v x`), and a product of more than 10000 elements is `overflow`. Elimination and powers are cubic, and the engine cannot be interrupted, so `Budget` estimates the time from the size and the length of the numbers (dividing one minor by another is quadratic in their length) and stops with `overflow` past about five seconds' worth: a 100 by 100 inverse of 12-digit numbers fits (2.5 s) and one of 16-digit numbers does not, and `100 v i 10 30 ^ ^` stops after about two seconds. An exact result may hold about a megabyte of digits in all (`MAX_EXACT_BYTES`), like an exact power of a number: `[[1,1],[1,0]] 100000000 ^` is `overflow`. Exponents that stay more than 100000 orders apart in one matrix after the scaling are `overflow`, as each would be that many digits. That also refuses a determinant a float could hold, such as `[[1e999999,2],[3,1e999999]] V D`; it is a known limit. A quotient of the integers becomes a float in one rounding (`ratio`): the integer quotient keeps `prec + 2` digits and a sticky digit for any remainder. Rounding the numerator and denominator to floats and then dividing rounded three times, and `30 p [[3.0000000000149999999999]] 12 p V D` came out `3.00000000002`. Exact sums of fractions (`exact_total`) add over a running product of the denominators and reduce once; a gcd after every addition made a 100 by 100 product of 6-digit fractions take 40 s, and it takes 1 s. The budget charges that final reduction (`Budget::fractions`) and the shifting of float terms whose exponents lie far apart (`Budget::spread`), and a product stops at `MAX_EXACT_BYTES` as it builds the result, so a product of long fractions is `overflow` at once instead of after minutes. Powers of ten come from `float::pow10`'s cache. `f A` and `A` take `x conj(x)` through `sum_products`, so the sum is rounded once.
- `L` flips inverse in `app.rs`, then uses the exp/ln family. `E` does not flip. `push` and a successful duplicate do not clear `I` or `H`.
- `n` while typing toggles a leading `_`. `+` and `-` while typing are exponent signs, or digits inside an unclosed `(…)`. Otherwise they add and subtract. Space inside an unparsed `(…` inserts a space. `:` while typing is a fraction; on an empty entry it is fraction division.
- `ec -e` feeds words through the same keys. Whitespace, including newlines, is a separator, and a finished number is pushed at the end of each word. `2 20 ^ d g` prints `1,048,576`. Both `d g` and `dg` group. A space on the screen still pushes or duplicates. An unclosed `(…` stays open across a space, so `(3, 4)` is one value. `12 p` sets the precision and pops 12. A prompt finishes when its answer is complete (`d r 16`, `b w 64`, `s s name`). The command prints the top value. An error goes to stderr and the process exits 1. `q` and `?` stay on the screen.
- Stack index 0 is the oldest value. The top of the stack is the last element, drawn at the bottom, with a bold `.` on the last row. The screen does not print level numbers. Values and the `.` start on the same column as the character after `❯ `. Each value is one row, as in Emacs Calc, which does not break a number (`calc-line-breaking` applies to formulas only). A value wider than the row is cut: `$` in the last column when more follows, `$` in the first when text is hidden to the left, and a lone `$` when a short row is scrolled past its end (`hscroll_row` in `ui.rs`). Left and Right (`C-b`, `C-f`) scroll every row together by the width less the two `$` columns, so no character is skipped; `C-a`/Home and `C-e`/End jump to the start and to where the end of the longest visible row reaches the last column (`app::hscroll`). Any other key returns to the start. While a shown row is cut, the shortcut bar adds `←→:scroll` before `?:help`; below about 84 columns that pushes `q:quit` off the bar. Values that do not fit vertically are left off the top. The screen reports its widths through `App::set_stack_view` and `App::set_trail_width` while drawing.
- `F` is floor toward minus infinity and returns an exact integer. An integer is unchanged. A fraction or a decimal float is exact (`-4:3` is `-2`). A complex number is an error. `f` is still the function prefix.
- `k s` is the signed Stirling number of the first kind `s(n, m)`, and `H k s` is the second kind `S(n, m)`, with `m` on top. Both arguments are nonnegative integers, and an exact integer float is accepted. `m > n` is `0`, and a negative or fractional argument is an error. The recurrence table is capped: `n` above 2000 is `overflow`, though `m` of `0`, `1`, or `n` is answered directly.
- `k i` is the modular inverse of `a` modulo `m`, with `m` on top. The result is the integer `b` with `0 <= b < m` and `a*b ≡ 1 (mod m)`. `3 7 k i` is `5`. Both values are integers, and an exact integer float is accepted. A non-positive modulus says `modulus must be positive`. If the gcd is not 1 the stack stays and the message is `no modular inverse`. Emacs Calc gets the same `b` as the reciprocal of a modulo form.
- `k f` replaces the top integer with the vector of its prime factors, smallest first, as Calc's `prfac`. `2023 k f` is `[7, 17, 17]`, and `V R *` (or `v u * *`) is `2023`. A negative number leads with `-1`. `0` and `1` give `[0]` and `[1]`. An exact integer float is accepted. A vector of integers gives a vector of factor lists. Trial division uses Calc's prime table through `5003`, then Pollard rho (Brent) splits a composite cofactor up to 1024 bits. `43920492304 k f` is `[2, 2, 2, 2, 13, 5903, 35771]`. A cofactor that cannot be proven prime or split stays in one piece, the stack still changes, and the message is `may not be fully factored`.
- `k p` replaces the top integer with `1` if it is prime and `0` if not, as Calc's `prime` function (Calc's `k p` key only prints a message and keeps the number). The message is `prime`, `not prime`, or `probably prime`, and a probably prime number gives `1`. Anything below 2, a negative number included, is `0`. A vector is tested element by element; the message is then `some are only probably prime` when that applies, and empty otherwise. RET first keeps the number.
- `k t` is Euler's totient, as Calc's `calcFunc-totient`. `0` and `1` stay, a negative number is an error (PARI/GP would take the absolute value), and an exact integer float is accepted. It uses the `k f` factorization, so it answers past Calc's trial-division limit (`1000000007 k t` is `1000000006`, which Calc leaves unevaluated). If a cofactor cannot be proven prime or split, it says `number too big to factor` and leaves the stack.
- `k n` and `I k n` follow Calc's `nextprime` and `prevprime`. `k n` truncates a non-integer and gives `2` for anything below 2. `I k n` rounds a non-integer up and gives `2` for anything below 4, so `2 I k n` is `2`, as in Calc. Past the proven bound of the prime test the answer replaces the top with the message `probably prime`. `H k n` is the nth prime, PARI/GP's `prime(n)`, for `1 <= n <= 10^8` (above is `overflow`). It sieves odd numbers in segments up to the Rosser bound `n (ln n + ln ln n)`; `10^8` takes a few seconds in a release build.
- `K` opens the constants, sixteen to a page, eight down each column. `0`–`9` and `a`–`f` insert that slot and close. Tab and Shift-Tab turn the page and wrap around. The last page has the remaining physics constants, then Earth mass (`Me`), Earth equatorial radius (`Re`, `6378137` m), Moon mass (`Mm`, `7.346e22` kg), Moon equatorial radius (`Rm`, `1738100` m), the mean Earth-Moon distance (`dm`, `384400000` m), Sun mass (`Ms`), Sun radius (`Rs`, `695700` km written in metres), and the astronomical unit (`AU`, `149597870700` m). Every slot is filled. A missing slot would say "no constant" and stay open. Esc closes. `q` does not quit while it is open. The physics numbers are the HP 35s CODATA 1998 values. The planetary numbers are the NASA Earth, Sun, and Moon fact sheets, in kilograms and metres. An integer literal stays an integer (`299792458`, `101325`, `6378137`, `149597870700`, `1738100`). The unit is only a label in the menu and the message (`c  m/s`, `Me  kg`). `e` is not listed; `H P` pushes it. `K` is still a digit while a radix literal above 20 is being typed.
- `atan2` of a value on an axis is an exact right angle (`0`, `±90`, `180` in degrees), including `atan2(0, 0) = 0` and `atan2(0, negative) = 180`.
- The trail (`t y`) replaces Calc's trail window with a dialog. `Engine::record` logs every pushed number and every result: `push`, `unary_mod`, `binop_mod`, `recall`, `k E` and `k f` (one vector each), `k n`, `k r`, `k a`, `k h`, `v p`, `v x`, and `V R`. `v u` is not logged. Duplicate, swap, roll, and pop are not logged, and neither is a value pushed back from the trail. The label is the command's keys (`*`, `I S`, `H k n`, `K c`, `rcl x`), set by `App::label_command` before the command runs, with `H` before `I` whichever was typed first; a typed number has an empty label. The full `Value` is kept and shown in the current display mode. Undo does not remove entries. The caps are 1000 entries (`TRAIL_CAP`) and 64 MB of digits (`TRAIL_BYTES`, by `Value::approx_bytes`), oldest dropped first, for the session only. In the dialog, newest at the bottom, typing filters by label or value text (grouping commas ignored on both sides, so `31415` finds `31,415,926…` under `d g`), Up and Down (or Tab and S-Tab) move and wrap, the scroll keys move the selected row sideways and moving resets it, RET pushes (undoable), Esc closes, and `q` is a search letter. Opening the dialog formats every entry once into `TrailView` (display text cut at 512 characters until that row is scrolled, which formats it whole, plus an ungrouped search text); typing and moving only compare those strings, and the draw reads them. A label longer than its column (8 to 16 wide) ends in `..`; a value wider than its column is cut with `$`, as on the stack. `ec -e` rejects `t y` with `the trail is for the screen`.
- Undo stores the stack and the variables, cap 100. Mode changes are not undoable. A new stack change clears redo. `s s` stores without popping, `s t` pops into the variable, `s r` recalls, and `s u` unstores. A digit selects `q0`–`q9` (`s 3`, `t 3`, `r 3`). A name is finished with RET. `s l` lists them, five per column. Tab and Shift-Tab page and wrap around. A letter recalls that one-letter name, and a digit recalls `q0`–`q9`. Longer names are visible there and still recalled with `s r`.
- Radix is display and entry only. A number typed without `#` is decimal. `d 2`, `d 8`, `d 6`, `d 0`, and `d r` set the radix (2–36). `#` uses the current radix; `16#FF` forces one. Letters are digits only after `#` when the radix is above 10. For radix 14 or less, `e` is a power of that radix (`8#1.2e3`). From 15 up, `e` is a digit and the power is `16#1.2*16^3`. `d z` pads integers to the word size. `b w` sets that width (default 32, negative allowed, display uses the absolute value). `O` before a radix command turns on two's complement for radices 2, 8, and 16 (`16##FD` is an 8-bit −3). Setting the radix without `O` turns it off. `radix##digits` reads a two's-complement word. `d g` groups integer digits with commas: 4 in binary and hex, 3 otherwise. The fractional part and the exponent stay plain, so at precision 13 `1234567.123454` displays as `1,234,567.123454`. At the default 12 the entry rounds to `1,234,567.12345`. Grouping is display only.
