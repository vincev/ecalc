# calc

Terminal RPN calculator in the shape of GNU Emacs Calc. Algebra, calculus,
vectors, and matrices are out of scope (use a CAS for those).

## Run and test

```sh
cargo test
cargo run
```

`cargo test` does not open a terminal. `cargo run` is the full-screen UI.
Quit with `q`.

## Layout

- `src/float.rs` — decimal floats (`BigInt` mantissa, base-10 exponent). Precision is significant digits, default 12. `+ - * /`, sqrt, exp, ln, sin/cos/tan, inverse trig.
- `src/value.rs` — `Int` / `Rat` / `Float` / rectangular complex. Promotion, powers, roots, trig in degrees or radians.
- `src/parse.rs` — entry syntax.
- `src/engine.rs` — stack, undo, modes, operations.
- `src/app.rs` — key mapping. Test this without a tty.
- `src/ui.rs`, `src/main.rs` — ratatui screen.

## Number rules

- Integers stay exact (`num-bigint`). `:` entry and fraction mode use rationals (`num-rational`), shown as `n:d`.
- Default fraction mode is off, matching Calc. `/` of integers that divide evenly stays an integer; otherwise it is a float at the current precision. `m f` turns fraction mode on. `:` outside entry is fraction division.
- Floats are decimal, so `0.1 + 0.2` is `0.3`.
- A negative square root, and non-real powers, become rectangular complex `(re, im)`. A zero imaginary part collapses back to a real.
- Angle mode defaults to degrees. It applies to real trig and to both parts of complex trig, same idea as Calc. `P` pushes pi. `H P` pushes e.
- Inverse (`I`) and hyperbolic (`H`) are one-shot prefixes.

## Keys

```
digits . e E _ - + : ( ) ,     entry
RET SPC   push, or duplicate the top if the entry is empty
DEL       backspace, or pop
TAB       swap the top two
S-Tab     roll the top three upward (level 3 becomes the top)
+ - * / ^ \ %                  arithmetic, power, idiv, mod
n & Q A   negate, reciprocal, sqrt, abs
I Q       square        I ^     nth root
S C T     sin cos tan    I … inverse    H … hyperbolic
L E       ln / exp       H L log10      H E 10^x
B         log (base on top)     I B antilog
f T f h f Q f A f s f I f E f L
          atan2, hypot, isqrt, abs^2, sign, ilog, expm1, lnp1
k g k l   gcd, lcm of the top two (integers or fractions)
k E       extended gcd: g on top, then b, then a, with g = a*x + b*y
k p       prime test; the number stays on the stack
k c       choose, M on top: N! / (M! (N-M)!)
H k c     permutations: N! / (N-M)!
!         factorial (exact for integers, gamma otherwise)
J G       conjugate, argument
P         pi             H P  e
p         set precision (then digits, RET)
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
q ? Esc   quit, help, cancel entry or prefixes
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

Transcendentals are checked by known prefixes (pi, e, ln 2), not by matching Emacs Calc digit for digit.

## Not in this version

Algebraic entry (`'`), formulas, vectors, matrices, units, dates, HMS, the trail, numeric prefixes (`C-u`), polar display, infinities.

## Status

Working RPN calculator. `cargo test` is green (float, value, parser, keys, and a headless screen test). `cargo build` produces the ratatui UI. That UI was checked with `TestBackend`, not by typing in a live terminal.

The screen uses the GrokNight palette: dark canvas, a rounded `❯` prompt with the mode on its bottom border, and a shortcut bar (`ret:push`, bold key, muted label). Content sits two columns in from each edge. Under the prompt is a blank row, then the shortcuts, then a blank row. The typed text sits one column inside the prompt border. `?` opens a centered Shortcuts box: gray `┌┐` frame, bold title on the top edge, `[x]` at the top right, and `esc close` centered inside the bottom. Bindings sit in four columns (fewer when the terminal is narrow), bold key and muted label.

Entry and stack details that are easy to get wrong:

- Keep `num-bigint` at 0.4.8 (`num-rational` 0.4 needs that major).
- `DecFloat::from_decimal(mantissa, exponent, prec)` builds a typed decimal. The stored precision is the number of significant digits typed, capped at the current precision.
- Do not polish add, sub, mul, or div. `(1/3)*3` can display as `0.999…`, and `sqrt(2)^2` as `1.999…`.
- `L` flips inverse in `app.rs`, then calls `Engine::exp_family`. `E` does not flip. `push` and duplicate do not clear `I` or `H`.
- `n` while typing toggles a leading `_`. `+` and `-` while typing are exponent signs, or digits inside an unclosed `(…)`. Otherwise they add and subtract. Space inside an unparsed `(…` inserts a space. `:` while typing is a fraction; on an empty entry it is fraction division.
- Stack index 0 is the oldest value. The top of the stack is the last element, drawn at the bottom, with `.` on the last row. The screen does not print level numbers. Values and the `.` start on the same column as the character after `❯ `.
- `atan2` of a value on an axis is an exact right angle (`0`, `±90`, `180` in degrees), including `atan2(0, 0) = 0` and `atan2(0, negative) = 180`.
- Undo stores the stack and the variables, cap 100. Mode changes are not undoable. A new stack change clears redo. `s s` stores without popping, `s t` pops into the variable, `s r` recalls, and `s u` unstores. A digit selects `q0`–`q9` (`s 3`, `t 3`, `r 3`). A name is finished with RET. `s l` lists them, five per column. Tab and Shift-Tab page. A letter recalls that one-letter name, and a digit recalls `q0`–`q9`. Longer names are visible there and still recalled with `s r`.
- Radix is display and entry only. A number typed without `#` is decimal. `d 2`, `d 8`, `d 6`, `d 0`, and `d r` set the radix (2–36). `#` uses the current radix; `16#FF` forces one. Letters are digits only after `#` when the radix is above 10. For radix 14 or less, `e` is a power of that radix (`8#1.2e3`). From 15 up, `e` is a digit and the power is `16#1.2*16^3`. `d z` pads integers to the word size. `b w` sets that width (default 32, negative allowed, display uses the absolute value). `O` before a radix command turns on two's complement for radices 2, 8, and 16 (`16##FD` is an 8-bit −3). Setting the radix without `O` turns it off. `radix##digits` reads a two's-complement word. `d g` groups integer digits with commas: 4 in binary and hex, 3 otherwise. The fractional part and the exponent stay plain, so `1234567.123454` displays as `1,234,567.123454`. Grouping is display only.
