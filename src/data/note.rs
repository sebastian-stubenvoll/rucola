use ratatui::{prelude::*, widgets::*};
use std::{fs, path};
use typst_syntax::{
    ast::{self, AstNode},
    SyntaxKind, SyntaxNode,
};

use itertools::Itertools;

use crate::{error, ui};

// ZSTs for the various filetypes
struct MarkdownFile;
struct TypstFile;

// Parser trait
trait ToNote {
    fn to_note(path: &path::Path) -> error::Result<Note>;
}

impl ToNote for MarkdownFile {
    fn to_note(path: &path::Path) -> error::Result<Note> {
        // Open the file.
        let content = fs::read_to_string(path)?;
        // Create a regex to check for YAML front matter.
        let regex = regex::Regex::new("---\n((.|\n)*)\n---\n((.|\n)*)")?;

        // Extract both the YAML front matter, if present, and the main content.
        let (yaml, content) = Note::extract_yaml(regex, content);

        // Parse markdown into AST
        let arena = comrak::Arena::new();
        let root = comrak::parse_document(
            &arena,
            &content,
            &comrak::Options {
                extension: comrak::ExtensionOptions::builder()
                    .wikilinks_title_after_pipe(true)
                    .build(),
                ..Default::default()
            },
        );

        // Parse YAML to obtain title and tags.
        let (title, tags) = Note::parse_yaml(yaml)?;

        Ok(Note {
            // Name: Check if there was one specified in the YAML fronmatter.
            // If not, remove file extension.
            display_name: title.unwrap_or(
                path.file_stem()
                    .map(|os| os.to_string_lossy().to_string())
                    .ok_or_else(|| error::RucolaError::NoteNameCannotBeRead(path.to_path_buf()))?,
            ),
            // File name: Remove file extension.
            name: path
                .file_stem()
                .map(|os| os.to_string_lossy().to_string())
                .ok_or_else(|| error::RucolaError::NoteNameCannotBeRead(path.to_path_buf()))?,
            // Path: Already given - convert to owned version.
            path: path.canonicalize().unwrap_or(path.to_path_buf()),
            // Tags: Go though all text nodes in the AST, split them at whitespace and look for those starting with a hash.
            // Finally, append tags specified in the YAML frontmatter.
            tags: root
                .descendants()
                .flat_map(|node| match &node.data.borrow().value {
                    comrak::nodes::NodeValue::Text(content) => content
                        .split_whitespace()
                        .filter(|s| s.starts_with('#'))
                        .map(|s| s.to_owned())
                        .collect_vec(),
                    _ => vec![],
                })
                .chain(tags)
                .collect(),
            // Links: Go though all wikilinks in the syntax tree and map them
            links: root
                .descendants()
                .flat_map(|node| match &node.data.borrow().value {
                    comrak::nodes::NodeValue::WikiLink(link) => Some(super::name_to_id(&link.url)),
                    comrak::nodes::NodeValue::Link(link) => {
                        if !link.url.contains('/') && !link.url.contains('.') {
                            Some(super::name_to_id(&link.url))
                        } else {
                            None
                        }
                    }
                    _ => None,
                })
                .collect(),
            // Words: Split at whitespace, grouping multiple consecutive instances of whitespace together.
            // See definition of `split_whitespace` for criteria.
            words: content.split_whitespace().count(),
            // Characters: Simply use the length of the string.
            characters: content.len(),
        })
    }
}

