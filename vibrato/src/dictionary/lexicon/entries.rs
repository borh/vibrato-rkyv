use crate::dictionary::LexType;
use crate::dictionary::lexicon::map::Surfaces;
use crate::dictionary::lexicon::{ArchivedLexicon, Lexicon};
use crate::dictionary::word_idx::WordIdx;
use crate::errors::Result;

/// Read-only view of a system or user lexicon.
///
/// Obtained from [`Dictionary::system_lexicon`](crate::Dictionary::system_lexicon),
/// [`Dictionary::user_lexicon`](crate::Dictionary::user_lexicon), or the
/// corresponding [`Tokenizer`](crate::Tokenizer) methods.
#[derive(Clone, Copy)]
pub struct LexiconRef<'a> {
    inner: LexiconKind<'a>,
}

#[derive(Clone, Copy)]
enum LexiconKind<'a> {
    Archived(&'a ArchivedLexicon),
    Owned(&'a Lexicon),
}

impl<'a> LexiconRef<'a> {
    pub(crate) const fn archived(lexicon: &'a ArchivedLexicon) -> Self {
        Self {
            inner: LexiconKind::Archived(lexicon),
        }
    }

    pub(crate) const fn owned(lexicon: &'a Lexicon) -> Self {
        Self {
            inner: LexiconKind::Owned(lexicon),
        }
    }

    /// Recovers the surface of every word, so that the words can be enumerated.
    ///
    /// The lexicon stores surfaces only as keys of its double-array trie, so this
    /// walks the whole trie once and copies each distinct surface into one buffer.
    /// Feature strings are not copied; entries borrow them from the lexicon.
    ///
    /// # Errors
    ///
    /// [`VibratoError`](crate::errors::VibratoError) is returned when the trie
    /// and the word list disagree, that is, when a word has no surface or more
    /// than one.
    pub fn entries(&self) -> Result<LexiconEntries<'a>> {
        let (lex_type, surfaces) = match self.inner {
            LexiconKind::Archived(lexicon) => (
                lexicon.lex_type.to_native(),
                lexicon.map.surfaces(lexicon.features.len())?,
            ),
            LexiconKind::Owned(lexicon) => (
                lexicon.lex_type,
                lexicon.map.surfaces(lexicon.features.len())?,
            ),
        };
        Ok(LexiconEntries {
            lexicon: *self,
            lex_type,
            surfaces,
        })
    }

    #[inline(always)]
    fn feature(&self, word_id: usize) -> &'a str {
        match self.inner {
            LexiconKind::Archived(lexicon) => lexicon.features.get(word_id),
            LexiconKind::Owned(lexicon) => lexicon.features.get(word_id),
        }
    }
}

/// Every word of a lexicon with its surface, returned by [`LexiconRef::entries`].
pub struct LexiconEntries<'a> {
    lexicon: LexiconRef<'a>,
    lex_type: LexType,
    surfaces: Surfaces,
}

impl LexiconEntries<'_> {
    /// Returns an iterator over the words in word-id order, which is the row
    /// order of the lexicon CSV.
    ///
    /// Each word appears exactly once. Words that share a surface, such as
    /// homographs with different parts of speech, are separate entries.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = LexEntry<'_>> + '_ {
        (0..self.surfaces.len()).map(move |word_id| LexEntry {
            word_idx: WordIdx::new(self.lex_type, u32::try_from(word_id).unwrap()),
            surface: self.surfaces.get(word_id),
            feature: self.lexicon.feature(word_id),
        })
    }
}

/// One word of a lexicon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LexEntry<'a> {
    word_idx: WordIdx,
    surface: &'a str,
    feature: &'a str,
}

impl<'a> LexEntry<'a> {
    /// Gets the word index, the same value [`Token::word_idx`](crate::token::Token::word_idx)
    /// reports for a token of this word.
    #[inline(always)]
    pub const fn word_idx(&self) -> WordIdx {
        self.word_idx
    }

    /// Gets the surface string, the key under which the lexicon stores the word.
    #[inline(always)]
    pub const fn surface(&self) -> &'a str {
        self.surface
    }

    /// Gets the feature string.
    #[inline(always)]
    pub const fn feature(&self) -> &'a str {
        self.feature
    }
}
