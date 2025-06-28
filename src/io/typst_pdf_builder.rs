use std::{collections::VecDeque, ffi::OsString, fs, path};

use crate::{data, error};

/// Struct that keeps configuration details for the creation of HTML files from markdown files.
#[derive(Debug, Clone)]
pub struct TypstPdfBuilder {
    /// Path to the vault to index.
    vault_path: path::PathBuf,
    /// When set to true, PDF files are mass-created on start and continuously kept up to date with file changes instead of being created on-demand.
    enable_typst_pdf: bool,
    /// Typst compiler cmd
    typst_cmds: VecDeque<OsString>,
}

impl Default for TypstPdfBuilder {
    fn default() -> Self {
        Self::new(std::env::current_dir().expect("Current directory to exist and be accessible."))
    }
}

impl TypstPdfBuilder {
    pub fn new(vault_path: path::PathBuf) -> Self {
        // Obtain config from OnceLock, Config::default evaluates lazily.
        let config = crate::config::CONFIGURATION.get_or_init(crate::config::Config::default);

        Self {
            vault_path,
            enable_typst_pdf: config.enable_typst_pdf,
            typst_cmds: config
                .typst_cmds
                .iter()
                .map(|c| OsString::from(&c))
                .collect(),
        }
    }

    pub fn create_typst_pdf(&self, note: &data::Note, force: bool) -> error::Result<()> {
        if !self.enable_typst_pdf && !force {
            return Ok(());
        }

        // Only process typst code.
        if note
            .path
            .extension()
            .is_some_and(|ext| ext.to_str() != Some("typ"))
        {
            return Ok(());
        }

        let tar_path = Self::name_to_pdf_path(&note.name, &self.vault_path);

        // ensure parent exists
        if let Some(parent) = tar_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        let mut cmd_buffer = self.typst_cmds.clone();
        let cmd = cmd_buffer.pop_front();
        let __ = std::process::Command::new(
            // Explicitly panic if vec is empty!
            cmd.expect("Compliation command to be provided."),
        )
        .args(cmd_buffer.iter())
        .arg(note.path.clone())
        .arg(tar_path)
        .spawn()?;

        Ok(())
    }

    /// For a given note id, returns the path its HTML representation _would_ be stored at.
    /// Makes no guarantees if that representation currently exists.
    pub fn name_to_pdf_path(name: &str, vault_path: &path::Path) -> path::PathBuf {
        // calculate target path
        let mut tar_path = vault_path.to_path_buf();
        tar_path.push(".pdf/");

        tar_path.set_file_name(format!(".pdf/{}", &data::name_to_id(name)));
        tar_path.set_extension("pdf");
        tar_path
    }
}

#[cfg(test)]
mod tests {

    use std::path::{Path, PathBuf};

    #[test]
    fn test_create_html_no_panic() {
        let pb = super::TypstPdfBuilder::new(PathBuf::from("./tests"));

        let os = crate::data::Note::from_path(Path::new("./tests/common/notes/Birds.typ")).unwrap();

        pb.create_typst_pdf(&os, true).unwrap();
    }

    #[test]
    fn test_name_to_html_path() {
        // let config = crate::Config::default();
        let vault_path = PathBuf::from("./tests");

        assert_eq!(
            super::TypstPdfBuilder::name_to_pdf_path("Birds", &vault_path),
            PathBuf::from("./tests/.pdf/birds.pdf")
        );
        assert_eq!(
            super::TypstPdfBuilder::name_to_pdf_path("birds", &vault_path),
            PathBuf::from("./tests/.pdf/birds.pdf")
        );
    }

    #[test]
    fn test_create_html_creates_files() {
        let vault_path = PathBuf::from("./tests");
        let b_path = super::TypstPdfBuilder::name_to_pdf_path("birds", &vault_path);
        let pb = super::TypstPdfBuilder::new(vault_path);

        let birds =
            crate::data::Note::from_path(Path::new("./tests/common/notes/Birds.typ")).unwrap();

        if b_path.exists() {
            std::fs::remove_file(&b_path).unwrap();
        }

        // assert!(!b_path.exists());

        pb.create_typst_pdf(&birds, true).unwrap();

        assert!(b_path.exists());
    }
}