impl ToNote for TypstFile {
    fn to_note(path: &path::Path) -> error::Result<Note> {
        // Open the file.
        let content = fs::read_to_string(path)?;
        // Create a regex to check for YAML front matter.
        // This assumes the yaml frontmatter is enclosed in a block comment.
        let regex = regex::Regex::new("/\\*\n---\n((.|\n)*)\n---\n\\*/((.|\n)*)")?;

        // Extract both the YAML front matter, if present, and the main content.
        let (yaml, content) = Note::extract_yaml(regex, content);

        // Parse YAML to obtain title and tags.
        let (title, mut tags) = Note::parse_yaml(yaml)?;

        // Parse typst into syntax tree
        let root = typst_syntax::parse(content.as_str());

        // Define recursive function for traversing the tree.
        // I don't belive we can skip any nodes?
        // Any String or Expression could hold a FuncCall.

        let mut links: Vec<String> = Vec::new();

        // Load config to obtain tpyst function identifiers to look for.
        // get_or_init uses a closure under the hood, so this should be evaulated lazily.
        let config = crate::config::CONFIGURATION.get_or_init(crate::config::Config::default);

        // Mutable references can be dropped here.
        let _ = TypstFile::traverse_tree(
            &root,
            &mut links,
            &mut tags,
            &config.link_function,
            &config.tag_function,
        );

        Ok(Note {
            // Name: Check if there was one specified in the YAML fronmatter.
            // If not, remove file extension.
            display_name: title.unwrap_or(
                path.file_stem()
                    .map(|os| os.to_string_lossy().to_string())
                    .ok_or_else(|| error::RucolaError::NoteNameCannotBeRead(path.to_path_buf()))?,
            ),
            // File name: Remove file extension.
            name: path
                .file_stem()
                .map(|os| os.to_string_lossy().to_string())
                .ok_or_else(|| error::RucolaError::NoteNameCannotBeRead(path.to_path_buf()))?,
            // Path: Already given - convert to owned version.
            path: path.canonicalize().unwrap_or(path.to_path_buf()),
            // Tags: Go though all text nodes in the AST, split them at whitespace and look for those starting with a hash.
            // Finally, append tags specified in the YAML frontmatter.
            // TODO: get tags from tag function!
            tags,
            links: links
                .iter()
                // Extract filename without extension.
                .filter_map(|l| path::Path::new(l).file_stem())
                // Conver OsStr to the owned String type via str
                .filter_map(|s| s.to_str())
                .map(|s| s.to_string())
                .collect(),

            // Words: Split at whitespace, grouping multiple consecutive instances of whitespace together.
            // See definition of `split_whitespace` for criteria.
            words: content.split_whitespace().count(),
            // Characters: Simply use the length of the string.
            characters: content.len(),
        })
    }
}

impl TypstFile {
    // Helper functions for extracting information from the syntax tree.
    fn traverse_tree<'a>(
        node: &'a SyntaxNode,
        mut links: &'a mut Vec<String>,
        mut tags: &'a mut Vec<String>,
        link_ident: &String,
        tag_ident: &String,
    ) -> (&'a mut Vec<String>, &'a mut Vec<String>) {
        // Recursively traverse all nodes.
        for child in node.children() {
            // Inspect function call closer.
            if child.kind() == SyntaxKind::FuncCall {
                // TODO: Use setting for ident here!
                if let Some(link) = TypstFile::look_ahead(child, link_ident) {
                    links.push(link);
                } else if let Some(mut tag) = TypstFile::look_ahead(child, tag_ident) {
                    if !tag.starts_with("#") {
                        tag.insert(0, '#');
                    }
                    tags.push(tag);
                }
            }
            // traverse_tree must return its mutable references...
            (links, tags) = TypstFile::traverse_tree(child, links, tags, link_ident, tag_ident);
        }
        // ...and does so here.
        (links, tags)
    }

    fn look_ahead(node: &SyntaxNode, ident: &str) -> Option<String> {
        // Check if the FuncCall has a child that is the inditifier for the link function.
        if node.cast_first_match::<ast::Ident>()?.as_str() == ident {
            return Some(
                // Per definition (see TYPST_README.md) the first argument must be the link target.
                node.cast_first_match::<ast::Args>()?
                    .to_untyped()
                    .cast_first_match::<ast::Str>()?
                    .get()
                    .to_string(),
            );
        }
        None
    }
}

