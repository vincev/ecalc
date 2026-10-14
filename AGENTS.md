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
+ - * / ^ \ %                  arithmetic, power, idiv, mod
n & Q A   negate, reciprocal, sqrt, abs
I Q       square        I ^     nth root
S C T     sin cos tan    I … inverse    H … hyperbolic
L E       ln / exp       H L log10      H E 10^x
B         log (base on top)     I B antilog
f T f h f Q f A f s f I f E f L
          atan2, hypot, isqrt, abs^2, sign, ilog, expm1, lnp1
J G       conjugate, argument
P         pi             H P  e
p         set precision (then digits, RET)
m d m r m f                degrees, radians, fraction mode
c f c F   to float, to fraction
u         undo           C-r redo
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
- Undo stores stacks only, cap 100. Mode changes are not undoable. A new stack change clears redo.
