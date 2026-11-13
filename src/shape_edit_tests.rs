//! Shaping a line again after an edit: [`ShapeLine::rebuild_edited`] through
//! [`BufferLine::set_text`].

use core::cell::Cell;

use crate::{
    Attrs, AttrsList, BufferLine, Family, FontSystem, LineEdit, LineEnding, ShapeLine, Shaping,
};

std::thread_local! {
    /// How many words [`ShapeWord::build`](crate::ShapeWord::build) shaped on this thread.
    pub(super) static WORDS_SHAPED: Cell<usize> = const { Cell::new(0) };
}

fn words_shaped() -> usize {
    WORDS_SHAPED.with(Cell::get)
}

fn font_system() -> FontSystem {
    let mut font_system =
        FontSystem::new_with_locale_and_db("en-US".into(), fontdb::Database::new());
    let db = font_system.db_mut();
    db.load_font_data(include_bytes!("../fonts/NotoSans-Regular.ttf").to_vec());
    db.load_font_data(include_bytes!("../fonts/NotoSansHebrew.ttf").to_vec());
    db.load_font_data(include_bytes!("../fonts/NotoSansArabic.ttf").to_vec());
    db.load_font_data(include_bytes!("../fonts/FiraMono-Medium.ttf").to_vec());
    font_system
}

fn attrs(family: &str) -> AttrsList {
    AttrsList::new(&Attrs::new().family(Family::Name(family)))
}

const SHAPINGS: [Shaping; 2] = [Shaping::Advanced, Shaping::Basic];

/// Shape `steps[0]`, set each later step's text in turn, then shape: the
/// result must be what shaping the last text from nothing makes.
fn assert_rebuild_matches(font_system: &mut FontSystem, family: &str, steps: &[&str]) {
    for shaping in SHAPINGS {
        let attrs = attrs(family);
        let mut line = BufferLine::new(steps[0], LineEnding::None, attrs.clone(), shaping);
        line.shape(font_system, 4);
        for step in &steps[1..] {
            line.set_text(step, LineEnding::None, attrs.clone());
        }
        let rebuilt = format!("{:?}", line.shape(font_system, 4));
        let last = steps[steps.len() - 1];
        let fresh = format!(
            "{:?}",
            ShapeLine::new(font_system, last, &attrs, shaping, 4)
        );
        assert_eq!(rebuilt, fresh, "{shaping:?} {family} {steps:?}");
    }
}

#[test]
fn an_edited_line_shapes_as_the_new_text_would() {
    let mut fonts = font_system();
    let base = "The quick brown fox jumps over the lazy dog.";
    for edited in [
        "The quick brown fox jumps over the lazy dog!",
        "The quick brown fox jumped over the lazy dog.",
        "The quick brown fox  jumps over the lazy dog.",
        "The quick brown fx jumps over the lazy dog.",
        "The quick fox jumps over the lazy dog.",
        "A quick brown fox jumps over the lazy dog.",
        "XThe quick brown fox jumps over the lazy dog.",
        "The quick brown fox jumps over the lazy dog. And again.",
        "The quickbrown fox jumps over the lazy dog.",
        "The quick brown fox jumps over the lazy dog.   ",
        "   The quick brown fox jumps over the lazy dog.",
        "",
        "dog.",
        "The quick brown fox jumps over the lazy dog.The quick brown fox jumps over the lazy dog.",
    ] {
        assert_rebuild_matches(&mut fonts, "Noto Sans", &[base, edited]);
        assert_rebuild_matches(&mut fonts, "Noto Sans", &[edited, base]);
    }
}

#[test]
fn several_edits_before_shaping_count_as_one() {
    let mut fonts = font_system();
    assert_rebuild_matches(
        &mut fonts,
        "Noto Sans",
        &[
            "one two three four five",
            "one two 3 four five",
            "one 2 3 four five",
            "one 2 3 4 5 six",
        ],
    );
    assert_rebuild_matches(
        &mut fonts,
        "Noto Sans",
        &["alpha beta gamma", "alpha beta", "alpha beta gamma delta"],
    );
}

#[test]
fn a_tab_after_an_edit_moves_to_its_new_stop() {
    let mut fonts = font_system();
    for family in ["Noto Sans", "Fira Mono"] {
        assert_rebuild_matches(&mut fonts, family, &["a\tb\tc\td", "ab\tb\tc\td"]);
        assert_rebuild_matches(&mut fonts, family, &["a\tb\tc\td", "a\tbcdefgh\tc\td"]);
        assert_rebuild_matches(&mut fonts, family, &["a\tb\tc\td", "a\tb\t\tc\td"]);
    }
}

