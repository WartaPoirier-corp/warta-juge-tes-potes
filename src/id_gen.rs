use itertools::Itertools;
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::iter::{self, FromIterator, FusedIterator};

#[derive(Debug)]
pub struct FunnyWords {
    /// Map of ASCII character (u8) to `Vec<Option<char>>` representing the possible following
    /// characters according to the funny word list. `None` represents a word (end) boundary.
    /// `next_letter[0]` represents word beginnings.
    next_letter: [Vec<Option<char>>; 128],
}

impl Default for FunnyWords {
    fn default() -> Self {
        const EMPTY_VEC: Vec<Option<char>> = Vec::new();

        Self {
            next_letter: [EMPTY_VEC; 128],
        }
    }
}

impl FunnyWords {
    pub fn push(&mut self, word: impl AsRef<str>) {
        let word = word.as_ref();

        for (key, next) in iter::once(None)
            .chain(word.chars().map(Some))
            .chain(iter::once(None))
            .tuple_windows()
        {
            let key = match key {
                None => 0,
                Some(c) => c as u8 as usize,
            };

            self.next_letter[key].push(next);
        }
    }
}

impl<S: AsRef<str>> FromIterator<S> for FunnyWords {
    fn from_iter<T: IntoIterator<Item = S>>(words: T) -> Self {
        let mut ret = Self::default();
        for word in words {
            ret.push(word);
        }
        ret
    }
}

#[derive(Clone)]
pub struct Chain<'a, R> {
    funny_words: &'a FunnyWords,
    entropy: f32,
    current: Option<char>,
    rng: R,
}

impl<'a, R: Rng> Chain<'a, R> {
    pub fn new(funny_words: &'a FunnyWords, entropy: f32, rng: R) -> Self {
        Self {
            funny_words,
            entropy,
            current: None,
            rng,
        }
    }

    fn rand_char(&mut self) -> char {
        (self.rng.sample(Alphanumeric) as char).to_ascii_uppercase()
    }
}

impl<R: Rng> Iterator for Chain<'_, R> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        use rand::seq::SliceRandom;

        let entropy = self.entropy;

        let ret = self
            .current
            .or_else(|| {
                self.funny_words.next_letter[0]
                    .choose(&mut self.rng)
                    .copied()
                    .flatten()
            })
            .filter(|_| self.rng.gen::<f32>() > entropy)
            .and_then(|key| {
                self.funny_words.next_letter[key as u8 as usize]
                    .choose(&mut self.rng)
                    .copied()
                    .flatten()
            })
            .or_else(|| Some(self.rand_char()));

        // `ret` will always be `Some` at this point

        self.current = ret;
        ret
    }
}

/// Micro-optimisation, I don't know if this is useful here...
/// Valid because `Chain<R>` is actually infinite.
impl<R: Rng> FusedIterator for Chain<'_, R> {}
