//! One short page for each key. `? ?` opens the list. Typing filters it.

pub(crate) struct Entry {
    pub keys: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    pub body: &'static str,
}

const ENTRIES: &[Entry] = &[
    Entry {
        keys: "ret",
        name: "push",
        summary: "push or duplicate",
        body: "RET or space pushes the number being typed. An empty entry duplicates the top of the stack, and leaves a pending I or H in place.",
    },
    Entry {
        keys: "del",
        name: "pop",
        summary: "erase or pop",
        body: "DEL erases the last typed character. With an empty entry it pops the top of the stack.",
    },
    Entry {
        keys: "tab",
        name: "swap",
        summary: "exchange the top two",
        body: "TAB exchanges the top two values.",
    },
    Entry {
        keys: "TAB",
        name: "roll",
        summary: "rotate the top three",
        body: "Shift-Tab brings the third value up to the top. The old top two each drop one level. Fewer than three values is an error.",
    },
    Entry {
        keys: "u",
        name: "undo",
        summary: "restore stack and variables",
        body: "u restores the stack and the stored variables. Mode changes stay as they are. The history keeps 100 steps.",
    },
    Entry {
        keys: "C-r",
        name: "redo",
        summary: "undo an undo",
        body: "C-r walks back toward the stack you undid. A new calculation drops the redo history.",
    },
    Entry {
        keys: "n",
        name: "negate",
        summary: "change sign",
        body: "n changes the sign of the top value. While a number is being typed, n toggles a leading _.",
    },
    Entry {
        keys: "&",
        name: "recip",
        summary: "1/x",
        body: "& replaces x with 1/x.",
    },
    Entry {
        keys: "Q",
        name: "sqrt",
        summary: "square root",
        body: "Q takes the square root. A perfect square of an integer stays an integer. A negative becomes a complex value (0, imag).",
    },
    Entry {
        keys: "I Q",
        name: "square",
        summary: "x^2",
        body: "I Q replaces x with x^2. I is the one-shot inverse prefix.",
    },
    Entry {
        keys: "I ^",
        name: "root",
        summary: "nth root",
        body: "I ^ takes the nth root of the buried value. The degree is on top. 125 3 I ^ is 5.",
    },
    Entry {
        keys: "A",
        name: "abs",
        summary: "absolute value",
        body: "A is the absolute value. On a complex number it is the magnitude, so (3, 4) A is 5.",
    },
    Entry {
        keys: "f A",
        name: "abs^2",
        summary: "square of the magnitude",
        body: "f A is the square of the absolute value. A complex number becomes re^2 + im^2.",
    },
    Entry {
        keys: "I",
        name: "inverse",
        summary: "one-shot inverse prefix",
        body: "I arms the inverse for the next operation. I S is arcsin, I Q squares, and I ^ takes an nth root. Pushing or duplicating leaves I set.",
    },
    Entry {
        keys: "H",
        name: "hyp",
        summary: "one-shot hyperbolic prefix",
        body: "H arms the hyperbolic form of the next operation. H S is sinh, H L is log10, and H P pushes e. It is used once.",
    },
    Entry {
        keys: "s s",
        name: "store",
        summary: "copy the top into a variable",
        body: "s s copies the top into a variable and leaves it on the stack. A digit stores q0 through q9, as in s 3. A longer name ends with RET.",
    },
    Entry {
        keys: "s t",
        name: "into",
        summary: "pop the top into a variable",
        body: "s t pops the top into a variable. A digit pops into q0 through q9. t 3 does the same.",
    },
    Entry {
        keys: "s r",
        name: "recall",
        summary: "push a variable",
        body: "s r pushes a stored variable. A digit recalls q0 through q9, and r 3 does the same. From s l, a letter recalls that one-letter name.",
    },
    Entry {
        keys: "s u",
        name: "unstore",
        summary: "forget a variable",
        body: "s u forgets a variable. A digit forgets that q register. A longer name ends with RET.",
    },
    Entry {
        keys: "s l",
        name: "list",
        summary: "browse stored variables",
        body: "s l lists the stored variables, five down each column. Tab and Shift-Tab turn the page. A letter or a digit recalls that name. Esc closes. q is the variable q while this list is open.",
    },
    Entry {
        keys: "+",
        name: "add",
        summary: "y + x",
        body: "+ adds the top two values, x on top. While typing, + is an exponent sign, or a character inside an unclosed (…).",
    },
    Entry {
        keys: "-",
        name: "sub",
        summary: "y - x",
        body: "- subtracts the top from the value under it. While typing, - is an exponent sign, or a character inside an unclosed (…). A leading minus in an entry is _.",
    },
    Entry {
        keys: "*",
        name: "mul",
        summary: "y * x",
        body: "* multiplies the top two values.",
    },
    Entry {
        keys: "/",
        name: "div",
        summary: "y / x",
        body: "/ divides the buried value by the top. Integers that divide evenly stay integers. Otherwise the result is a float at the current precision. m f makes it a fraction instead.",
    },
    Entry {
        keys: "^",
        name: "power",
        summary: "y^x",
        body: "^ raises the buried value to the top. A non-real result becomes complex. I ^ takes an nth root.",
    },
    Entry {
        keys: "\\",
        name: "idiv",
        summary: "integer quotient",
        body: "\\ divides and keeps the integer quotient, toward minus infinity. 10 \\ 3 is 3, and -5 \\ 3 is -2.",
    },
    Entry {
        keys: "F",
        name: "floor",
        summary: "integer toward minus infinity",
        body: "F rounds toward minus infinity and returns an exact integer. 45.3 F is 45, and -45.3 F is -46. A float such as 4.02e26 expands to every digit. A complex value is an error.",
    },
    Entry {
        keys: "%",
        name: "mod",
        summary: "remainder",
        body: "% is the remainder from integer division toward minus infinity. 10 % 3 is 1, and -5 % 3 is 1.",
    },
    Entry {
        keys: ":",
        name: "fraction",
        summary: "exact division",
        body: "On an empty entry, : divides the top two as a fraction, so 3 4 : is 3:4. While typing, : is the fraction bar.",
    },
    Entry {
        keys: "!",
        name: "fact",
        summary: "factorial",
        body: "! is the factorial. A non-negative integer is exact, through 100000. Any other real uses the gamma function. A negative integer is an error.",
    },
    Entry {
        keys: "k g",
        name: "gcd",
        summary: "greatest common divisor",
        body: "k g is the greatest common divisor of the top two values. For fractions it is the gcd of the numerators over the lcm of the denominators.",
    },
    Entry {
        keys: "k l",
        name: "lcm",
        summary: "least common multiple",
        body: "k l is the least common multiple of the top two values. Fractions are accepted.",
    },
    Entry {
        keys: "k E",
        name: "egcd",
        summary: "extended gcd",
        body: "k E takes y on top and x under it. It pushes a, then b, then g, with g on top, so a*x + b*y = g. 2 5 k E leaves -2, then 1, then 1, and (-2)*2 + 1*5 = 1.",
    },
    Entry {
        keys: "k i",
        name: "invmod",
        summary: "modular inverse",
        body: "k i replaces a and m with b. m is on top, 0 <= b < m, and a*b is 1 modulo m. 3 7 k i is 5. The modulus is a positive integer. If the gcd is not 1, the message is no modular inverse.",
    },
    Entry {
        keys: "k p",
        name: "prime",
        summary: "primality test",
        body: "k p reports whether the integer on top is prime, and leaves that number on the stack. The message is prime, not prime, or probably prime.",
    },
    Entry {
        keys: "k c",
        name: "choose",
        summary: "binomial coefficient",
        body: "k c is N! / (M! (N-M)!), with M on top. 5 2 k c is 10. Integers stay exact.",
    },
    Entry {
        keys: "Hk c",
        name: "perm",
        summary: "permutations",
        body: "H k c is N! / (N-M)!, with M on top. 5 2 H k c is 20.",
    },
    Entry {
        keys: "P",
        name: "pi",
        summary: "push pi",
        body: "P pushes pi at the current precision. H P pushes e.",
    },
    Entry {
        keys: "J",
        name: "conj",
        summary: "complex conjugate",
        body: "J replaces a complex value (re, im) with (re, -im). A real value stays as it is.",
    },
    Entry {
        keys: "S",
        name: "sin",
        summary: "sine",
        body: "S is the sine. Angles start in degrees. I S is arcsin, H S is sinh, and H I S is asinh.",
    },
    Entry {
        keys: "C",
        name: "cos",
        summary: "cosine",
        body: "C is the cosine. I C is arccos, H C is cosh, and H I C is acosh.",
    },
    Entry {
        keys: "T",
        name: "tan",
        summary: "tangent",
        body: "T is the tangent. I T is arctan, H T is tanh, and H I T is atanh. f T is atan2.",
    },
    Entry {
        keys: "L",
        name: "ln",
        summary: "natural log",
        body: "L is the natural log. The key flips the inverse flag first, so I L is exp. H L is log10, and H I L is 10^x.",
    },
    Entry {
        keys: "E",
        name: "exp",
        summary: "e^x",
        body: "E is the exponential. I E is ln. H E is 10^x. Unlike L, E does not flip the inverse flag.",
    },
    Entry {
        keys: "B",
        name: "log",
        summary: "logarithm, base on top",
        body: "B is the logarithm of the buried value in the base on top. 8 2 B is 3.",
    },
    Entry {
        keys: "I B",
        name: "antilog",
        summary: "base^exponent",
        body: "I B raises the base on top to the buried power. 3 2 I B is 8.",
    },
    Entry {
        keys: "H L",
        name: "log10",
        summary: "common log",
        body: "H L is the base-10 logarithm. A power of ten stays an integer, so 1000 H L is 3.",
    },
    Entry {
        keys: "H E",
        name: "10^x",
        summary: "power of ten",
        body: "H E replaces x with 10^x.",
    },
    Entry {
        keys: "H P",
        name: "e",
        summary: "push e",
        body: "H P pushes e at the current precision. P pushes pi. e is not in the K catalog.",
    },
    Entry {
        keys: "f T",
        name: "atan2",
        summary: "angle of (x, y)",
        body: "f T is atan2(y, x) with x on top, in the current angle mode. 1 0 f T is 90 degrees. A point on an axis is an exact right angle, and 0 0 f T is 0.",
    },
    Entry {
        keys: "f h",
        name: "hypot",
        summary: "hypotenuse",
        body: "f h is the square root of the sum of squares of the top two values. 3 4 f h is 5.",
    },
    Entry {
        keys: "f Q",
        name: "isqrt",
        summary: "integer square root",
        body: "f Q is the greatest integer whose square is at most the value. 10 f Q is 3. An integer, or a float that is already a whole number, is exact. A negative number is an error.",
    },
    Entry {
        keys: "f E",
        name: "expm1",
        summary: "exp(x) - 1",
        body: "f E is exp(x) - 1. A small x keeps digits that exp(x) then subtracting 1 would lose.",
    },
    Entry {
        keys: "f L",
        name: "lnp1",
        summary: "ln(1 + x)",
        body: "f L is ln(1 + x). A small x keeps digits that adding 1 then taking ln would lose.",
    },
    Entry {
        keys: "f I",
        name: "ilog",
        summary: "integer logarithm",
        body: "f I is the integer log of the buried value, with the base on top. 1000 10 f I is 3. A result that is not already whole is floored.",
    },
    Entry {
        keys: "f s",
        name: "sign",
        summary: "sign of x",
        body: "f s is -1, 0, or 1 for a real value. A complex value becomes that value divided by its magnitude.",
    },
    Entry {
        keys: "G",
        name: "arg",
        summary: "complex argument",
        body: "G is the argument of a complex value, in the current angle mode. The argument of zero is an error.",
    },
    Entry {
        keys: "K",
        name: "const",
        summary: "physics and astronomy constants",
        body: "K opens the constant catalog, sixteen to a page. 0-9 and a-f insert that slot. Tab and Shift-Tab turn the page. The unit is only a label. Esc closes, and q does not quit while the catalog is open.",
    },
    Entry {
        keys: "p",
        name: "prec",
        summary: "working precision",
        body: "p sets the precision in significant digits, then digits, then RET. The default is 12. q cancels the prompt and leaves the stack.",
    },
    Entry {
        keys: "m d",
        name: "deg",
        summary: "degrees",
        body: "m d measures angles in degrees. Real trig and both parts of complex trig use this mode. A degree angle is reduced modulo 360.",
    },
    Entry {
        keys: "m r",
        name: "rad",
        summary: "radians",
        body: "m r measures angles in radians.",
    },
    Entry {
        keys: "m f",
        name: "fractions",
        summary: "fraction mode",
        body: "m f toggles fraction mode. While it is on, / of two numbers that do not divide evenly stays a fraction. : divides as a fraction either way.",
    },
    Entry {
        keys: "c f",
        name: "float",
        summary: "convert to a float",
        body: "c f turns the top value into a float at the current precision.",
    },
    Entry {
        keys: "c F",
        name: "fraction",
        summary: "convert to a fraction",
        body: "c F turns the top value into a fraction. A float that is already a whole number becomes that integer. F floors a float to an integer instead.",
    },
    Entry {
        keys: "d 2",
        name: "bin",
        summary: "binary display",
        body: "d 2 shows integers in binary. A typed number with no # is still decimal. # uses binary, and 2#11111111 is 255. O d 2 shows two's complement.",
    },
    Entry {
        keys: "d 8",
        name: "oct",
        summary: "octal display",
        body: "d 8 shows integers in octal. # uses octal. O d 8 shows two's complement.",
    },
    Entry {
        keys: "d 6",
        name: "hex",
        summary: "hexadecimal display",
        body: "d 6 shows integers in hexadecimal. 16#FF is 255 in any mode. From radix 15 up, e is a digit, so a power is written 16#1.2*16^3.",
    },
    Entry {
        keys: "d 0",
        name: "dec",
        summary: "decimal display",
        body: "d 0 returns the display to decimal and turns two's complement off.",
    },
    Entry {
        keys: "d r",
        name: "radix",
        summary: "display radix 2-36",
        body: "d r asks for a radix from 2 to 36, then digits, then RET. Through radix 14, e is a power of that radix, as in 8#1.2e3. q cancels the prompt.",
    },
    Entry {
        keys: "d z",
        name: "zeros",
        summary: "pad integers to the word size",
        body: "d z pads integers with leading zeros out to the word size. The stored value stays the same. Fractions and floats are left as they are.",
    },
    Entry {
        keys: "d g",
        name: "group",
        summary: "commas in integers",
        body: "d g groups integer digits with commas: 4 digits in binary and hex, 3 otherwise. The fractional part and the exponent stay plain. Grouping is display only.",
    },
    Entry {
        keys: "b w",
        name: "word",
        summary: "word size",
        body: "b w sets the width used by leading zeros and two's complement. The default is 32. A negative size uses that same width. Finish with RET. q cancels the prompt.",
    },
    Entry {
        keys: "O d",
        name: "twos",
        summary: "two's complement display",
        body: "O before a radix key shows binary, octal, and hex integers in two's complement. An 8-bit -3 is 16##FD. Choosing a radix without O turns that display off.",
    },
    Entry {
        keys: "Esc",
        name: "cancel",
        summary: "drop entry, prefix, or dialog",
        body: "Esc cancels the number being typed, a prefix, or an open dialog. The stack stays. q quits when nothing is being typed.",
    },
];

#[cfg(test)]
pub(crate) fn entries() -> &'static [Entry] {
    ENTRIES
}

/// Matching pages, best first. An empty query keeps catalog order.
pub(crate) fn search(query: &str) -> Vec<&'static Entry> {
    let mut ranked: Vec<(u8, usize, &'static Entry)> = ENTRIES
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| rank(entry, query).map(|score| (score, index, entry)))
        .collect();
    ranked.sort_by_key(|(score, index, _)| (*score, *index));
    ranked.into_iter().map(|(_, _, entry)| entry).collect()
}

fn rank(entry: &Entry, query: &str) -> Option<u8> {
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
    fn keys_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for entry in ENTRIES {
            assert!(seen.insert(entry.keys), "duplicate key {}", entry.keys);
        }
    }

    #[test]
    fn isqrt_is_the_first_hit() {
        let hits = search("isqrt");
        assert_eq!(hits[0].keys, "f Q");
        assert_eq!(hits[0].name, "isqrt");
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
        assert_eq!(hits.len(), ENTRIES.len());
        assert_eq!(hits[0].name, "push");
        assert_eq!(hits[1].name, "pop");
    }
}
