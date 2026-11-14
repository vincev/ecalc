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
        keys: "←→",
        name: "scroll",
        summary: "scroll long values sideways",
        body: "A value wider than the screen stays on one row, as in Emacs Calc: $ in the last column means more follows, and $ in the first means text is hidden to the left. Left and Right, or C-b and C-f, scroll every stack row by a screen. C-a or Home goes to the start, C-e or End to the end. Any other key returns to the start. In the trail the same keys scroll the selected row.",
        help: Some(7),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "[ ]",
        name: "vector",
        summary: "type a vector",
        body: "[ starts a vector and ] ends it: [1, 2, 3]. Commas or spaces separate the elements, as in Calc: [1 2 3] is the same vector. An element can be any number, a complex (1, 2), a modulo form (3M7), or another vector. + and - go element by element, and a number meets every element: [1, 2] 10 + is [11, 12]. * of two vectors is the dot product, and [1, 2, 3] 2 ^ is 14, the vector dotted with itself. Most other keys go element by element: [4, 9] Q is [2, 3], and 3M7 [1, 2, 3] ^ is [3 mod 7, 2 mod 7, 6 mod 7]. A is the length: [3, 4] A is 5. A vector of rows is a matrix: see [[ ]]. A long vector stays on one row and scrolls sideways.",
        help: Some(75),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "v p",
        name: "pack",
        summary: "pack values into a vector",
        body: "v p pops a count n, then packs the n values under it into a vector, the deepest first. 1 2 3 3 v p is [1, 2, 3]. V p works too.",
        help: Some(76),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "v u",
        name: "unpack",
        summary: "vector onto the stack",
        body: "v u replaces the vector on top with its elements, the last one on top. [6, 8, 15] k f v u leaves [2, 3], [2, 2, 2], and [3, 5].",
        help: Some(77),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "v x",
        name: "index",
        summary: "the vector 1 to n",
        body: "v x replaces n with [1, 2, ..., n], as Calc's index. 6 v x is [1, 2, 3, 4, 5, 6]. Up to 10000 elements.",
        help: Some(78),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "V M",
        name: "map",
        summary: "next key element by element",
        body: "V M makes the next operation go element by element, even where a vector means something else: [1, 2] [3, 4] V M * is [3, 8], where * gives the dot product 11. [1, 2, 3] 2 V M ^ is [1, 4, 9], and [1, 2] V M & is [1, 0.5]. A matrix too: [[1, 2], [3, 4]] 2 V M ^ is [[1, 4], [9, 16]], where ^ gives the matrix square. The mode line shows map until the operation runs.",
        help: Some(80),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "V R",
        name: "reduce",
        summary: "fold the next key over",
        body: "V R folds the next two-argument operation over the vector on top: [1, 2, 3, 4] V R + is 10, and 2023 k f V R * is 2023. A matrix folds over all its elements, as in Calc: [[1, 2], [3, 4]] V R + is 10. An empty vector gives 0 for + and -, and 1 for *. The mode line shows reduce until the operation runs.",
        help: Some(81),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k r",
        name: "random",
        summary: "random number below the top",
        body: "k r replaces a limit with a random number, as Calc's random. An integer m gives an integer from 0 to m - 1 (or from m + 1 to 0 when m is negative), each exactly as likely. A float or a fraction x gives a float of [0, x) with every digit of the precision random. 0 gives a normal sample, mean 0 and spread 1. A vector gives one of its elements. The numbers come from a cryptographic generator seeded by the system.",
        help: Some(60),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k a",
        name: "again",
        summary: "another random number",
        body: "k a pushes another random number from the limit k r or k h used last, leaving the stack under it. Before any k r the limit is 1., a float of [0, 1).",
        help: Some(61),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k h",
        name: "shuffle",
        summary: "n picks without repeats",
        body: "k h takes a count n on top and a limit under it, and gives n values of the limit without repeats, in random order, as Calc's shuffle. 10 10 k h is 0 to 9 in a random order, 49 6 k h is six numbers of 0 to 48, and [a vector] 2 k h is two of its elements. With a float limit it draws n different floats.",
        help: Some(62),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "v l",
        name: "length",
        summary: "number of elements",
        body: "v l replaces the vector on top with its number of elements. [1, [2, 3]] v l is 2.",
        help: Some(79),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "[[ ]]",
        name: "matrix",
        summary: "type a matrix",
        body: "A vector of rows of one length is a matrix: [[1, 2], [3, 4]]. * is the matrix product: [[1, 2], [3, 4]] [[5, 6], [7, 8]] * is [[19, 22], [43, 50]]. A flat vector is a column on the right and a row on the left: [[1, 2], [3, 4]] [5, 6] * is [17, 39]. & is the inverse, ^ an integer power, and b A / solves A x = b. V D is the determinant, V T the trace, v t the transpose, v i the identity. + and -, a number with *, and every function go element by element, and V M makes * / ^ & do so too. Elimination is exact and takes up to 100 by 100; past the limit the message is overflow.",
        help: Some(82),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "V D",
        name: "det",
        summary: "determinant",
        body: "V D replaces a square matrix with its determinant. [[1, 2], [3, 4]] V D is -2. Exact entries give an exact result, fractions included: [[1:2, 1:3], [1:4, 1:5]] V D is 1:60. With a float the result is a float, rounded once from the exact value: [[0.5, 0.25], [0.125, 2]] V D is 0.96875. Modulo forms work too: [[2M7, 3M7], [1M7, 4M7]] V D is 5 mod 7.",
        help: Some(86),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "v &",
        name: "inverse",
        summary: "inverse of a matrix",
        body: "v & is the same key as &. A square matrix gives its inverse: [[1, 2], [3, 4]] & is [[-2, 1], [1.5, -0.5]]. As with /, an element that is not whole is a float unless fraction mode is on: m f first gives [[-2, 1], [3:2, -1:2]]. The work is exact, so a matrix is singular matrix only when its determinant is exactly zero, as [[1.5, 2], [3, 4]] is. Modulo forms invert whenever the determinant has an inverse: [[2M7, 3M7], [1M7, 4M7]] & is [[5 mod 7, 5 mod 7], [4 mod 7, 6 mod 7]]. To solve A x = b, divide instead: b A /.",
        help: Some(85),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "v t",
        name: "transpose",
        summary: "transpose a matrix",
        body: "v t exchanges the rows and columns of a matrix: [[1, 2, 3], [4, 5, 6]] v t is [[1, 4], [2, 5], [3, 6]]. A flat vector becomes a matrix of one column, as in Calc: [1, 2, 3] v t is [[1], [2], [3]].",
        help: Some(84),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "V J",
        name: "ctrn",
        summary: "conjugate transpose",
        body: "V J transposes a matrix and conjugates every element: [[(1, 2), 3], [4, (5, 6)]] V J is [[(1, -2), 4], [3, (5, -6)]].",
        help: Some(88),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "V T",
        name: "trace",
        summary: "sum of the diagonal",
        body: "V T replaces a square matrix with the sum of its diagonal. [[1, 2], [3, 4]] V T is 5.",
        help: Some(87),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "v i",
        name: "identity",
        summary: "identity matrix of order n",
        body: "v i replaces n with the n by n identity matrix. 3 v i is [[1, 0, 0], [0, 1, 0], [0, 0, 1]]. Calc asks for n in a prompt; here it comes from the stack, like v x. Up to 100 by 100.",
        help: Some(83),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "V C",
        name: "cross",
        summary: "cross product",
        body: "V C is the cross product of two vectors of three numbers. [1, 2, 3] [4, 5, 6] V C is [-3, 6, -3].",
        help: Some(89),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "n",
        name: "negate",
        summary: "change sign",
        body: "n changes the sign of the top value. While a number is being typed, n toggles a leading _.",
        help: Some(17),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "&",
        name: "recip",
        summary: "1/x",
        body: "& replaces x with 1/x. A square matrix gives its inverse: [[1, 2], [3, 4]] & is [[-2, 1], [1.5, -0.5]].",
        help: Some(18),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "Q",
        name: "sqrt",
        summary: "square root",
        body: "Q takes the square root. A perfect square of an integer stays an integer. A negative becomes a complex value (0, imag).",
        help: Some(19),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "I Q",
        name: "square",
        summary: "x^2",
        body: "I Q replaces x with x^2. I is the one-shot inverse prefix. A vector gives its dot product with itself, and a square matrix its matrix square.",
        help: Some(20),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "I ^",
        name: "root",
        summary: "nth root",
        body: "I ^ takes the nth root of the buried value. The degree is on top. 125 3 I ^ is 5.",
        help: Some(21),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "A",
        name: "abs",
        summary: "absolute value",
        body: "A is the absolute value. On a complex number it is the magnitude, so (3, 4) A is 5. A vector gives its length, and a matrix the root of the sum of its squared elements.",
        help: Some(22),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "f A",
        name: "abs^2",
        summary: "square of the magnitude",
        body: "f A is the square of the absolute value. A complex number becomes re^2 + im^2.",
        help: Some(23),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "I",
        name: "inverse",
        summary: "one-shot inverse prefix",
        body: "I arms the inverse for the next operation. I S is arcsin, I Q squares, and I ^ takes an nth root. Pushing or duplicating leaves I set.",
        help: Some(15),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "H",
        name: "hyp",
        summary: "one-shot hyperbolic prefix",
        body: "H arms the hyperbolic form of the next operation. H S is sinh, H L is log10, and H P pushes e. It is used once.",
        help: Some(16),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "s s",
        name: "store",
        summary: "copy the top into a variable",
        body: "s s copies the top into a variable and leaves it on the stack. A digit stores q0 through q9, as in s 3. A longer name ends with RET.",
        help: Some(70),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "s t",
        name: "into",
        summary: "pop the top into a variable",
        body: "s t pops the top into a variable. A digit pops into q0 through q9. t 3 does the same.",
        help: Some(71),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "s r",
        name: "recall",
        summary: "push a variable",
        body: "s r pushes a stored variable. A digit recalls q0 through q9, and r 3 does the same. From s l, a letter recalls that one-letter name.",
        help: Some(72),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "s u",
        name: "unstore",
        summary: "forget a variable",
        body: "s u forgets a variable. A digit forgets that q register. A longer name ends with RET.",
        help: Some(73),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "s l",
        name: "list",
        summary: "browse stored variables",
        body: "s l lists the stored variables, five down each column. Tab and S-Tab turn the page. A letter or a digit recalls that name. Esc closes. q is the variable q while this list is open.",
        help: Some(74),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "+",
        name: "add",
        summary: "y + x",
        body: "+ adds the top two values, x on top. While typing, + is an exponent sign, or a character inside an unclosed (…).",
        help: Some(8),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "-",
        name: "sub",
        summary: "y - x",
        body: "- subtracts the top from the value under it. While typing, - is an exponent sign, or a character inside an unclosed (…). A leading minus in an entry is _.",
        help: Some(9),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "*",
        name: "mul",
        summary: "y * x",
        body: "* multiplies the top two values. A number scales a vector or a matrix. Two vectors give the dot product: [1, 2] [3, 4] * is 11. Two matrices give the matrix product, and a matrix with a vector takes the vector as a column on the right and as a row on the left: [[1, 2], [3, 4]] [5, 6] * is [17, 39]. Sizes that do not fit are dimensions do not match. V M * multiplies element by element.",
        help: Some(10),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "/",
        name: "div",
        summary: "y / x",
        body: "/ divides the buried value by the top. Integers that divide evenly stay integers. Otherwise the result is a float at the current precision. m f makes it a fraction instead. Dividing by a square matrix solves a linear system, as in Calc: B A / is the X with A X = B, so [5, 6] [[1, 2], [3, 4]] / is [-4, 4.5].",
        help: Some(11),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "^",
        name: "power",
        summary: "y^x",
        body: "^ raises the buried value to the top. A non-real result becomes complex. I ^ takes an nth root. A square matrix takes an integer power: [[1, 2], [3, 4]] 3 ^ is [[37, 54], [81, 118]], 0 gives the identity, and a negative power is a power of the inverse.",
        help: Some(12),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "\\",
        name: "idiv",
        summary: "integer quotient",
        body: "\\ divides and keeps the integer quotient, toward minus infinity. 10 \\ 3 is 3, and -5 \\ 3 is -2. An integer or a fraction divides exactly; a float rounds at the current precision.",
        help: Some(13),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "F",
        name: "floor",
        summary: "integer toward minus infinity",
        body: "F rounds toward minus infinity and returns an exact integer. 45.3 F is 45, and -45.3 F is -46. A float such as 4.02e26 expands to every digit. A complex value is an error.",
        help: Some(27),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "%",
        name: "mod",
        summary: "remainder",
        body: "% is the remainder from integer division toward minus infinity. 10 % 3 is 1, and -5 % 3 is 1. An integer or a fraction is exact; a float rounds at the current precision.",
        help: Some(14),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: ":",
        name: "fraction",
        summary: "exact division",
        body: "On an empty entry, : divides the top two as a fraction, so 3 4 : is 3:4. While typing, : is the fraction bar.",
        help: Some(63),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "M",
        name: "modform",
        summary: "modulo form",
        body: "While typing an integer, M starts its modulus, so 3M1000 enters 3 mod 1000. A second M fills in the previous modulus. Both sides are integers, the modulus is positive, and the value is kept in 0 <= a < m. + - * / ^, n, &, and I Q work with an integer or a form of the same modulus. / multiplies by the modular inverse. ^ uses fast powering, so 3M1000 567 ^ is 587 mod 1000, and a negative power inverts first. Other operations say not for modulo forms.",
        help: Some(64),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "!",
        name: "fact",
        summary: "factorial",
        body: "! is the factorial. A non-negative integer is exact, through 100000. Any other real uses the gamma function. A negative integer is an error.",
        help: Some(45),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k g",
        name: "gcd",
        summary: "greatest common divisor",
        body: "k g is the greatest common divisor of the top two values. For fractions it is the gcd of the numerators over the lcm of the denominators.",
        help: Some(50),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k l",
        name: "lcm",
        summary: "least common multiple",
        body: "k l is the least common multiple of the top two values. Fractions are accepted.",
        help: Some(51),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k E",
        name: "egcd",
        summary: "extended gcd",
        body: "k E takes y on top and x under it and gives the vector [g, a, b], with g the gcd and a*x + b*y = g, as Calc's egcd. 2 5 k E is [1, -2, 1], and (-2)*2 + 1*5 = 1. v u puts g, a, and b on the stack.",
        help: Some(52),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k i",
        name: "invmod",
        summary: "modular inverse",
        body: "k i replaces a and m with b. m is on top, 0 <= b < m, and a*b is 1 modulo m. 3 7 k i is 5. The modulus is a positive integer. If the gcd is not 1, the message is no modular inverse.",
        help: Some(53),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k p",
        name: "prime",
        summary: "primality test, 1 or 0",
        body: "k p replaces the integer on top with 1 if it is prime and 0 if not, as Calc's prime function, and the message says prime, not prime, or probably prime. A probably prime number gives 1. Anything below 2 gives 0. A vector is tested element by element: [2, 4, 7] k p is [1, 0, 1]. RET first keeps the number.",
        help: Some(54),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k f",
        name: "factor",
        summary: "prime factors",
        body: "k f replaces an integer with the vector of its prime factors, smallest first, as Calc's prfac: 2023 k f is [7, 17, 17], and V R * multiplies them back to 2023. v u puts the factors on the stack one by one. A negative number leads with -1. 0 and 1 give [0] and [1]. A vector of integers gives a vector of factor lists: [6, 8, 15] k f is [[2, 3], [2, 2, 2], [3, 5]]. Trial division uses Calc's prime table through 5003, then Pollard rho splits a composite cofactor up to 1024 bits. A cofactor that cannot be proven prime or split stays in one piece, and the message is may not be fully factored.",
        help: Some(55),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k t",
        name: "totient",
        summary: "Euler's totient",
        body: "k t replaces a nonnegative integer n with Euler's totient phi(n), the count of 1..n coprime to n. 12 k t is 4, and a prime p gives p - 1. 0 and 1 stay as they are. It factors n first, so a number that cannot be fully factored says number too big to factor.",
        help: Some(56),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k n",
        name: "nextprime",
        summary: "next prime",
        body: "k n replaces a number with the smallest prime above it. 7 k n is 11. A non-integer is truncated first, and anything below 2 gives 2. Above the proven bound the message is probably prime.",
        help: Some(57),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "Ik n",
        name: "prevprime",
        summary: "previous prime",
        body: "I k n replaces a number with the largest prime below it. 11 I k n is 7. A non-integer is rounded up first, and anything below 4 gives 2, as in Calc.",
        help: Some(58),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "Hk n",
        name: "nthprime",
        summary: "nth prime",
        body: "H k n replaces n with the nth prime, as PARI/GP's prime(n). 1 H k n is 2 and 1000000 H k n is 15485863. n runs from 1 to 100000000; above that is overflow.",
        help: Some(59),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k c",
        name: "choose",
        summary: "binomial coefficient",
        body: "k c is N! / (M! (N-M)!), with M on top. 5 2 k c is 10. Integers stay exact.",
        help: Some(46),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "Hk c",
        name: "perm",
        summary: "permutations",
        body: "H k c is N! / (N-M)!, with M on top. 5 2 H k c is 20.",
        help: Some(47),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "k s",
        name: "stirling1",
        summary: "Stirling number, first kind",
        body: "k s is the signed Stirling number of the first kind s(n, m), with m on top. 4 2 k s is 11. n and m are nonnegative integers, and m > n is 0.",
        help: Some(48),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "Hk s",
        name: "stirling2",
        summary: "Stirling number, second kind",
        body: "H k s is the Stirling number of the second kind S(n, m), with m on top. 4 2 H k s is 7. n and m are nonnegative integers, and m > n is 0.",
        help: Some(49),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "P",
        name: "pi",
        summary: "push pi",
        body: "P pushes pi at the current precision. H P pushes e.",
        help: Some(30),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "J",
        name: "conj",
        summary: "complex conjugate",
        body: "J replaces a complex value (re, im) with (re, -im). A real value stays as it is.",
        help: Some(25),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "S",
        name: "sin",
        summary: "sine",
        body: "S is the sine. Angles start in degrees. I S is arcsin, H S is sinh, and H I S is asinh.",
        help: Some(32),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "C",
        name: "cos",
        summary: "cosine",
        body: "C is the cosine. I C is arccos, H C is cosh, and H I C is acosh.",
        help: Some(33),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "T",
        name: "tan",
        summary: "tangent",
        body: "T is the tangent. I T is arctan, H T is tanh, and H I T is atanh. f T is atan2.",
        help: Some(34),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "L",
        name: "ln",
        summary: "natural log",
        body: "L is the natural log. The key flips the inverse flag first, so I L is exp. H L is log10, and H I L is 10^x.",
        help: Some(37),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "E",
        name: "exp",
        summary: "e^x",
        body: "E is the exponential. I E is ln. H E is 10^x. Unlike L, E does not flip the inverse flag.",
        help: Some(38),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "B",
        name: "log",
        summary: "logarithm, base on top",
        body: "B is the logarithm of the buried value in the base on top. 8 2 B is 3. An integer base greater than 1 stays exact when the value is a ratio of integer powers of that base, so 1:8 2 B is -3.",
        help: Some(39),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "I B",
        name: "antilog",
        summary: "base^exponent",
        body: "I B raises the base on top to the buried power. 3 2 I B is 8.",
        help: Some(40),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "H L",
        name: "log10",
        summary: "common log",
        body: "H L is the base-10 logarithm. A power of ten, or its reciprocal, stays an integer, so 1000 H L is 3 and 1:1000 H L is -3.",
        help: Some(41),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "H E",
        name: "10^x",
        summary: "power of ten",
        body: "H E replaces x with 10^x.",
        help: Some(42),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "H P",
        name: "e",
        summary: "push e",
        body: "H P pushes e at the current precision. P pushes pi. e is not in the K catalog.",
        help: Some(31),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "f T",
        name: "atan2",
        summary: "angle of (x, y)",
        body: "f T is atan2(y, x) with x on top, in the current angle mode. 1 0 f T is 90 degrees. A point on an axis is an exact right angle, and 0 0 f T is 0.",
        help: Some(35),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "f h",
        name: "hypot",
        summary: "hypotenuse",
        body: "f h is the square root of the sum of squares of the top two values. 3 4 f h is 5.",
        help: Some(36),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "f Q",
        name: "isqrt",
        summary: "integer square root",
        body: "f Q is the greatest integer whose square is at most the value. 10 f Q is 3. An integer, or a float that is already a whole number, is exact. A negative number is an error.",
        help: Some(28),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "f E",
        name: "expm1",
        summary: "exp(x) - 1",
        body: "f E is exp(x) - 1. A small x keeps digits that exp(x) then subtracting 1 would lose.",
        help: Some(43),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "f L",
        name: "lnp1",
        summary: "ln(1 + x)",
        body: "f L is ln(1 + x). A small x keeps digits that adding 1 then taking ln would lose.",
        help: Some(44),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "f I",
        name: "ilog",
        summary: "integer logarithm",
        body: "f I is the integer log of the buried value, with the base on top. 1000 10 f I is 3. A result that is not already whole is floored.",
        help: Some(29),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "f s",
        name: "sign",
        summary: "sign of x",
        body: "f s is -1, 0, or 1 for a real value. A complex value becomes that value divided by its magnitude.",
        help: Some(24),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "G",
        name: "arg",
        summary: "complex argument",
        body: "G is the argument of a complex value, in the current angle mode. The argument of zero is an error.",
        help: Some(26),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "K",
        name: "const",
        summary: "physics and astronomy constants",
        body: "K opens the constant catalog, sixteen to a page. 0-9 and a-f insert that slot. Tab and S-Tab turn the page. The unit is only a label. Esc closes, and q does not quit while the catalog is open.",
        help: Some(68),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "t y",
        name: "trail",
        summary: "pick a past value from the trail",
        body: "t y opens the trail: every number pushed and every result, newest at the bottom, with the command that made it. Typing filters by label or digits. Up and Down, or Tab and S-Tab, move. Left and Right, C-a, and C-e scroll a long value. RET pushes the selected value, and u takes it back. Esc closes. Undo does not remove trail entries. The trail keeps the last 1000 values for this session.",
        help: Some(69),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "p",
        name: "prec",
        summary: "working precision",
        body: "p sets the precision in significant digits from the integer on top, then pops it. The default is 12, and the value must be from 3 to 1000. A float is written in full while its order runs from -2 through one less than that precision, and in scientific form past either end. Undo puts the number back and leaves the precision.",
        help: Some(67),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "m d",
        name: "deg",
        summary: "degrees",
        body: "m d measures angles in degrees. Real trig and both parts of complex trig use this mode. A degree angle is reduced modulo 360.",
        help: Some(90),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "m r",
        name: "rad",
        summary: "radians",
        body: "m r measures angles in radians.",
        help: Some(91),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "m f",
        name: "fractions",
        summary: "fraction mode",
        body: "m f toggles fraction mode. While it is on, / of two numbers that do not divide evenly stays a fraction. : divides as a fraction either way.",
        help: Some(92),
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
        help: Some(93),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "d 8",
        name: "oct",
        summary: "octal display",
        body: "d 8 shows integers in octal. # uses octal. O d 8 shows two's complement.",
        help: Some(94),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "d 6",
        name: "hex",
        summary: "hexadecimal display",
        body: "d 6 shows integers in hexadecimal. 16#FF is 255 in any mode. From radix 15 up, e is a digit, so a power is written 16#1.2*16^3.",
        help: Some(95),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "d 0",
        name: "dec",
        summary: "decimal display",
        body: "d 0 returns the display to decimal and turns two's complement off.",
        help: Some(96),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "d r",
        name: "radix",
        summary: "display radix 2-36",
        body: "d r asks for a radix from 2 to 36, then digits, then RET. Through radix 14, e is a power of that radix, as in 8#1.2e3. q cancels the prompt.",
        help: Some(97),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "d z",
        name: "zeros",
        summary: "pad integers to the word size",
        body: "d z pads integers with leading zeros out to the word size. The stored value stays the same. Fractions and floats are left as they are.",
        help: Some(98),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "d g",
        name: "group",
        summary: "commas in integers",
        body: "d g groups integer digits with commas: 4 digits in binary and hex, 3 otherwise. The fractional part and the exponent stay plain. Grouping is display only.",
        help: Some(99),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "b w",
        name: "word",
        summary: "word size",
        body: "b w sets the width used by leading zeros and two's complement. The default is 32. A negative size uses that same width. Finish with RET. q cancels the prompt.",
        help: Some(100),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "O d",
        name: "twos",
        summary: "two's complement display",
        body: "O before a radix key shows binary, octal, and hex integers in two's complement. An 8-bit -3 is 16##FD. Choosing a radix without O turns that display off.",
        help: Some(101),
        bar: None,
        mode: None,
        catalog: true,
    },
    Binding {
        keys: "Esc",
        name: "cancel",
        summary: "drop entry, prefix, or dialog",
        body: "Esc cancels the number being typed, a prefix, or an open dialog. The stack stays. q quits when nothing is being typed.",
        help: Some(6),
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
        keys: "v",
        name: "v-",
        summary: "",
        body: "",
        help: None,
        bar: None,
        mode: Some("v-"),
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
        assert_eq!(pos("C"), pos("S") + 1);
        assert_eq!(pos("T"), pos("C") + 1);
        assert_eq!(pos("M"), pos(":") + 1);
        assert_eq!(pos("k c"), pos("!") + 1);

        // Each prefix keeps its keys together: `k`, `s`, `v` and `V`,
        // `d`, and the `k` random keys right after the rest of `k`.
        let run = |prefix: &str| -> Vec<usize> {
            entries
                .iter()
                .enumerate()
                .filter(|(_, (keys, _))| keys.starts_with(prefix))
                .map(|(i, _)| i)
                .collect()
        };
        for prefix in ["s ", "d ", "m "] {
            let at = run(prefix);
            assert_eq!(at.last().unwrap() - at[0] + 1, at.len(), "{prefix}");
        }
        let k: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, (keys, _))| {
                keys.starts_with("k ") || keys.starts_with("Hk ") || keys.starts_with("Ik ")
            })
            .map(|(i, _)| i)
            .collect();
        assert_eq!(k.last().unwrap() - k[0] + 1, k.len(), "k");
        let vectors: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, (keys, _))| {
                keys.starts_with("v ") || keys.starts_with("V ") || keys.starts_with('[')
            })
            .map(|(i, _)| i)
            .collect();
        assert_eq!(vectors.last().unwrap() - vectors[0] + 1, vectors.len());

        // Three columns of fifteen: stack and arithmetic, functions, then
        // constants, trig, and logs. Page two: `!` and `k`, then random,
        // entry, and storage, then vectors and matrices, one column. The
        // twelve left over are the modes and display, page three.
        let page = help_columns(0, 3, HELP_ROWS);
        assert_eq!(page.len(), 3);
        assert!(page.iter().all(|column| column.len() == HELP_ROWS));
        assert_eq!(page[0][0].0, "ret");
        assert_eq!(page[1][0].0, "I");
        assert_eq!(page[2][0].0, "P");

        let rest = help_columns(1, 3, HELP_ROWS);
        assert!(rest.iter().all(|column| column.len() == HELP_ROWS));
        assert_eq!(rest[0][0].0, "!");
        assert_eq!(rest[1][0].0, "k r");
        assert_eq!(rest[2][0].0, "[ ]");
        assert_eq!(rest[2][HELP_ROWS - 1].0, "V C");
        let last = help_columns(2, 3, HELP_ROWS);
        assert_eq!(last[0][0].0, "m d");
        let shown: usize = last.iter().map(|column| column.len()).sum();
        assert_eq!(shown, entries.len() - HELP_ROWS * 3 * 2);
        assert_eq!(shown, 12);

        let short = help_columns(0, 3, 13);
        assert_eq!(short.iter().map(|column| column.len()).sum::<usize>(), 39);
        assert_eq!(short[0][0].0, "ret");
        assert_eq!(help_columns(1, 3, 13)[0][0].0, "B");
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
    fn matrix_bindings_are_searchable() {
        assert_eq!(search("matrix")[0].keys, "[[ ]]");
        assert_eq!(search("det")[0].keys, "V D");
        assert_eq!(search("inverse")[0].keys, "v &");
        assert_eq!(search("transpose")[0].keys, "v t");
        assert_eq!(search("identity")[0].keys, "v i");
        assert_eq!(search("cross")[0].keys, "V C");
        assert_eq!(search("trace")[0].keys, "V T");
        assert_eq!(search("vj")[0].name, "ctrn");
        // They are on the third page of the shortcut list.
        let listed = help_entries();
        for keys in [
            "[[ ]]", "V D", "v &", "v t", "V J", "V T", "v i", "V C", "v l",
        ] {
            assert!(listed.iter().any(|(entry, _)| *entry == keys), "{keys}");
        }
    }

    #[test]
    fn modulo_forms_are_searchable() {
        assert_eq!(search("modform")[0].keys, "M");
        assert!(search("modulo").iter().any(|entry| entry.keys == "M"));
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