/// An abstract representation of a note that contains statistics about it but _not_ the full text.
#[derive(Clone, Debug, Default)]
pub struct Note {
    /// The title of the note.
    pub display_name: String,
    /// The name of the file the note is saved in.
    pub name: String,
    /// All tags contained at any part of the note.
    pub tags: Vec<String>,
    /// All links contained within the note - no external (e.g. web) links.
    pub links: Vec<String>,
    /// The number of words.
    pub words: usize,
    /// The number of characters.
    pub characters: usize,
    /// A copy of the path leading to this note.
    pub path: path::PathBuf,
}

impl Note {
    /// Opens the file from the given path (if possible) and extracts metadata.
    pub fn from_path(path: &path::Path) -> error::Result<Self> {
        // Check filetype and create the corresponding note struct
        let note = match path
            .extension()
            .ok_or(error::RucolaError::UnhandledFiletype)?
            .to_str()
        {
            Some("md") => MarkdownFile::to_note(path),
            Some("typ") => TypstFile::to_note(path),
            // Fallback to markdown file if no ext could be determined.
            _ => MarkdownFile::to_note(path),
        };
        note
    }

    /// Converts this note to a small ratatui table displaying its most vital stats.
    pub fn to_stats_table(&self, styles: &ui::UiStyles) -> Table {
        let stats_widths = [
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Min(20),
        ];

        // Display the note's tags
        let tags = self
            .tags
            .iter()
            .enumerate()
            .flat_map(|(index, s)| {
                [
                    Span::styled(if index == 0 { "" } else { ", " }, styles.text_style),
                    Span::styled(s.as_str(), styles.subtitle_style),
                ]
            })
            .collect_vec();

        // Stats Area
        let stats_rows = [
            Row::new(vec![
                Cell::from("Words:").style(styles.text_style),
                Cell::from(format!("{:7}", self.words)).style(styles.text_style),
                Cell::from("Tags:").style(styles.text_style),
                Cell::from(Line::from(tags)).style(styles.text_style),
            ]),
            Row::new(vec![
                Cell::from("Chars:").style(styles.text_style),
                Cell::from(format!("{:7}", self.characters)).style(styles.text_style),
                Cell::from("Path:").style(styles.text_style),
                Cell::from(self.path.to_str().unwrap_or_default()).style(styles.text_style),
            ]),
        ];

        Table::new(stats_rows, stats_widths).column_spacing(1)
    }

    fn extract_yaml(regex: regex::Regex, content: String) -> (Option<String>, String) {
        let extracted = if let Some(matches) = regex.captures(&content) {
            // If the regex matched, YAML front matter was present.
            (
                // The 1st capture group is the front matter.
                matches.get(1).map(|m| m.as_str().to_owned()),
                // The 3rd capture group is the actual content.
                matches.get(3).unwrap().as_str().to_owned(),
            )
        } else {
            // If the regex didn't match, then just use the content.
            (None, content)
        };
        extracted
    }

    fn parse_yaml(yaml: Option<String>) -> error::Result<(Option<String>, Vec<String>)> {
        // Parse YAML.
        let (title, tags) = if let Some(yaml) = yaml {
            let docs = yaml_rust::YamlLoader::load_from_str(&yaml)?;
            let doc = &docs[0];

            // Check if there was a title specified.
            let title = doc["title"].as_str().map(|s| s.to_owned());

            // Check if tags were specified.
            let tags = doc["tags"]
                // Convert the entry into a vec - if the entry isn't there, use an empty vec.
                .as_vec()
                .unwrap_or(&Vec::new())
                .iter()
                // Convert the individual entries into strs, as rust-yaml doesn't do nested lists.
                .flat_map(|v| v.as_str())
                // Convert those into Strings and prepend the #.
                .flat_map(|s| {
                    // Entries of sublists will appear as separated by ` - `, so split by that.
                    let parts = s.split(" - ").collect_vec();

                    if parts.is_empty() {
                        // This should not happen.
                        Vec::new()
                    } else if parts.len() == 1 {
                        // Only one parts => There were not subtags. Simply prepend a `#`.
                        vec![format!("#{}", s)]
                    } else {
                        // More than 1 part => There were subtags.
                        let mut res = Vec::new();

                        // Iterate through all of the substrings except for the first, which is the supertag.
                        for subtag in parts.iter().skip(1) {
                            res.push(format!("#{}/{}", parts[0], subtag));
                        }

                        res
                    }
                })
                // Collect all tags in a vec.
                .collect_vec();

            (title, tags)
        } else {
            (None, Vec::new())
        };
        Ok((title, tags))
    }
}

