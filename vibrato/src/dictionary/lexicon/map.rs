pub mod posting;
pub mod trie;

use rkyv::{Archive, Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::dictionary::lexicon::map::posting::{Postings, PostingsBuilder};
use crate::dictionary::lexicon::map::trie::Trie;
use crate::errors::{Result, VibratoError};
use crate::utils::FromU32;

#[derive(Archive, Serialize, Deserialize)]
pub struct WordMap {
    trie: Trie,
    postings: Postings,
}

impl WordMap {
    pub fn new<I, W>(words: I) -> Result<Self>
    where
        I: IntoIterator<Item = W>,
        W: AsRef<str>,
    {
        let mut b = WordMapBuilder::new();
        for (i, w) in words.into_iter().enumerate() {
            b.add_record(w.as_ref().to_string(), u32::try_from(i)?);
        }
        b.build()
    }

    #[inline(always)]
    pub fn common_prefix_iterator<'a>(
        &'a self,
        input: &'a [char],
    ) -> impl Iterator<Item = (u32, usize)> + 'a {
        self.trie.common_prefix_iterator(input).flat_map(move |e| {
            self.postings
                .ids(usize::from_u32(e.value))
                .map(move |word_id| (word_id, e.end_char))
        })
    }

    /// Recovers the surface of every word from the trie keys.
    pub fn surfaces(&self, num_words: usize) -> Result<Surfaces> {
        let mut builder = SurfacesBuilder::new(num_words);
        self.trie.for_each_record(|key, offset| {
            let surface = builder.push_key(key)?;
            for word_id in self.postings.ids(usize::from_u32(offset)) {
                builder.assign(word_id, surface)?;
            }
            Ok(())
        })?;
        builder.finish()
    }
}

/// Surface of every word in a lexicon, indexed by word id.
///
/// Words with the same surface share one copy of it.
pub struct Surfaces {
    /// Every distinct surface, concatenated.
    text: String,
    /// End of each distinct surface in `text`.
    ends: Vec<u32>,
    /// Index into `ends` of each word's surface.
    surface_of_word: Vec<u32>,
}

impl Surfaces {
    /// Returns the number of words.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.surface_of_word.len()
    }

    /// Returns the surface of the word.
    #[inline(always)]
    pub fn get(&self, word_id: usize) -> &str {
        let surface = usize::from_u32(self.surface_of_word[word_id]);
        let start = match surface {
            0 => 0,
            _ => usize::from_u32(self.ends[surface - 1]),
        };
        &self.text[start..usize::from_u32(self.ends[surface])]
    }
}

struct SurfacesBuilder {
    surfaces: Surfaces,
}

impl SurfacesBuilder {
    fn new(num_words: usize) -> Self {
        Self {
            surfaces: Surfaces {
                text: String::new(),
                ends: vec![],
                surface_of_word: vec![u32::MAX; num_words],
            },
        }
    }

    /// Appends a distinct surface and returns its index.
    fn push_key(&mut self, key: &[char]) -> Result<u32> {
        let surfaces = &mut self.surfaces;
        surfaces.text.extend(key);
        surfaces.ends.push(u32::try_from(surfaces.text.len())?);
        Ok(u32::try_from(surfaces.ends.len() - 1)?)
    }

    fn assign(&mut self, word_id: u32, surface: u32) -> Result<()> {
        match self
            .surfaces
            .surface_of_word
            .get_mut(usize::from_u32(word_id))
        {
            Some(slot) if *slot == u32::MAX => {
                *slot = surface;
                Ok(())
            }
            _ => Err(VibratoError::invalid_format(
                "lexicon",
                format!("word {word_id} is out of range or has two surfaces"),
            )),
        }
    }

    fn finish(self) -> Result<Surfaces> {
        let unassigned = self
            .surfaces
            .surface_of_word
            .iter()
            .position(|&s| s == u32::MAX);
        if let Some(word_id) = unassigned {
            return Err(VibratoError::invalid_format(
                "lexicon",
                format!("word {word_id} has no surface"),
            ));
        }
        Ok(self.surfaces)
    }
}

#[derive(Default)]
pub struct WordMapBuilder {
    map: BTreeMap<String, Vec<u32>>,
}

impl WordMapBuilder {
    #[inline(always)]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline(always)]
    pub fn add_record(&mut self, word: String, id: u32) {
        self.map.entry(word).or_default().push(id);
    }

    pub fn build(self) -> Result<WordMap> {
        let mut entries = vec![];
        let mut builder = PostingsBuilder::new();
        for (word, ids) in self.map {
            let offset = builder.push(&ids)?;
            entries.push((word, u32::try_from(offset)?));
        }
        Ok(WordMap {
            trie: Trie::from_records(&entries)?,
            postings: builder.build(),
        })
    }
}

impl ArchivedWordMap {
    #[inline(always)]
    pub fn common_prefix_iterator<'a>(
        &'a self,
        input: &'a [char],
    ) -> impl Iterator<Item = (u32, usize)> + 'a {
        self.trie.common_prefix_iterator(input).flat_map(move |e| {
            self.postings
                .ids(usize::from_u32(e.value))
                .map(move |word_id| (word_id.to_native(), e.end_char))
        })
    }

    /// Recovers the surface of every word from the trie keys.
    pub fn surfaces(&self, num_words: usize) -> Result<Surfaces> {
        let mut builder = SurfacesBuilder::new(num_words);
        self.trie.for_each_record(|key, offset| {
            let surface = builder.push_key(key)?;
            for word_id in self.postings.ids(usize::from_u32(offset)) {
                builder.assign(word_id.to_native(), surface)?;
            }
            Ok(())
        })?;
        builder.finish()
    }
}

#[cfg(feature = "legacy")]
impl From<crate::legacy::dictionary::lexicon::map::WordMap> for WordMap {
    fn from(old: crate::legacy::dictionary::lexicon::map::WordMap) -> Self {
        Self {
            trie: old.trie.into(),
            postings: old.postings.into(),
        }
    }
}
