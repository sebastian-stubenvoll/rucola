mod note;
pub use note::Note;

mod note_statistics;
pub use note_statistics::EnvironmentStats;
pub use note_statistics::SortingMode;

mod filter;
pub use filter::Filter;

mod index;
pub use index::NoteIndex;
pub use index::NoteIndexContainer;

use unicode_normalization::UnicodeNormalization;

/// Turns a file name or link into its id in the following steps:
///  - normalize the unicode characters into their composed forms
///  - everything after the first # or ., including the # or ., is ignored
///  - All characters are turned to lowercase
///  - Spaces ` ` are replaced by dashes `-`.
///  - A possible file extension is removed.
/// ```
///  assert_eq!(name_to_id("Lie Theory#Definition"), "lie-theory");
///  assert_eq!(name_to_id("Lie Theory.md"), "lie-theory");
///  assert_eq!(name_to_id("Lie Theory"), "lie-theory");
///  assert_eq!(name_to_id("lie-theory"), "lie-theory");
/// ```
pub fn name_to_id(name: &str) -> String {
    name.nfc()
        .collect::<String>()
        .split(['#', '.'])
        .take(1)
        .collect::<String>()
        .to_lowercase()
        .replace(' ', "-")
        .replace(".md", "")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_id_conversion() {
        assert_eq!(name_to_id("Lie Theory#Definition"), "lie-theory");
        assert_eq!(name_to_id("Lie Theory.md"), "lie-theory");
        assert_eq!(name_to_id("Lie Theory"), "lie-theory");
        assert_eq!(name_to_id("lie-theory"), "lie-theory");
    }

    #[test]
    fn test_id_conversion_unicode() {
        // Composed form "ö".
        let nfc_o = "\u{00F6}";
        // Decomposed form "ö".
        let nfd_o = "o\u{0308}";

        assert_ne!(nfc_o, nfd_o);
        assert_ne!(format!("K{}rper", nfc_o), format!("K{}rper", nfd_o));

        assert_eq!(name_to_id(nfc_o), name_to_id(nfd_o));
        assert_eq!(
            name_to_id(&format!("K{}rper", nfc_o)),
            name_to_id(&format!("K{}rper", nfd_o))
        );
        assert_eq!(
            name_to_id(&format!("K{}rper.md", nfc_o)),
            name_to_id(&format!("K{}rper#Definition", nfd_o))
        );
        assert_eq!(
            name_to_id(&format!("K{}rper.md", nfc_o)),
            name_to_id(&format!("k{}rper", nfd_o))
        );
    }
}
