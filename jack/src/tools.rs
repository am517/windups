//! Local programs Jack may use when they are installed. A missing tool is not a failure.
use std::io::ErrorKind;
use std::process::{Command, Stdio};

pub fn installed(program: &str) -> bool {
    !matches!(
        Command::new(program)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status(),
        Err(error) if error.kind() == ErrorKind::NotFound
    )
}

pub fn missing(program: &str) -> String {
    format!("{program} is not installed, so I could not read metadata with it")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_program_is_reported_in_plain_words() {
        assert!(!installed("jack-there-is-no-such-tool"));
        assert_eq!(
            missing("yt-dlp"),
            "yt-dlp is not installed, so I could not read metadata with it"
        );
    }
}
