//! One binding for each key. Help, the command pages, and the mode line read it. `? ?` opens the list. Typing filters it.

pub(crate) struct Binding {
    pub keys: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    pub body: &'static str,
    /// Place on the shortcut list, starting at 0. Absent when the list skips it.
    help: Option<u16>,
    /// Order on the bottom bar. Absent when the bar skips it.
    bar: Option<u8>,
    /// Mode line while this prefix is armed, such as `f-`.
    mode: Option<&'static str>,
    /// `? ?` lists this page. Prefix rows and the quit and help keys stay out.
    catalog: bool,
}

/// Widest key label. `S-Tab` is five characters.
pub(crate) const KEY_WIDTH: usize = 5;
/// Most rows down one column of the shortcut list. Short terminals use fewer.
pub(crate) const HELP_ROWS: usize = 15;

const BINDINGS: &[Binding] = &[
    Binding {
        keys: "ret",
        name: "push",
        summary: "push or duplicate",
        body: "RET or space pushes the number being typed. An empty entry duplicates the top of the stack, and leaves a pending I or H in place.",
        help: Some(0),
        bar: Some(0),
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "del",
        name: "pop",
        summary: "erase or pop",
        body: "DEL erases the last typed character. With an empty entry it pops the top of the stack.",
        help: Some(1),
        bar: Some(2),
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "tab",
        name: "swap",
        summary: "exchange the top two",
        body: "TAB exchanges the top two values.",
        help: Some(2),
        bar: Some(1),
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "S-Tab",
        name: "roll",
        summary: "rotate the top three",
        body: "S-Tab brings the third value up to the top. The old top two each drop one level. Fewer than three values is an error.",
        help: Some(3),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "u",
        name: "undo",
        summary: "restore stack and variables",
        body: "u restores the stack and the stored variables. Mode changes stay as they are. The history keeps 100 steps.",
        help: Some(4),
        bar: Some(3),
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "C-r",
        name: "redo",
        summary: "undo an undo",
        body: "C-r walks back toward the stack you undid. A new calculation drops the redo history.",
        help: Some(5),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "n",
        name: "negate",
        summary: "change sign",
        body: "n changes the sign of the top value. While a number is being typed, n toggles a leading _.",
        help: Some(13),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "&",
        name: "recip",
        summary: "1/x",
        body: "& replaces x with 1/x.",
        help: Some(14),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "Q",
        name: "sqrt",
        summary: "square root",
        body: "Q takes the square root. A perfect square of an integer stays an integer. A negative becomes a complex value (0, imag).",
        help: Some(15),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "I Q",
        name: "square",
        summary: "x^2",
        body: "I Q replaces x with x^2. I is the one-shot inverse prefix.",
        help: Some(16),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "I ^",
        name: "root",
        summary: "nth root",
        body: "I ^ takes the nth root of the buried value. The degree is on top. 125 3 I ^ is 5.",
        help: Some(17),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "A",
        name: "abs",
        summary: "absolute value",
        body: "A is the absolute value. On a complex number it is the magnitude, so (3, 4) A is 5.",
        help: Some(18),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "f A",
        name: "abs^2",
        summary: "square of the magnitude",
        body: "f A is the square of the absolute value. A complex number becomes re^2 + im^2.",
        help: Some(19),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "I",
        name: "inverse",
        summary: "one-shot inverse prefix",
        body: "I arms the inverse for the next operation. I S is arcsin, I Q squares, and I ^ takes an nth root. Pushing or duplicating leaves I set.",
        help: Some(11),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "H",
        name: "hyp",
        summary: "one-shot hyperbolic prefix",
        body: "H arms the hyperbolic form of the next operation. H S is sinh, H L is log10, and H P pushes e. It is used once.",
        help: Some(12),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "s s",
        name: "store",
        summary: "copy the top into a variable",
        body: "s s copies the top into a variable and leaves it on the stack. A digit stores q0 through q9, as in s 3. A longer name ends with RET.",
        help: Some(6),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "s t",
        name: "into",
        summary: "pop the top into a variable",
        body: "s t pops the top into a variable. A digit pops into q0 through q9. t 3 does the same.",
        help: Some(7),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "s r",
        name: "recall",
        summary: "push a variable",
        body: "s r pushes a stored variable. A digit recalls q0 through q9, and r 3 does the same. From s l, a letter recalls that one-letter name.",
        help: Some(8),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "s u",
        name: "unstore",
        summary: "forget a variable",
        body: "s u forgets a variable. A digit forgets that q register. A longer name ends with RET.",
        help: Some(9),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "s l",
        name: "list",
        summary: "browse stored variables",
        body: "s l lists the stored variables, five down each column. Tab and S-Tab turn the page. A letter or a digit recalls that name. Esc closes. q is the variable q while this list is open.",
        help: Some(10),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "+",
        name: "add",
        summary: "y + x",
        body: "+ adds the top two values, x on top. While typing, + is an exponent sign, or a character inside an unclosed (…).",
        help: Some(23),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "-",
        name: "sub",
        summary: "y - x",
        body: "- subtracts the top from the value under it. While typing, - is an exponent sign, or a character inside an unclosed (…). A leading minus in an entry is _.",
        help: Some(24),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "*",
        name: "mul",
        summary: "y * x",
        body: "* multiplies the top two values.",
        help: Some(25),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "/",
        name: "div",
        summary: "y / x",
        body: "/ divides the buried value by the top. Integers that divide evenly stay integers. Otherwise the result is a float at the current precision. m f makes it a fraction instead.",
        help: Some(26),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "^",
        name: "power",
        summary: "y^x",
        body: "^ raises the buried value to the top. A non-real result becomes complex. I ^ takes an nth root.",
        help: Some(27),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "\\",
        name: "idiv",
        summary: "integer quotient",
        body: "\\ divides and keeps the integer quotient, toward minus infinity. 10 \\ 3 is 3, and -5 \\ 3 is -2. An integer or a fraction divides exactly; a float rounds at the current precision.",
        help: Some(28),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "F",
        name: "floor",
        summary: "integer toward minus infinity",
        body: "F rounds toward minus infinity and returns an exact integer. 45.3 F is 45, and -45.3 F is -46. A float such as 4.02e26 expands to every digit. A complex value is an error.",
        help: Some(29),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "%",
        name: "mod",
        summary: "remainder",
        body: "% is the remainder from integer division toward minus infinity. 10 % 3 is 1, and -5 % 3 is 1. An integer or a fraction is exact; a float rounds at the current precision.",
        help: Some(30),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: ":",
        name: "fraction",
        summary: "exact division",
        body: "On an empty entry, : divides the top two as a fraction, so 3 4 : is 3:4. While typing, : is the fraction bar.",
        help: Some(31),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "!",
        name: "fact",
        summary: "factorial",
        body: "! is the factorial. A non-negative integer is exact, through 100000. Any other real uses the gamma function. A negative integer is an error.",
        help: Some(32),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k g",
        name: "gcd",
        summary: "greatest common divisor",
        body: "k g is the greatest common divisor of the top two values. For fractions it is the gcd of the numerators over the lcm of the denominators.",
        help: Some(37),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k l",
        name: "lcm",
        summary: "least common multiple",
        body: "k l is the least common multiple of the top two values. Fractions are accepted.",
        help: Some(38),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k E",
        name: "egcd",
        summary: "extended gcd",
        body: "k E takes y on top and x under it. It pushes a, then b, then g, with g on top, so a*x + b*y = g. 2 5 k E leaves -2, then 1, then 1, and (-2)*2 + 1*5 = 1.",
        help: Some(39),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k i",
        name: "invmod",
        summary: "modular inverse",
        body: "k i replaces a and m with b. m is on top, 0 <= b < m, and a*b is 1 modulo m. 3 7 k i is 5. The modulus is a positive integer. If the gcd is not 1, the message is no modular inverse.",
        help: Some(40),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k p",
        name: "prime",
        summary: "primality test",
        body: "k p reports whether the integer on top is prime, and leaves that number on the stack. The message is prime, not prime, or probably prime.",
        help: Some(41),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k f",
        name: "factor",
        summary: "prime factors",
        body: "k f replaces an integer with its prime factors, smallest first. 2023 k f leaves 7, then 17, then 17, and * * multiplies them back to 2023. A negative number leads with -1. 0 and 1 stay as they are. Trial division uses Calc's prime table through 5003. A cofactor that cannot be proven prime stays in one piece, and the message is may not be fully factored.",
        help: Some(42),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k c",
        name: "choose",
        summary: "binomial coefficient",
        body: "k c is N! / (M! (N-M)!), with M on top. 5 2 k c is 10. Integers stay exact.",
        help: Some(33),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "Hk c",
        name: "perm",
        summary: "permutations",
        body: "H k c is N! / (N-M)!, with M on top. 5 2 H k c is 20.",
        help: Some(34),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k s",
        name: "stirling1",
        summary: "Stirling number, first kind",
        body: "k s is the signed Stirling number of the first kind s(n, m), with m on top. 4 2 k s is 11. n and m are nonnegative integers, and m > n is 0.",
        help: Some(35),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "Hk s",
        name: "stirling2",
        summary: "Stirling number, second kind",
        body: "H k s is the Stirling number of the second kind S(n, m), with m on top. 4 2 H k s is 7. n and m are nonnegative integers, and m > n is 0.",
        help: Some(36),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "P",
        name: "pi",
        summary: "push pi",
        body: "P pushes pi at the current precision. H P pushes e.",
        help: Some(43),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "J",
        name: "conj",
        summary: "complex conjugate",
        body: "J replaces a complex value (re, im) with (re, -im). A real value stays as it is.",
        help: Some(21),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "S",
        name: "sin",
        summary: "sine",
        body: "S is the sine. Angles start in degrees. I S is arcsin, H S is sinh, and H I S is asinh.",
        help: Some(46),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "C",
        name: "cos",
        summary: "cosine",
        body: "C is the cosine. I C is arccos, H C is cosh, and H I C is acosh.",
        help: Some(47),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "T",
        name: "tan",
        summary: "tangent",
        body: "T is the tangent. I T is arctan, H T is tanh, and H I T is atanh. f T is atan2.",
        help: Some(48),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "L",
        name: "ln",
        summary: "natural log",
        body: "L is the natural log. The key flips the inverse flag first, so I L is exp. H L is log10, and H I L is 10^x.",
        help: Some(51),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "E",
        name: "exp",
        summary: "e^x",
        body: "E is the exponential. I E is ln. H E is 10^x. Unlike L, E does not flip the inverse flag.",
        help: Some(52),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "B",
        name: "log",
        summary: "logarithm, base on top",
        body: "B is the logarithm of the buried value in the base on top. 8 2 B is 3. An integer base greater than 1 stays exact when the value is a ratio of integer powers of that base, so 1:8 2 B is -3.",
        help: Some(53),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "I B",
        name: "antilog",
        summary: "base^exponent",
        body: "I B raises the base on top to the buried power. 3 2 I B is 8.",
        help: Some(54),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "H L",
        name: "log10",
        summary: "common log",
        body: "H L is the base-10 logarithm. A power of ten, or its reciprocal, stays an integer, so 1000 H L is 3 and 1:1000 H L is -3.",
        help: Some(55),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "H E",
        name: "10^x",
        summary: "power of ten",
        body: "H E replaces x with 10^x.",
        help: Some(56),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "H P",
        name: "e",
        summary: "push e",
        body: "H P pushes e at the current precision. P pushes pi. e is not in the K catalog.",
        help: Some(44),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "f T",
        name: "atan2",
        summary: "angle of (x, y)",
        body: "f T is atan2(y, x) with x on top, in the current angle mode. 1 0 f T is 90 degrees. A point on an axis is an exact right angle, and 0 0 f T is 0.",
        help: Some(49),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "f h",
        name: "hypot",
        summary: "hypotenuse",
        body: "f h is the square root of the sum of squares of the top two values. 3 4 f h is 5.",
        help: Some(50),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "f Q",
        name: "isqrt",
        summary: "integer square root",
        body: "f Q is the greatest integer whose square is at most the value. 10 f Q is 3. An integer, or a float that is already a whole number, is exact. A negative number is an error.",
        help: Some(57),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "f E",
        name: "expm1",
        summary: "exp(x) - 1",
        body: "f E is exp(x) - 1. A small x keeps digits that exp(x) then subtracting 1 would lose.",
        help: Some(58),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "f L",
        name: "lnp1",
        summary: "ln(1 + x)",
        body: "f L is ln(1 + x). A small x keeps digits that adding 1 then taking ln would lose.",
        help: Some(59),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "f I",
        name: "ilog",
        summary: "integer logarithm",
        body: "f I is the integer log of the buried value, with the base on top. 1000 10 f I is 3. A result that is not already whole is floored.",
        help: Some(60),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "f s",
        name: "sign",
        summary: "sign of x",
        body: "f s is -1, 0, or 1 for a real value. A complex value becomes that value divided by its magnitude.",
        help: Some(20),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "G",
        name: "arg",
        summary: "complex argument",
        body: "G is the argument of a complex value, in the current angle mode. The argument of zero is an error.",
        help: Some(22),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "K",
        name: "const",
        summary: "physics and astronomy constants",
        body: "K opens the constant catalog, sixteen to a page. 0-9 and a-f insert that slot. Tab and S-Tab turn the page. The unit is only a label. Esc closes, and q does not quit while the catalog is open.",
        help: Some(45),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "p",
        name: "prec",
        summary: "working precision",
        body: "p sets the precision in significant digits from the integer on top, then pops it. The default is 12, and the value must be from 3 to 1000. A float is written in full while its order runs from -2 through one less than that precision, and in scientific form past either end. Undo puts the number back and leaves the precision.",
        help: Some(61),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "m d",
        name: "deg",
        summary: "degrees",
        body: "m d measures angles in degrees. Real trig and both parts of complex trig use this mode. A degree angle is reduced modulo 360.",
        help: Some(62),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "m r",
        name: "rad",
        summary: "radians",
        body: "m r measures angles in radians.",
        help: Some(63),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "m f",
        name: "fractions",
        summary: "fraction mode",
        body: "m f toggles fraction mode. While it is on, / of two numbers that do not divide evenly stays a fraction. : divides as a fraction either way.",
        help: Some(64),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "c f",
        name: "float",
        summary: "convert to a float",
        body: "c f turns the top value into a float at the current precision. A whole number keeps a trailing point, so 6378137 becomes 6378137. At precision 12, 4.2e10 is written 42000000000. and 1e20 stays 1e20.",
        help: Some(65),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "c F",
        name: "fraction",
        summary: "convert to a fraction",
        body: "c F turns the top value into a fraction. A float that is already a whole number becomes that integer. F floors a float to an integer instead.",
        help: Some(66),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "d 2",
        name: "bin",
        summary: "binary display",
        body: "d 2 shows integers in binary. A typed number with no # is still decimal. # uses binary, and 2#11111111 is 255. O d 2 shows two's complement.",
        help: Some(67),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "d 8",
        name: "oct",
        summary: "octal display",
        body: "d 8 shows integers in octal. # uses octal. O d 8 shows two's complement.",
        help: Some(68),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "d 6",
        name: "hex",
        summary: "hexadecimal display",
        body: "d 6 shows integers in hexadecimal. 16#FF is 255 in any mode. From radix 15 up, e is a digit, so a power is written 16#1.2*16^3.",
        help: Some(69),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "d 0",
        name: "dec",
        summary: "decimal display",
        body: "d 0 returns the display to decimal and turns two's complement off.",
        help: Some(70),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "d r",
        name: "radix",
        summary: "display radix 2-36",
        body: "d r asks for a radix from 2 to 36, then digits, then RET. Through radix 14, e is a power of that radix, as in 8#1.2e3. q cancels the prompt.",
        help: Some(71),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "d z",
        name: "zeros",
        summary: "pad integers to the word size",
        body: "d z pads integers with leading zeros out to the word size. The stored value stays the same. Fractions and floats are left as they are.",
        help: Some(72),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "d g",
        name: "group",
        summary: "commas in integers",
        body: "d g groups integer digits with commas: 4 digits in binary and hex, 3 otherwise. The fractional part and the exponent stay plain. Grouping is display only.",
        help: Some(73),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "b w",
        name: "word",
        summary: "word size",
        body: "b w sets the width used by leading zeros and two's complement. The default is 32. A negative size uses that same width. Finish with RET. q cancels the prompt.",
        help: Some(74),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "O d",
        name: "twos",
        summary: "two's complement display",
        body: "O before a radix key shows binary, octal, and hex integers in two's complement. An 8-bit -3 is 16##FD. Choosing a radix without O turns that display off.",
        help: Some(75),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "Esc",
        name: "cancel",
        summary: "drop entry, prefix, or dialog",
        body: "Esc cancels the number being typed, a prefix, or an open dialog. The stack stays. q quits when nothing is being typed.",
        help: Some(76),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "?",
        name: "help",
        summary: "",
        body: "",
        help: None,
        bar: Some(4),
        mode: None,
        catalog: false,
    },
    Binding {
        keys: "q",
        name: "quit",
        summary: "",
        body: "",
        help: None,
        bar: Some(5),
        mode: None,
        catalog: false,
    },
    Binding {
        keys: "f",
        name: "f-",
        summary: "",
        body: "",
        help: None,
        bar: None,
        mode: Some("f-"),
        catalog: false,
    },
    Binding {
        keys: "m",
        name: "m-",
        summary: "",
        body: "",
        help: None,
        bar: None,
        mode: Some("m-"),
        catalog: false,
    },
    Binding {
        keys: "c",
        name: "c-",
        summary: "",
        body: "",
        help: None,
        bar: None,
        mode: Some("c-"),
        catalog: false,
    },
    Binding {
        keys: "d",
        name: "d-",
        summary: "",
        body: "",
        help: None,
        bar: None,
        mode: Some("d-"),
        catalog: false,
    },
    Binding {
        keys: "b",
        name: "b-",
        summary: "",
        body: "",
        help: None,
        bar: None,
        mode: Some("b-"),
        catalog: false,
    },
    Binding {
        keys: "k",
        name: "k-",
        summary: "",
        body: "",
        help: None,
        bar: None,
        mode: Some("k-"),
        catalog: false,
    },
    Binding {
        keys: "s",
        name: "s-",
        summary: "",
        body: "",
        help: None,
        bar: None,
        mode: Some("s-"),
        catalog: false,
    },
    Binding {
        keys: "t",
        name: "t-",
        summary: "",
        body: "",
        help: None,
        bar: None,
        mode: Some("t-"),
        catalog: false,
    },
    Binding {
        keys: "r",
        name: "r-",
        summary: "",
        body: "",
        help: None,
        bar: None,
        mode: Some("r-"),
        catalog: false,
    },
    Binding {
        keys: "O",
        name: "O-",
        summary: "",
        body: "",
        help: None,
        bar: None,
        mode: Some("O-"),
        catalog: false,
    },
    Binding {
        keys: "Od",
        name: "Od-",
        summary: "",
        body: "",
        help: None,
        bar: None,
        mode: Some("Od-"),
        catalog: false,
    },
];