#[test]
fn an_edit_in_multibyte_text_splits_no_character() {
    let mut fonts = font_system();
    assert_rebuild_matches(
        &mut fonts,
        "Noto Sans",
        &["héllo wörld café", "héllo wèrld café"],
    );
    assert_rebuild_matches(
        &mut fonts,
        "Noto Sans",
        &["héllo wörld café", "héllo wörld cafè"],
    );
    assert_rebuild_matches(&mut fonts, "Noto Sans", &["ééé ééé", "éé ééé"]);
}

#[test]
fn an_edit_in_right_to_left_and_mixed_text_shapes_as_the_new_text_would() {
    let mut fonts = font_system();
    for steps in [
        ["שלום עולם טוב מאוד", "שלום עולם רע מאוד"],
        ["שלום עולם טוב מאוד", "שלום עולם טוב מאוד וגם"],
        [
            "Hello שלום world עולם again",
            "Hello שלום big world עולם again",
        ],
        [
            "Hello שלום world עולם again",
            "Hello שלום world עולמות again",
        ],
        ["مرحبا بالعالم الجميل", "مرحبا بالعالم الكبير الجميل"],
        // The direction of the line changes, so nothing is kept.
        ["abc def ghi", "שabc def ghi"],
        ["שabc def ghi", "abc def ghi"],
        // An edit that turns a run of one direction into the other.
        ["one two three", "one שתיים three"],
        ["one שתיים three", "one two three"],
    ] {
        assert_rebuild_matches(&mut fonts, "Noto Sans", &steps);
    }
}

#[test]
fn an_edit_between_punctuation_keeps_the_ligature_breaks() {
    let mut fonts = font_system();
    for family in ["Noto Sans", "Fira Mono"] {
        assert_rebuild_matches(
            &mut fonts,
            family,
            &["a != b -> c |> d", "a !== b -> c |> d"],
        );
        assert_rebuild_matches(
            &mut fonts,
            family,
            &["a != b -> c |> d", "a != b - > c |> d"],
        );
    }
}

#[test]
fn an_edit_to_one_word_of_a_long_paragraph_shapes_that_word_alone() {
    let mut fonts = font_system();
    let words: Vec<String> = (0..2000).map(|i| format!("word{i}")).collect();
    let base = words.join(" ");
    let edited = base.replace("word1000 ", "changed ");
    let inserted = base.replace("word1000 ", "word1000 new ");
    for shaping in SHAPINGS {
        for (to, shaped) in [(&edited, 1), (&inserted, 2)] {
            let attrs = attrs("Noto Sans");
            let mut line = BufferLine::new(&base, LineEnding::None, attrs.clone(), shaping);
            line.shape(&mut fonts, 4);
            line.set_text(to, LineEnding::None, attrs.clone());
            let before = words_shaped();
            line.shape(&mut fonts, 4);
            // The new word, and for the insertion the space after it.
            assert_eq!(words_shaped() - before, shaped, "{shaping:?}");
        }
    }
}

#[test]
fn a_line_whose_attributes_change_is_shaped_whole() {
    let mut fonts = font_system();
    let base = "one two three four";
    let mut line = BufferLine::new(
        base,
        LineEnding::None,
        attrs("Noto Sans"),
        Shaping::Advanced,
    );
    line.shape(&mut fonts, 4);
    line.set_text("one two 3 four", LineEnding::None, attrs("Fira Mono"));
    let before = words_shaped();
    line.shape(&mut fonts, 4);
    // Four words, three spaces, and the ellipsis shaped with the new attributes.
    assert_eq!(words_shaped() - before, 8);
}

#[test]
fn the_edit_between_two_texts_is_their_common_ends() {
    let edit = |prefix, suffix, old_len| LineEdit {
        prefix,
        suffix,
        old_len,
    };
    assert_eq!(LineEdit::between("abcdef", "abXdef"), edit(2, 3, 6));
    assert_eq!(LineEdit::between("abcdef", "abcdef!"), edit(6, 0, 6));
    assert_eq!(LineEdit::between("abcdef", "!abcdef"), edit(0, 6, 6));
    // The suffix never overlaps the prefix: "aa" to "aaa" is one "a" added.
    assert_eq!(LineEdit::between("aa", "aaa"), edit(2, 0, 2));
    assert_eq!(LineEdit::between("aaa", "aa"), edit(2, 0, 3));
    assert_eq!(LineEdit::between("", "abc"), edit(0, 0, 0));
    assert_eq!(
        LineEdit::between("abcdef", "abXdef").then(LineEdit::between("abXdef", "abXdeY")),
        edit(2, 0, 6)
    );
}