#[cfg(test)]
mod tests {

    use super::ToNote;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_md_loading() {
        let _note =
            super::MarkdownFile::to_note(Path::new("./tests/common/notes/Books.md")).unwrap();
    }

    #[test]
    fn test_typ_loading() {
        let _note = super::TypstFile::to_note(Path::new("./tests/common/notes/Birds.typ")).unwrap();
    }

    #[test]
    fn test_values_md() {
        let note =
            crate::data::Note::from_path(Path::new("./tests/common/notes/math/Chart.md")).unwrap();

        assert_eq!(note.name, String::from("Chart"));
        assert_eq!(
            note.tags,
            vec![String::from("#diffgeo"), String::from("#topology")]
        );
        assert_eq!(
            note.links,
            vec![String::from("manifold"), String::from("diffeomorphism")]
        );
        assert_eq!(note.words, 115);
        assert_eq!(note.characters, 678);
        assert_eq!(
            note.path,
            PathBuf::from("./tests/common/notes/math/Chart.md")
                .canonicalize()
                .unwrap()
        );
    }

    #[test]
    fn test_values_typ() {
        let note =
            crate::data::Note::from_path(Path::new("./tests/common/notes/Links.typ")).unwrap();

        assert_eq!(note.name, String::from("Links"));
        assert_eq!(
            note.tags,
            vec![String::from("#link"), String::from("#typst")]
        );
        assert_eq!(
            note.links,
            vec![String::from("Birds"), String::from("Warbler")]
        );
        assert_eq!(note.words, 25);
        assert_eq!(note.characters, 230);
        assert_eq!(
            note.path,
            PathBuf::from("./tests/common/notes/Links.typ")
                .canonicalize()
                .unwrap()
        );
    }

    #[test]
    fn test_yaml_name_md() {
        let note =
            crate::data::Note::from_path(Path::new("./tests/common/notes/note25.md")).unwrap();

        assert_eq!(note.display_name, String::from("YAML Format"));
        assert_eq!(note.name, String::from("note25"));

        assert_eq!(
            note.path,
            PathBuf::from("./tests/common/notes/note25.md")
                .canonicalize()
                .unwrap()
        );
    }

    #[test]
    fn test_yaml_name_typ() {
        let note =
            crate::data::Note::from_path(Path::new("./tests/common/notes/Warbler.typ")).unwrap();

        assert_eq!(note.display_name, String::from("YAML Format in .typ files"));
        assert_eq!(note.name, String::from("Warbler"));

        assert_eq!(
            note.path,
            PathBuf::from("./tests/common/notes/Warbler.typ")
                .canonicalize()
                .unwrap()
        );
    }

    #[test]
    fn test_yaml_tags_md() {
        let note =
            crate::data::Note::from_path(Path::new("./tests/common/notes/note25.md")).unwrap();

        assert_eq!(
            note.tags,
            vec![
                String::from("#test"),
                String::from("#files/yaml"),
                String::from("#files/markdown"),
                String::from("#abbreviations")
            ]
        );
    }

    #[test]
    fn test_yaml_tags_typ() {
        let note =
            crate::data::Note::from_path(Path::new("./tests/common/notes/Warbler.typ")).unwrap();

        assert_eq!(
            note.tags,
            vec![
                String::from("#animals/birds"),
                String::from("#animals/america"),
                String::from("#biology"),
                String::from("#warblers")
            ]
        );
    }
}
