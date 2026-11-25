//! A small enum the user picks from (a camera path, an isolation, a read-back mode): one list
//! beside the type, each entry its name as plans spell it, a fresh value and what it is, which
//! the GUI's pickers and the docs read. `choice` is an exhaustive match, so a new variant does
//! not build until it has its index, and `check` (each crate's tests) fails until it is listed.

/// One entry of a type's `CHOICES`.
pub struct Choice<T> {
    /// As plans spell it (the serde name).
    pub name: &'static str,
    /// A fresh value of it (an example's fields where it has any).
    pub make: fn() -> T,
    /// What it is, for a tooltip.
    pub about: &'static str,
}

pub trait Choices: Sized + 'static {
    /// Every variant, in the order offered.
    const CHOICES: &'static [Choice<Self>];

    /// Its index in `CHOICES` (an exhaustive match).
    fn choice(&self) -> usize;

    /// Its entry in `CHOICES`.
    fn entry(&self) -> &'static Choice<Self> {
        &Self::CHOICES[self.choice()]
    }

    /// Whether `other` is the same variant (fields aside).
    fn same_choice(&self, other: &Self) -> bool {
        self.choice() == other.choice()
    }
}

/// For tests: each entry's value is its own variant, the names are distinct, and each name is
/// how serde writes the value (`serialized` gives that spelling).
pub fn check<T: Choices>(serialized: impl Fn(&T) -> String) {
    for (i, c) in T::CHOICES.iter().enumerate() {
        let v = (c.make)();
        assert_eq!(v.choice(), i, "`{}` makes a value of another entry", c.name);
        assert_eq!(serialized(&v), c.name, "`{}` is not how plans spell it", c.name);
        assert!(!c.about.is_empty(), "`{}` says what it is", c.name);
        assert_eq!(T::CHOICES.iter().filter(|o| o.name == c.name).count(), 1, "`{}` listed twice", c.name);
    }
}
