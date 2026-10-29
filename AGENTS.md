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

## Layout

- `src/float.rs` — decimal floats (`BigInt` mantissa, base-10 exponent). Precision is significant digits, default 12. `+ - * /`, sqrt, exp, ln, sin/cos/tan, inverse trig.
- `src/value.rs` — `Int` / `Rat` / `Float` / rectangular complex. Promotion, powers, roots, trig in degrees or radians.
- `src/consts.rs` — physics constants (HP 35s / CODATA 1998), plus Earth, the Sun, the Moon, and the astronomical unit. `K` lists them.
- `src/parse.rs` — entry syntax.
- `src/engine.rs` — stack, undo, modes, operations.
- `src/app.rs` — key mapping. Test this without a tty.
- `src/ui.rs`, `src/main.rs` — arguments (`clap`) and the ratatui screen.

## Number rules

- Integers stay exact (`num-bigint`). `:` entry and fraction mode use rationals (`num-rational`), shown as `n:d`.
- Default fraction mode is off, matching Calc. `/` of integers that divide evenly stays an integer; otherwise it is a float at the current precision. `m f` turns fraction mode on. `:` outside entry is fraction division.
- Floats are decimal, so `0.1 + 0.2` is `0.3`. A whole-number float keeps a trailing point (`6378137.`), including after `c f`. Notation follows the precision, as in Calc's normal format: a float is written in full while its order runs from -2 through one less than the precision, and in scientific form outside that. At precision 12, `4.2e10` is `42000000000.`, `1e11` is `100000000000.`, `1e12` and `1e20` stay scientific, `0.01` stays fixed, and `0.001` is `1e-3`. An exact integer result, such as `sin(90)`, has no point.
- A negative square root, and non-real powers, become rectangular complex `(re, im)`. A zero imaginary part collapses back to a real.
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
k c       choose, M on top: N! / (M! (N-M)!)
H k c     permutations: N! / (N-M)!
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
c f c F   to float, to fraction
s s       store the top, leaving it on the stack
s t       pop the top into a variable
s r       recall a variable onto the stack
s u       unstore a variable
s l       list stored variables; letter or digit recalls, Tab pages
s 0-9     store into q0–q9; t 0-9 pops into one; r 0-9 recalls one
u         undo           C-r redo
          undo restores the stack and the variables
q ? Esc   quit, shortcut list, cancel entry or prefixes
? ?       command pages. Type to search. Esc closes. ? returns to the list.
```

`L` follows Calc: it flips the inverse flag and then uses the exp/ln family, so `L` is ln and `I L` is exp.

## Tests worth keeping green

Tutorial shapes live in `src/app.rs`. Parser cases are in `src/parse.rs`. The screen test in `src/ui.rs` uses ratatui's `TestBackend` (the top value sits above the `.`).

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
- `K` then `0` = `299792458` (speed of light)
- `ec -e '2 20 ^ d g'` = `1,048,576`
- last `K` page, `8` = `5.9722e24` (Earth mass), `A` = `7.346e22` (Moon mass), and `F` = `149597870700` (AU)

Transcendentals are checked by known prefixes (pi, e, ln 2), not by matching Emacs Calc digit for digit. Exact values can be checked with the Emacs Calc command above.

## Not in this version

Algebraic entry (`'`), formulas, vectors, matrices, units, dates, HMS, the trail, numeric prefixes (`C-u`), polar display, infinities.

## Status

Working RPN calculator. `cargo test` is green (float, value, parser, keys, and a headless screen test). `cargo build` produces the ratatui UI. That UI was checked with `TestBackend`, not by typing in a live terminal.

The screen keeps the terminal's own background. A dark terminal uses the GrokNight foregrounds; a light one uses dark text. Startup asks for the background color and treats no answer as dark. The headless screen test still paints the GrokNight canvas (`#141414`). The live screen has a rounded `❯` prompt with the mode on its bottom border, and a shortcut bar (`ret:push`, bold key, muted label). Content sits two columns in from each edge. Under the prompt is a blank row, then the shortcuts, then a blank row. The typed text sits one column inside the prompt border. `?` opens a Shortcuts box centered on the input's width, in the space above the prompt: the same number of rows sits above the frame as between the frame and the input. Gray `┌┐` frame, bold title on the top edge, `[x]` at the top right, and `esc close` centered inside the bottom. Bindings sit in four columns (fewer when the terminal is narrow), bold key and muted label. `?` again opens Commands, in the same frame. The top row is a search field. Several matches show a short list and the selected page under a rule. One match shows only that page. Up and Down, or Tab and Shift-Tab, move and wrap. Backspace edits the search. Esc closes. `?` returns to the shortcut list. `q` is a search letter there.

Entry and stack details that are easy to get wrong:

