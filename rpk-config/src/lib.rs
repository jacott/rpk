use std::fmt::Write;
use std::{ops::Range, path::Path};

pub mod compiler;
pub mod globals;
pub mod keycodes;
pub mod vendor_coms;
pub mod builder;

#[derive(Debug)]
pub struct ConfigError {
    pub message: String,
    pub span: Option<Range<usize>>,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n    at: ({:?})", &self.message, &self.span)
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(err: std::io::Error) -> Self {
        Self {
            message: err.to_string(),
            span: None,
        }
    }
}

impl From<&str> for ConfigError {
    fn from(err: &str) -> Self {
        Self {
            message: err.to_string(),
            span: None,
        }
    }
}

impl ConfigError {
    pub fn new(message: String, span: Range<usize>) -> Self {
        Self {
            message,
            span: Some(span),
        }
    }

    fn char_span(&self, source: &str) -> Option<Range<usize>> {
        self.span.clone().map(|b: Range<usize>| {
            let mut s = usize::MAX;
            let mut j = 0;
            for (i, c) in source.chars().enumerate() {
                if j >= b.start {
                    if s == usize::MAX {
                        s = i;
                    }
                    if j >= b.end {
                        return Some(s..i);
                    }
                }
                j += c.len_utf8();
            }
            Some(s..j - 1)
        })?
    }

    pub fn long_format(&self, source_file: &Path, source: &str) -> String {
        let (line, col, slice) = self.line_col_slice(source);
        let width = format!("{}", line + 10).len();
        format!(
            "error: {} \n   --> {}:{}:{}\n{}",
            self.message,
            source_file.display(),
            line,
            col,
            source[slice.0..slice.1]
                .split('\n')
                .skip(1)
                .enumerate()
                .fold(String::new(), |mut output, l| {
                    let _ = writeln!(output, " {:>width$} | {}", line + l.0, l.1);
                    output
                })
        )
    }

    pub fn line_col_slice(&self, source: &str) -> (usize, usize, (usize, usize)) {
        let mut line = 1;
        let mut col = 0;
        let mut sol = 0;
        let Some(span) = self.span.clone() else {
            return (0, 0, (0, 0));
        };

        for (i, c) in source.char_indices() {
            match c {
                '\n' => {
                    if i <= span.start {
                        line += 1;
                        col = 0;
                        sol = i;
                    } else {
                        return (line, col, (sol, i));
                    }
                }
                _ if i <= span.start => {
                    col += 1;
                }
                _ => {}
            }
        }
        (line, col, (sol, source.len()))
    }
}

pub fn pretty_compile<'s>(
    file: &Path,
    src: &'s str,
) -> Result<compiler::KeyboardConfig<'s>, ConfigError> {
    match compiler::compile(src) {
        Ok(config) => Ok(config),
        Err(err) => {
            use ariadne::{ColorGenerator, Label, Report, ReportKind, Source};
            let filename = file.to_str().unwrap_or("<unknown>");
            let mut colors = ColorGenerator::new();

            let a = colors.next();
            if let Some(span) = err.char_span(src) {
                Report::build(ReportKind::Error, filename, 12)
                    .with_message("Invalid config".to_string())
                    .with_label(
                        Label::new((filename, span))
                            .with_message(&err.message)
                            .with_color(a),
                    )
                    .finish()
                    .eprint((filename, Source::from(src)))
                    .unwrap();
            }
            Err(err)
        }
    }
}

pub fn text_to_binary(source: &str) -> Result<Vec<u16>, ConfigError> {
    let file = Path::new("<unknown>");
    let config = pretty_compile(file, source)?;
    Ok(config.serialize())
}

pub fn f32_to_u16(n: f32) -> ByteToU16IntoIter<4> {
    bytes_to_u16(n.to_le_bytes())
}
pub fn bytes_to_u16<const N: usize>(bytes: [u8; N]) -> ByteToU16IntoIter<N> {
    ByteToU16IntoIter::new(bytes)
}
pub struct ByteToU16IntoIter<const N: usize>([u8; N], usize);
impl<const N: usize> ByteToU16IntoIter<N> {
    pub fn new(bytes: [u8; N]) -> Self {
        Self(bytes, 0)
    }
}
impl<const N: usize> Iterator for ByteToU16IntoIter<N> {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        let i = self.1;
        if i + 1 < self.0.len() {
            self.1 += 2;
            Some(self.0[i] as u16 | ((self.0[i + 1] as u16) << 8))
        } else {
            None
        }
    }
}

#[macro_export]
macro_rules! fixme {
    ($a:expr) => {{
        extern crate std;
        std::eprintln!(
            // split so that not found when looking for the word in an editor
            "FIXME\
             ! at ./{}:{}:{}\n{:?}",
            file!(),
            line!(),
            column!(),
            $a,
        )
    }};
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod test;
