use crate::dictionary::lexicon::{LexMatch, Lexicon, LexiconRef, WordParam};
use crate::dictionary::word_idx::WordIdx;
use crate::dictionary::{LexType, SystemDictionaryBuilder};
use crate::{Dictionary, Tokenizer};

const LEX_CSV: &str = include_str!("./resources/lex.csv");
const USER_CSV: &str = include_str!("./resources/user.csv");
const MATRIX_DEF: &str = include_str!("./resources/matrix.def");
const CHAR_DEF: &str = include_str!("./resources/char.def");
const UNK_DEF: &str = include_str!("./resources/unk.def");

/// Builds the fixture dictionary with `USER_CSV` as its own user lexicon, owned
/// and round-tripped through the archived format.
fn fixture_dictionaries() -> [Dictionary; 2] {
    let inner = SystemDictionaryBuilder::from_readers(
        LEX_CSV.as_bytes(),
        MATRIX_DEF.as_bytes(),
        CHAR_DEF.as_bytes(),
        UNK_DEF.as_bytes(),
    )
    .unwrap()
    .reset_user_lexicon_from_reader(Some(USER_CSV.as_bytes()))
    .unwrap();
    let owned = Dictionary::from_inner(inner);
    let mut bytes = vec![];
    owned.write(&mut bytes).unwrap();
    let archived = Dictionary::from_bytes(&bytes).unwrap();
    assert!(matches!(archived, Dictionary::Archived(_)));
    [owned, archived]
}

/// Collects `(word_idx, surface, feature)` for every entry of the lexicon.
fn collect_entries(lexicon: LexiconRef<'_>) -> Vec<(WordIdx, String, String)> {
    let entries = lexicon.entries().unwrap();
    entries
        .iter()
        .map(|e| {
            (
                e.word_idx(),
                e.surface().to_string(),
                e.feature().to_string(),
            )
        })
        .collect()
}

/// The rows of a lexicon CSV as `(word_idx, surface, feature)`, in row order.
fn csv_rows(csv: &str, lex_type: LexType) -> Vec<(WordIdx, String, String)> {
    Lexicon::parse_csv(csv.as_bytes(), "lex.csv")
        .unwrap()
        .into_iter()
        .enumerate()
        .map(|(i, e)| {
            let word_idx = WordIdx::new(lex_type, u32::try_from(i).unwrap());
            (word_idx, e.surface, e.feature.to_string())
        })
        .collect()
}

#[test]
fn test_entries_enumerate_every_word_once() {
    let system = csv_rows(LEX_CSV, LexType::System);
    let user = csv_rows(USER_CSV, LexType::User);
    // The fixture covers duplicate surfaces (X), surfaces that are prefixes of
    // others (東, 東京, 東京都), and a surface that differs from the first
    // feature field (アイアイウ), so the surface must come from the trie.
    assert!(
        system
            .iter()
            .any(|(_, s, f)| s == "アイアイウ" && f.starts_with("アイウ,"))
    );

    for dict in fixture_dictionaries() {
        assert_eq!(collect_entries(dict.system_lexicon()), system);
        assert_eq!(collect_entries(dict.user_lexicon().unwrap()), user);
    }
}

#[test]
fn test_entries_of_tokenizer_user_lexicon() {
    const OTHER_USER_CSV: &str = "東京タワー,6,6,-1000,固有名詞\n";
    for dict in fixture_dictionaries() {
        let tokenizer = Tokenizer::new(dict);
        assert_eq!(
            collect_entries(tokenizer.user_lexicon().unwrap()),
            csv_rows(USER_CSV, LexType::User),
        );

        // A tokenizer-owned user lexicon replaces the dictionary's.
        let tokenizer = tokenizer
            .with_user_lexicon(OTHER_USER_CSV.as_bytes())
            .unwrap();
        assert_eq!(
            collect_entries(tokenizer.user_lexicon().unwrap()),
            csv_rows(OTHER_USER_CSV, LexType::User),
        );
        assert_eq!(
            collect_entries(tokenizer.system_lexicon()),
            csv_rows(LEX_CSV, LexType::System),
        );
    }
}

#[test]
fn test_no_user_lexicon() {
    let dict = Dictionary::from_inner(
        SystemDictionaryBuilder::from_readers(
            LEX_CSV.as_bytes(),
            MATRIX_DEF.as_bytes(),
            CHAR_DEF.as_bytes(),
            UNK_DEF.as_bytes(),
        )
        .unwrap(),
    );
    assert!(dict.user_lexicon().is_none());
    assert!(Tokenizer::new(dict).user_lexicon().is_none());
}

#[test]
fn test_common_prefix_iterator_1() {
    let lexicon = Lexicon::from_reader(LEX_CSV.as_bytes(), LexType::System).unwrap();
    let input: Vec<_> = "東京都に行く".chars().collect();
    let mut it = lexicon.common_prefix_iterator(&input);
    // 東
    assert_eq!(
        it.next(),
        Some(LexMatch::new(
            WordIdx::new(LexType::System, 4),
            WordParam::new(7, 7, 4675),
            1
        ))
    );
    // 東京
    assert_eq!(
        it.next(),
        Some(LexMatch::new(
            WordIdx::new(LexType::System, 5),
            WordParam::new(6, 6, 2816),
            2
        ))
    );
    // 東京都
    assert_eq!(
        it.next(),
        Some(LexMatch::new(
            WordIdx::new(LexType::System, 6),
            WordParam::new(6, 8, 5320),
            3
        ))
    );
    assert_eq!(it.next(), None);
}

#[test]
fn test_common_prefix_iterator_2() {
    let lexicon = Lexicon::from_reader(LEX_CSV.as_bytes(), LexType::System).unwrap();
    let mut it = lexicon.common_prefix_iterator(&['X']);
    for word_id in 40..46 {
        assert_eq!(
            it.next(),
            Some(LexMatch::new(
                WordIdx::new(LexType::System, word_id),
                WordParam::new(8, 8, -20000),
                1
            ))
        );
    }
    assert_eq!(it.next(), None);
}

#[test]
fn test_get_word_feature() {
    let lexicon = Lexicon::from_reader(LEX_CSV.as_bytes(), LexType::System).unwrap();
    assert_eq!(
        lexicon.word_feature(WordIdx::new(LexType::System, 0)),
        "た,助動詞,*,*,*,助動詞-タ,終止形-一般,タ,た,*,A,*,*,*,*"
    );
    assert_eq!(
        lexicon.word_feature(WordIdx::new(LexType::System, 2)),
        "に,助詞,格助詞,*,*,*,*,ニ,に,*,A,*,*,*,*"
    );
    assert_eq!(
        lexicon.word_feature(WordIdx::new(LexType::System, 39)),
        " ,空白,*,*,*,*,*, , ,*,A,*,*,*,*"
    );
    assert_eq!(
        lexicon.word_feature(WordIdx::new(LexType::System, 45)),
        "X,名詞,固有名詞,地名,一般,*,*,X,X,*,A,*,*,*,*"
    );
}
