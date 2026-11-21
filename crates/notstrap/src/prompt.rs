//! Interactive prompting behind a port, so the wizard is testable and `--yes` is trivial.

use std::cell::RefCell;
use std::io::{BufRead, Write};

use anyhow::{Context, Result};

pub trait Prompter {
    /// Yes/no question. `default` is used on an empty answer.
    fn confirm(&self, question: &str, default: bool) -> Result<bool>;
    /// Free-text question. `default` is used on an empty answer.
    fn input(&self, question: &str, default: &str) -> Result<String>;
    /// Choose from `items`, all selected by default. Returns the chosen indices in order.
    fn multi_select(&self, question: &str, items: &[String]) -> Result<Vec<usize>>;
}

/// Answers every question with its default and selects every item. Used for `--yes`.
pub struct AssumeYes;

impl Prompter for AssumeYes {
    fn confirm(&self, _question: &str, _default: bool) -> Result<bool> {
        Ok(true)
    }

    fn input(&self, _question: &str, default: &str) -> Result<String> {
        Ok(default.to_string())
    }

    fn multi_select(&self, _question: &str, items: &[String]) -> Result<Vec<usize>> {
        Ok((0..items.len()).collect())
    }
}

/// Line-based prompter over any reader and writer.
pub struct LinePrompter<R, W> {
    reader: RefCell<R>,
    writer: RefCell<W>,
}

impl<R: BufRead, W: Write> LinePrompter<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader: RefCell::new(reader),
            writer: RefCell::new(writer),
        }
    }

    fn ask(&self, prompt: &str) -> Result<String> {
        let mut w = self.writer.borrow_mut();
        write!(w, "{prompt}")?;
        w.flush()?;
        let mut line = String::new();
        let n = self
            .reader
            .borrow_mut()
            .read_line(&mut line)
            .context("reading answer")?;
        anyhow::ensure!(n > 0, "input closed while waiting for an answer");
        Ok(line.trim().to_string())
    }
}

impl LinePrompter<std::io::BufReader<std::io::Stdin>, std::io::Stdout> {
    pub fn stdio() -> Self {
        Self::new(std::io::BufReader::new(std::io::stdin()), std::io::stdout())
    }
}

impl<R: BufRead, W: Write> Prompter for LinePrompter<R, W> {
    fn confirm(&self, question: &str, default: bool) -> Result<bool> {
        let hint = if default { "Y/n" } else { "y/N" };
        loop {
            let a = self.ask(&format!("{question} [{hint}] "))?.to_lowercase();
            match a.as_str() {
                "" => return Ok(default),
                "y" | "yes" => return Ok(true),
                "n" | "no" => return Ok(false),
                _ => writeln!(self.writer.borrow_mut(), "Please answer y or n.")?,
            }
        }
    }

    fn input(&self, question: &str, default: &str) -> Result<String> {
        let a = if default.is_empty() {
            self.ask(&format!("{question}: "))?
        } else {
            self.ask(&format!("{question} [{default}]: "))?
        };
        Ok(if a.is_empty() { default.to_string() } else { a })
    }

    fn multi_select(&self, question: &str, items: &[String]) -> Result<Vec<usize>> {
        {
            let mut w = self.writer.borrow_mut();
            writeln!(w, "{question}")?;
            for (i, item) in items.iter().enumerate() {
                writeln!(w, "  {}. {item}", i + 1)?;
            }
        }
        loop {
            let a = self.ask("Numbers, comma separated (all / none) [all]: ")?;
            match parse_selection(&a, items.len()) {
                Some(sel) => return Ok(sel),
                None => writeln!(
                    self.writer.borrow_mut(),
                    "Enter numbers between 1 and {}, or all / none.",
                    items.len()
                )?,
            }
        }
    }
}

/// Parse a selection answer into sorted, deduplicated zero-based indices.
fn parse_selection(answer: &str, len: usize) -> Option<Vec<usize>> {
    match answer.trim().to_lowercase().as_str() {
        "" | "all" => return Some((0..len).collect()),
        "none" => return Some(Vec::new()),
        _ => {}
    }
    let mut out = Vec::new();
    for part in answer.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let n: usize = part.parse().ok()?;
        if n == 0 || n > len {
            return None;
        }
        out.push(n - 1);
    }
    out.sort_unstable();
    out.dedup();
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn prompter(input: &str) -> LinePrompter<Cursor<Vec<u8>>, Vec<u8>> {
        LinePrompter::new(Cursor::new(input.as_bytes().to_vec()), Vec::new())
    }

    #[test]
    fn confirm_uses_default_on_empty_and_parses_answers() {
        assert!(prompter("\n").confirm("ok?", true).unwrap());
        assert!(!prompter("\n").confirm("ok?", false).unwrap());
        assert!(prompter("YES\n").confirm("ok?", false).unwrap());
        assert!(!prompter("n\n").confirm("ok?", true).unwrap());
    }

    #[test]
    fn confirm_reasks_on_garbage() {
        assert!(prompter("maybe\ny\n").confirm("ok?", false).unwrap());
    }

    #[test]
    fn input_uses_default_on_empty() {
        assert_eq!(prompter("\n").input("dir", "~/x").unwrap(), "~/x");
        assert_eq!(prompter("~/y\n").input("dir", "~/x").unwrap(), "~/y");
    }

    #[test]
    fn closed_input_is_an_error() {
        assert!(prompter("").confirm("ok?", true).is_err());
    }

    #[test]
    fn selection_parsing() {
        assert_eq!(parse_selection("", 3), Some(vec![0, 1, 2]));
        assert_eq!(parse_selection("all", 3), Some(vec![0, 1, 2]));
        assert_eq!(parse_selection("none", 3), Some(vec![]));
        assert_eq!(parse_selection("3, 1,1", 3), Some(vec![0, 2]));
        assert_eq!(parse_selection("0", 3), None);
        assert_eq!(parse_selection("4", 3), None);
        assert_eq!(parse_selection("x", 3), None);
    }

    #[test]
    fn multi_select_reasks_until_valid() {
        let items = vec!["a".to_string(), "b".to_string()];
        let got = prompter("9\n2\n").multi_select("pick", &items).unwrap();
        assert_eq!(got, [1]);
    }

    #[test]
    fn assume_yes_accepts_everything() {
        let p = AssumeYes;
        assert!(p.confirm("q", false).unwrap());
        assert_eq!(p.input("q", "d").unwrap(), "d");
        assert_eq!(
            p.multi_select("q", &["a".into(), "b".into()]).unwrap(),
            [0, 1]
        );
    }
}
