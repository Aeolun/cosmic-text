//! Shaping a line again after an edit: [`ShapeLine::rebuild_edited`] through
//! [`BufferLine::set_text`].

use core::cell::Cell;

use crate::{
    Align, Attrs, AttrsList, BufferLine, Ellipsize, Family, FontSystem, Hinting, LineEdit,
    LineEnding, ShapeLine, Shaping, Wrap,
};

std::thread_local! {
    /// How many words [`ShapeWord::build`](crate::ShapeWord::build) shaped on this thread.
    pub(super) static WORDS_SHAPED: Cell<usize> = const { Cell::new(0) };
    /// How many lines the layout made on this thread.
    pub(super) static LINES_LAID_OUT: Cell<usize> = const { Cell::new(0) };
}

fn words_shaped() -> usize {
    WORDS_SHAPED.with(Cell::get)
}

fn lines_laid_out() -> usize {
    LINES_LAID_OUT.with(Cell::get)
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
        // Plain text gaining what makes it more than one left-to-right
        // paragraph at level 0: an embedding, an isolate, an Arabic number,
        // a paragraph separator; and losing it again.
        ["one two three", "one \u{202B}two\u{202C} three"],
        ["one two three", "one \u{2067}two\u{2069} three"],
        ["one 12 three", "one \u{0661}\u{0662} three"],
        ["one two three", "one two\u{1C}three"],
        ["one \u{202B}two\u{202C} three", "one two three"],
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
/// A small deterministic generator, so a failure names its seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

const LATIN: [&str; 12] = [
    "a",
    "an",
    "the",
    "quick",
    "brown",
    "fox",
    "jumps",
    "over",
    "lazy",
    "dog,",
    "extraordinarily",
    "supercalifragilisticexpialidocious",
];
const HEBREW: [&str; 8] = [
    "של",
    "שלום",
    "עולם",
    "טוב",
    "מאוד",
    "וגם",
    "ספר",
    "ארוךארוךארוךארוך",
];
/// Both, so a paragraph mixes directions and is laid out whole.
const MIXED: [&str; 6] = ["the", "quick", "fox,", "שלום", "עולם", "ספר"];
const SPACES: [&str; 4] = [" ", " ", " ", "  "];

fn paragraph(rng: &mut Rng, words: &[&str], count: usize) -> String {
    let mut text = String::new();
    for i in 0..count {
        if i > 0 {
            text.push_str(rng.pick(&SPACES));
        }
        text.push_str(rng.pick(words));
    }
    text
}

/// An edit at a random place: up to 30 characters replaced by nothing, a
/// word, a spaced word, or a few words.
fn edited(rng: &mut Rng, text: &str, words: &[&str]) -> String {
    let bounds: Vec<usize> = (0..=text.len())
        .filter(|&i| text.is_char_boundary(i))
        .collect();
    let start = bounds[rng.below(bounds.len())];
    let after: Vec<usize> = bounds.iter().copied().filter(|&i| i >= start).collect();
    let end = after[rng.below(after.len().min(30))];
    let insert = match rng.below(4) {
        0 => String::new(),
        1 => rng.pick(words).to_string(),
        2 => format!(" {} ", rng.pick(words)),
        _ => {
            let count = 1 + rng.below(6);
            paragraph(rng, words, count)
        }
    };
    format!("{}{}{}", &text[..start], insert, &text[end..])
}

fn layout_of(line: &mut BufferLine, fonts: &mut FontSystem, width: f32, wrap: Wrap) -> String {
    let lines = line.layout(
        fonts,
        14.0,
        Some(width),
        wrap,
        Ellipsize::None,
        None,
        4,
        Hinting::Disabled,
    );
    format!("{lines:?}")
}

#[test]
fn an_edited_line_lays_out_as_the_new_text_would() {
    let mut fonts = font_system();
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for case in 0..240 {
        let (family, words): (&str, &[&str]) = match case % 4 {
            0 | 1 => ("Noto Sans", &LATIN),
            2 => ("Noto Sans Hebrew", &HEBREW),
            _ => ("Noto Sans", &MIXED),
        };
        let wrap = [Wrap::Word, Wrap::WordOrGlyph, Wrap::Glyph][case / 4 % 3];
        let align = [
            None,
            Some(Align::Center),
            Some(Align::Right),
            Some(Align::Justified),
        ][rng.below(4)];
        let width = [40.0, 90.0, 200.0, 333.3][rng.below(4)];
        let attrs = attrs(family);
        let count = 60 + rng.below(80);
        let mut text = paragraph(&mut rng, words, count);
        let mut line = BufferLine::new(&text, LineEnding::None, attrs.clone(), Shaping::Advanced);
        line.set_align(align);
        layout_of(&mut line, &mut fonts, width, wrap);
        for step in 0..4 {
            let seed = rng.0;
            text = edited(&mut rng, &text, words);
            line.set_text(&text, LineEnding::None, attrs.clone());
            let relaid = layout_of(&mut line, &mut fonts, width, wrap);
            let mut fresh_line =
                BufferLine::new(&text, LineEnding::None, attrs.clone(), Shaping::Advanced);
            fresh_line.set_align(align);
            let fresh = layout_of(&mut fresh_line, &mut fonts, width, wrap);
            assert!(
                relaid == fresh,
                "case {case} step {step} seed {seed:#x} {wrap:?} {align:?} {width} {text:?}"
            );
        }
    }
}

#[test]
fn an_edit_to_one_word_of_a_long_paragraph_lays_out_a_few_lines() {
    let mut fonts = font_system();
    let words: Vec<String> = (0..2000).map(|i| format!("word{i}")).collect();
    let base = words.join(" ");
    let attrs = attrs("Noto Sans");
    for (edited, at_most) in [
        // The same width: the lines around it, then the old lines again.
        (base.replace("word1000 ", "word1001 "), 6),
        // A word inserted moves the rest of the paragraph's wrapping.
        (
            base.replace("word1000 ", "word1000 a much longer word "),
            usize::MAX,
        ),
    ] {
        let mut line = BufferLine::new(&base, LineEnding::None, attrs.clone(), Shaping::Advanced);
        let before = lines_laid_out();
        layout_of(&mut line, &mut fonts, 300.0, Wrap::WordOrGlyph);
        let full = lines_laid_out() - before;
        line.set_text(&edited, LineEnding::None, attrs.clone());
        let before = lines_laid_out();
        let relaid = layout_of(&mut line, &mut fonts, 300.0, Wrap::WordOrGlyph);
        let laid = lines_laid_out() - before;
        assert!(laid <= at_most, "{laid} lines laid out of {full}");
        let mut fresh =
            BufferLine::new(&edited, LineEnding::None, attrs.clone(), Shaping::Advanced);
        assert_eq!(
            relaid,
            layout_of(&mut fresh, &mut fonts, 300.0, Wrap::WordOrGlyph)
        );
    }
}
