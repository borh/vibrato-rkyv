use rkyv::{Archive, Deserialize, Serialize};

use crate::errors::{Result, VibratoError};
use crate::utils::FromU32;

#[derive(Archive, Serialize, Deserialize)]
pub struct Trie {
    da: crawdad_rkyv::Trie,
}

impl Trie {
    pub fn from_records<K>(records: &[(K, u32)]) -> Result<Self>
    where
        K: AsRef<str>,
    {
        Ok(Self {
            da: crawdad_rkyv::Trie::from_records(records.iter().map(|(k, v)| (k, *v)))
                .map_err(|e| VibratoError::invalid_argument("records", e.to_string()))?,
        })
    }

    #[inline(always)]
    pub fn common_prefix_iterator<'a>(
        &'a self,
        input: &'a [char],
    ) -> impl Iterator<Item = TrieMatch> + 'a {
        self.da
            .common_prefix_search(input.iter().cloned())
            .map(move |(value, end_char)| TrieMatch::new(value, end_char))
    }

    /// Calls `f` once for every key in the trie with the key and its value.
    pub fn for_each_record<F>(&self, f: F) -> Result<()>
    where
        F: FnMut(&[char], u32) -> Result<()>,
    {
        for_each_serialized_record(&self.da.serialize_to_vec(), f)
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct TrieMatch {
    pub value: u32,
    pub end_char: usize,
}

impl TrieMatch {
    #[inline(always)]
    pub const fn new(value: u32, end_char: usize) -> Self {
        Self { value, end_char }
    }
}

impl ArchivedTrie {
    #[inline(always)]
    pub fn common_prefix_iterator<'a>(
        &'a self,
        input: &'a [char],
    ) -> impl Iterator<Item = TrieMatch> + 'a {
        self.da
            .common_prefix_search(input.iter().cloned())
            .map(move |(value, end_char)| TrieMatch::new(value, end_char))
    }

    /// Calls `f` once for every key in the trie with the key and its value.
    pub fn for_each_record<F>(&self, f: F) -> Result<()>
    where
        F: FnMut(&[char], u32) -> Result<()>,
    {
        // crawdad-rkyv exposes neither its nodes nor key enumeration, so this
        // decodes its public byte format, the one the legacy conversion reads.
        let da: crawdad_rkyv::Trie = rkyv::deserialize::<_, rkyv::rancor::Error>(&self.da)?;
        for_each_serialized_record(&da.serialize_to_vec(), f)
    }
}

/// Mask of the offset bits of a crawdad node; the top bit is a flag.
const OFFSET_MASK: u32 = 0x7fff_ffff;
/// Code that crawdad assigns to its end marker, which is not part of any key.
const END_CODE: u32 = 0;

/// Recovers every key of a crawdad double array from the bytes of
/// `crawdad_rkyv::Trie::serialize_to_vec`.
///
/// The layout is the code-mapper table length, the table of codes indexed by
/// character, the alphabet size, the node count, and each node as a `base`,
/// `check` pair, all little-endian `u32`. A node whose `base` has the top bit
/// set is a leaf whose `base` holds the value. Each non-root node's `check` is
/// its parent, and the label of the edge into it is the parent's `base` XOR
/// its own index. A key that is a prefix of another ends in an extra edge
/// labelled with the end code.
fn for_each_serialized_record<F>(bytes: &[u8], mut f: F) -> Result<()>
where
    F: FnMut(&[char], u32) -> Result<()>,
{
    let malformed = || VibratoError::invalid_format("trie", "inconsistent double array");
    let read = |pos: usize| -> Result<u32> {
        let word = bytes.get(pos..pos + 4).ok_or_else(malformed)?;
        Ok(u32::from_le_bytes(word.try_into().unwrap()))
    };

    let table_len = usize::from_u32(read(0)?);
    let alphabet_size = usize::from_u32(read(4 + table_len * 4)?);
    let mut labels = vec![None; alphabet_size];
    for c in 0..table_len {
        let code = read(4 + c * 4)?;
        if code != u32::MAX {
            let label = labels
                .get_mut(usize::from_u32(code))
                .ok_or_else(malformed)?;
            *label = char::from_u32(u32::try_from(c)?);
        }
    }

    let num_nodes_pos = 4 + table_len * 4 + 4;
    let num_nodes = read(num_nodes_pos)?;
    let nodes = &bytes[num_nodes_pos + 4..];
    if nodes.len() != usize::from_u32(num_nodes) * 8 {
        return Err(malformed());
    }
    let node = |idx: u32| -> (u32, u32) {
        let pos = usize::from_u32(idx) * 8;
        let base = u32::from_le_bytes(nodes[pos..pos + 4].try_into().unwrap());
        let check = u32::from_le_bytes(nodes[pos + 4..pos + 8].try_into().unwrap());
        (base, check)
    };

    let mut key = vec![];
    for idx in 1..num_nodes {
        let (base, _) = node(idx);
        if base & !OFFSET_MASK == 0 {
            continue;
        }
        key.clear();
        let mut child = idx;
        while child != 0 {
            let parent = node(child).1 & OFFSET_MASK;
            if num_nodes <= parent || key.len() >= usize::from_u32(num_nodes) {
                return Err(malformed());
            }
            let parent_base = node(parent).0;
            if parent_base & !OFFSET_MASK != 0 {
                return Err(malformed());
            }
            let code = parent_base ^ child;
            if code == END_CODE {
                if child != idx {
                    return Err(malformed());
                }
            } else {
                let label = labels.get(usize::from_u32(code)).copied().flatten();
                key.push(label.ok_or_else(malformed)?);
            }
            child = parent;
        }
        key.reverse();
        f(&key, base & OFFSET_MASK)?;
    }
    Ok(())
}

#[cfg(feature = "legacy")]
impl From<crate::legacy::dictionary::lexicon::map::trie::Trie> for Trie {
    fn from(old: crate::legacy::dictionary::lexicon::map::trie::Trie) -> Self {
        Self {
            da: crawdad_rkyv::Trie::deserialize_from_slice(&old.da.serialize_to_vec()).0,
        }
    }
}