- Keep `num-bigint` at 0.4.8 (`num-rational` 0.4 needs that major).
- `DecFloat::from_decimal(mantissa, exponent, prec)` builds a typed decimal. The stored precision is the number of significant digits typed, capped at the current precision.
- A typed exponent, decimal or radix, outside ±1000000 is overflow. `1e1000000` and `1e-1000000` still parse. An exponent that does not fit in a 64-bit integer is a bad exponent.
- `q` quits when nothing is being typed. While radix or word size is being typed, `q` cancels that prompt and leaves the stack. A variable name can contain `q`. `q` does not quit while `K`, `s l`, or the command reference is open. `p` is not a prompt: it pops an integer and sets the precision.
- Do not polish add, sub, mul, or div. `(1/3)*3` can display as `0.999…`, and `sqrt(2)^2` as `1.999…`.
- `L` flips inverse in `app.rs`, then calls `Engine::exp_family`. `E` does not flip. `push` and duplicate do not clear `I` or `H`.
- `n` while typing toggles a leading `_`. `+` and `-` while typing are exponent signs, or digits inside an unclosed `(…)`. Otherwise they add and subtract. Space inside an unparsed `(…` inserts a space. `:` while typing is a fraction; on an empty entry it is fraction division.
- `ec -e` feeds words through the same keys. Whitespace, including newlines, is a separator, and a finished number is pushed at the end of each word. `2 20 ^ d g` prints `1,048,576`. Both `d g` and `dg` group. A space on the screen still pushes or duplicates. An unclosed `(…` stays open across a space, so `(3, 4)` is one value. `12 p` sets the precision and pops 12. A prompt finishes when its answer is complete (`d r 16`, `b w 64`, `s s name`). The command prints the top value. An error goes to stderr and the process exits 1. `q` and `?` stay on the screen.
- Stack index 0 is the oldest value. The top of the stack is the last element, drawn at the bottom, with a bold `.` on the last row. The screen does not print level numbers. Values and the `.` start on the same column as the character after `❯ `. A value wider than that row is split across lines: every line but the last ends with `\`, and each later line is indented one column. When that split value is taller than the rows above `.`, the leading lines stay and still end in `\`. Older values that do not fit are left off the top.
- `F` is floor toward minus infinity and returns an exact integer. An integer is unchanged. A fraction or a decimal float is exact (`-4:3` is `-2`). A complex number is an error. `f` is still the function prefix.
- `k i` is the modular inverse of `a` modulo `m`, with `m` on top. The result is the integer `b` with `0 <= b < m` and `a*b ≡ 1 (mod m)`. `3 7 k i` is `5`. Both values are integers, and an exact integer float is accepted. A non-positive modulus says `modulus must be positive`. If the gcd is not 1 the stack stays and the message is `no modular inverse`. Emacs Calc gets the same `b` as the reciprocal of a modulo form.
- `K` opens the constants, sixteen to a page, eight down each column. `0`–`9` and `a`–`f` insert that slot and close. Tab and Shift-Tab turn the page and wrap around. The last page has the remaining physics constants, then Earth mass (`Me`), Earth equatorial radius (`Re`, `6378137` m), Moon mass (`Mm`, `7.346e22` kg), Moon equatorial radius (`Rm`, `1738100` m), the mean Earth-Moon distance (`dm`, `384400000` m), Sun mass (`Ms`), Sun radius (`Rs`, `695700` km written in metres), and the astronomical unit (`AU`, `149597870700` m). Every slot is filled. A missing slot would say "no constant" and stay open. Esc closes. `q` does not quit while it is open. The physics numbers are the HP 35s CODATA 1998 values. The planetary numbers are the NASA Earth, Sun, and Moon fact sheets, in kilograms and metres. An integer literal stays an integer (`299792458`, `101325`, `6378137`, `149597870700`, `1738100`). The unit is only a label in the menu and the message (`c  m/s`, `Me  kg`). `e` is not listed; `H P` pushes it. `K` is still a digit while a radix literal above 20 is being typed.
- `atan2` of a value on an axis is an exact right angle (`0`, `±90`, `180` in degrees), including `atan2(0, 0) = 0` and `atan2(0, negative) = 180`.
- Undo stores the stack and the variables, cap 100. Mode changes are not undoable. A new stack change clears redo. `s s` stores without popping, `s t` pops into the variable, `s r` recalls, and `s u` unstores. A digit selects `q0`–`q9` (`s 3`, `t 3`, `r 3`). A name is finished with RET. `s l` lists them, five per column. Tab and Shift-Tab page and wrap around. A letter recalls that one-letter name, and a digit recalls `q0`–`q9`. Longer names are visible there and still recalled with `s r`.
- Radix is display and entry only. A number typed without `#` is decimal. `d 2`, `d 8`, `d 6`, `d 0`, and `d r` set the radix (2–36). `#` uses the current radix; `16#FF` forces one. Letters are digits only after `#` when the radix is above 10. For radix 14 or less, `e` is a power of that radix (`8#1.2e3`). From 15 up, `e` is a digit and the power is `16#1.2*16^3`. `d z` pads integers to the word size. `b w` sets that width (default 32, negative allowed, display uses the absolute value). `O` before a radix command turns on two's complement for radices 2, 8, and 16 (`16##FD` is an 8-bit −3). Setting the radix without `O` turns it off. `radix##digits` reads a two's-complement word. `d g` groups integer digits with commas: 4 in binary and hex, 3 otherwise. The fractional part and the exponent stay plain, so `1234567.123454` displays as `1,234,567.123454`. Grouping is display only.
