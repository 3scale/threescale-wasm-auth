#[cfg(any(feature = "json_config", not(feature = "yaml_config")))]
pub mod json;
#[cfg(all(
    feature = "yaml_config",
    feature = "danger",
    not(feature = "json_config")
))]
pub mod yaml;

#[derive(Debug, thiserror::Error)]
pub enum ErrorLocationError {
    #[error("could not extract location from document")]
    NoLocationAvailable,
}

pub struct ErrorLocation<'e, E> {
    err: &'e E,
    line: usize,
    column: usize,
    kind: Option<String>,
}

impl<'e, E: std::error::Error + std::fmt::Display> ErrorLocation<'e, E> {
    fn error(&self) -> &E {
        self.err
    }

    fn line(&self) -> usize {
        self.line
    }

    fn column(&self) -> usize {
        self.column
    }

    fn kind(&self) -> Option<&str> {
        self.kind.as_deref()
    }

    pub fn new(
        err: &'e E,
        line: usize,
        column: usize,
        kind: Option<&str>,
    ) -> Result<Self, ErrorLocationError> {
        match (line, column) {
            (0, _) | (_, 0) => Err(ErrorLocationError::NoLocationAvailable),
            (line, column) => Ok(ErrorLocation {
                err,
                line,
                column,
                kind: kind.map(|s| format!("({})", s)),
            }),
        }
    }

    pub fn error_lines<'i>(
        &self,
        input: &'i str,
        before_ctx: usize,
        after_ctx: usize,
    ) -> impl Iterator<Item = String> + 'i {
        let line = self.line();
        // this is not a parsing error (ie. programmatic)
        assert_ne!(line, 0);
        let column = self.column();
        let line_skip = line.saturating_sub(before_ctx.saturating_add(1));
        // before_len also takes the error line
        let before_len = line - line_skip;
        let numchars = |mut num: usize| {
            let mut chars: usize = 1;
            while num > 9 {
                num /= 10;
                chars += 1;
            }
            chars
        };
        let last_line = (0..=after_ctx)
            .rev()
            .find_map(|after| line.checked_add(after))
            .unwrap_or(line);
        let after_len = last_line - line;
        let lineno_width = numchars(last_line);
        let format_line = move |(current_line, line)| {
            format!(
                "{:>width$}: {}",
                current_line + line_skip + 1,
                line,
                width = lineno_width
            )
        };
        let before_it = input.lines().skip(line_skip);
        let after_it = before_it
            .clone()
            .enumerate()
            .skip(before_len)
            .take(after_len)
            .map(format_line);

        before_it
            .enumerate()
            .take(before_len)
            .map(format_line)
            .chain(core::iter::once(format!(
                "{: >width$}  {: >columns$} error {} {}",
                "",
                "^",
                self.kind().unwrap_or(""),
                self.error(),
                width = lineno_width,
                columns = column
            )))
            .chain(after_it)
    }

    #[cfg(test)]
    pub fn error_to_string(&self, input: &str) -> String {
        self.error_lines(input, 2, 2).collect::<Vec<_>>().join("\n")
    }
}
