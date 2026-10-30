//! An *algorithm* is the wiring between the 4 operators: who modulates whom,
//! and which ones you actually hear.
//!
//! - A **carrier** is heard: its output goes to the speakers.
//! - A **modulator** is not heard directly: its output bends another
//!   operator's phase, which changes that operator's *timbre*.
//!
//! In the diagrams `4→3` means "operator 4 modulates operator 3", and `+`
//! means "both are heard". Rule: an operator can only be modulated by a
//! higher-numbered one. That way we can compute them in order 4, 3, 2, 1 and
//! every modulator's output is ready before it's needed.
//!
//! In code, operators are indexed from 0: index 0 is operator 1, and so on.

pub struct Algorithm {
    pub name: &'static str,
    /// `modulators[i]` lists the operators that modulate operator `i`.
    pub modulators: [&'static [usize]; 4],
    /// The operators you hear.
    pub carriers: &'static [usize],
}

pub const ALGORITHMS: [Algorithm; 8] = [
    // One long chain: the most complex, harsh timbres. Brass, leads.
    Algorithm {
        name: "4→3→2→1",
        modulators: [&[1], &[2], &[3], &[]],
        carriers: &[0],
    },
    // Two modulators mixed into one, then the chain continues.
    Algorithm {
        name: "(3+4)→2→1",
        modulators: [&[1], &[2, 3], &[], &[]],
        carriers: &[0],
    },
    // Operator 1 is modulated by a simple sine (2) and a complex one (4→3).
    Algorithm {
        name: "(2 + 4→3)→1",
        modulators: [&[1, 2], &[], &[3], &[]],
        carriers: &[0],
    },
    Algorithm {
        name: "(3→2 + 4)→1",
        modulators: [&[1, 3], &[2], &[], &[]],
        carriers: &[0],
    },
    // Two independent 2-operator sounds layered together. The classic choice
    // for electric pianos: one pair for the body, one for the "tine".
    Algorithm {
        name: "2→1 + 4→3",
        modulators: [&[1], &[], &[3], &[]],
        carriers: &[0, 2],
    },
    // One modulator shared by three carriers.
    Algorithm {
        name: "4→(1+2+3)",
        modulators: [&[3], &[3], &[3], &[]],
        carriers: &[0, 1, 2],
    },
    // Three carriers, one of them modulated. Organs with a percussive edge.
    Algorithm {
        name: "1 + 2 + 4→3",
        modulators: [&[], &[], &[3], &[]],
        carriers: &[0, 1, 2],
    },
    // No modulation at all: 4 sine waves added up, like organ drawbars.
    Algorithm {
        name: "1+2+3+4",
        modulators: [&[], &[], &[], &[]],
        carriers: &[0, 1, 2, 3],
    },
];