#[cfg(test)]
pub(crate) fn bindings() -> &'static [Binding] {
    BINDINGS
}

/// Shortcut list in display order. The box fills down a column, then the next.
pub(crate) fn help_entries() -> Vec<(&'static str, &'static str)> {
    let mut placed: Vec<(u16, &str, &str)> = BINDINGS
        .iter()
        .filter_map(|binding| binding.help.map(|ord| (ord, binding.keys, binding.name)))
        .collect();
    placed.sort_by_key(|(ord, _, _)| *ord);
    placed
        .into_iter()
        .map(|(_, keys, name)| (keys, name))
        .collect()
}

/// One page of the shortcut list. Columns fill downward, `rows` each.
pub(crate) fn help_columns(
    page: usize,
    cols: usize,
    rows: usize,
) -> Vec<Vec<(&'static str, &'static str)>> {
    let cols = cols.max(1);
    let rows = rows.max(1);
    let entries = help_entries();
    let page_size = rows * cols;
    let start = page.saturating_mul(page_size).min(entries.len());
    let end = (start + page_size).min(entries.len());
    let mut columns = vec![Vec::new(); cols];
    for (offset, entry) in entries[start..end].iter().enumerate() {
        columns[offset / rows].push(*entry);
    }
    columns
}

/// Bottom bar, left to right.
pub(crate) fn shortcuts() -> Vec<(&'static str, &'static str)> {
    let mut items: Vec<(u8, &str, &str)> = BINDINGS
        .iter()
        .filter_map(|binding| binding.bar.map(|ord| (ord, binding.keys, binding.name)))
        .collect();
    items.sort_by_key(|(ord, _, _)| *ord);
    items
        .into_iter()
        .map(|(_, keys, name)| (keys, name))
        .collect()
}

pub(crate) fn keys_named(name: &str) -> &'static str {
    BINDINGS
        .iter()
        .find(|binding| binding.name == name)
        .map(|binding| binding.keys)
        .unwrap_or_else(|| panic!("no binding named {name}"))
}

pub(crate) fn mode_label(keys: &str) -> Option<&'static str> {
    BINDINGS
        .iter()
        .find(|binding| binding.keys == keys)
        .and_then(|binding| binding.mode)
}

/// Matching pages, best first. An empty query keeps catalog order.
pub(crate) fn search(query: &str) -> Vec<&'static Binding> {
    let mut ranked: Vec<(u8, usize, &'static Binding)> = BINDINGS
        .iter()
        .enumerate()
        .filter(|(_, binding)| binding.catalog)
        .filter_map(|(index, binding)| rank(binding, query).map(|score| (score, index, binding)))
        .collect();
    ranked.sort_by_key(|(score, index, _)| (*score, *index));
    ranked.into_iter().map(|(_, _, binding)| binding).collect()
}

fn rank(entry: &Binding, query: &str) -> Option<u8> {
    let q = query.trim();
    if q.is_empty() {
        return Some(9);
    }
    let q = q.to_ascii_lowercase();
    let name = entry.name.to_ascii_lowercase();
    let keys = entry.keys.to_ascii_lowercase();
    let keys_tight: String = keys.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let q_tight: String = q.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    if name == q {
        return Some(0);
    }
    if name.starts_with(&q) {
        return Some(1);
    }
    if keys == q || keys_tight == q_tight {
        return Some(2);
    }
    if name.contains(&q) {
        return Some(3);
    }
    if keys.contains(&q) || keys_tight.contains(&q_tight) {
        return Some(4);
    }
    if entry.summary.to_ascii_lowercase().contains(&q) {
        return Some(5);
    }
    if entry.body.to_ascii_lowercase().contains(&q) {
        return Some(6);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_order_groups_related_keys() {
        let entries = help_entries();
        let ords: Vec<u16> = BINDINGS.iter().filter_map(|binding| binding.help).collect();
        let mut sorted = ords.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ords.len());
        assert_eq!(sorted.first().copied(), Some(0));
        assert_eq!(sorted.last().copied(), Some(ords.len() as u16 - 1));

        let pos = |keys: &str| {
            entries
                .iter()
                .position(|(entry_keys, _)| *entry_keys == keys)
                .unwrap()
        };
        assert_eq!(pos("C-r"), pos("u") + 1);
        assert_eq!(pos("H"), pos("I") + 1);
        assert_eq!(pos("f A"), pos("A") + 1);
        assert_eq!(pos("G"), pos("J") + 1);
        assert_eq!(pos("Hk c"), pos("k c") + 1);
        assert_eq!(pos("Hk s"), pos("k s") + 1);
        assert_eq!(pos("H P"), pos("P") + 1);
        assert_eq!(pos("K"), pos("H P") + 1);
        assert_eq!(pos("C"), pos("S") + 1);
        assert_eq!(pos("T"), pos("C") + 1);

        let page = help_columns(0, 3, HELP_ROWS);
        assert_eq!(page.len(), 3);
        assert!(page.iter().all(|column| column.len() == HELP_ROWS));
        assert_eq!(page[0][0].0, "ret");
        assert_eq!(page[1][0].0, "Q");
        assert_eq!(page[2][0].0, "%");

        let rest = help_columns(1, 3, HELP_ROWS);
        let shown: usize = rest.iter().map(|column| column.len()).sum();
        assert_eq!(shown, entries.len() - HELP_ROWS * 3);
        assert_eq!(rest[0][0].0, "K");
        assert!(rest.last().unwrap().len() < HELP_ROWS);

        let short = help_columns(0, 3, 13);
        assert_eq!(short.iter().map(|column| column.len()).sum::<usize>(), 39);
        assert_eq!(short[0][0].0, "ret");
        assert_eq!(short[1][0].0, "n");
        assert_eq!(short[2][0].0, "/");
        assert_eq!(help_columns(1, 3, 13)[0][0].0, "k E");
    }

    #[test]
    fn keys_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for entry in BINDINGS {
            assert!(seen.insert(entry.keys), "duplicate key {}", entry.keys);
        }
    }

    #[test]
    fn key_labels_fit_their_column() {
        for binding in BINDINGS {
            if binding.help.is_some() || binding.bar.is_some() || binding.catalog {
                assert!(
                    binding.keys.chars().count() <= KEY_WIDTH,
                    "{}",
                    binding.keys
                );
            }
        }
    }

    #[test]
    fn shift_tab_is_labeled_s_tab() {
        assert_eq!(keys_named("roll"), "S-Tab");
        assert_eq!(keys_named("swap"), "tab");
    }

    #[test]
    fn isqrt_is_the_first_hit() {
        let hits = search("isqrt");
        assert_eq!(hits[0].keys, "f Q");
        assert_eq!(hits[0].name, "isqrt");
    }

    #[test]
    fn stirling_bindings_are_searchable() {
        let hits = search("stirling");
        assert_eq!(hits[0].name, "stirling1");
        assert!(hits.iter().any(|entry| entry.name == "stirling2"));
    }

    #[test]
    fn exact_name_beats_a_longer_name() {
        let hits = search("sqrt");
        assert_eq!(hits[0].name, "sqrt");
        assert!(hits.iter().any(|entry| entry.name == "isqrt"));
    }

    #[test]
    fn search_ignores_case_and_spaces() {
        assert_eq!(search("ISQRT")[0].name, "isqrt");
        assert_eq!(search("fq")[0].name, "isqrt");
        assert!(search("zzz").is_empty());
    }

    #[test]
    fn an_empty_query_keeps_catalog_order() {
        let hits = search("  ");
        let catalog = BINDINGS.iter().filter(|binding| binding.catalog).count();
        assert_eq!(hits.len(), catalog);
        assert_eq!(hits[0].name, "push");
        assert_eq!(hits[1].name, "pop");
    }
}
