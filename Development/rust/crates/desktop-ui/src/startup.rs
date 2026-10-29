use std::{ffi::OsString, path::PathBuf};

pub fn paths(arguments: impl IntoIterator<Item = OsString>) -> Vec<PathBuf> {
    let mut arguments = arguments.into_iter().peekable();
    if arguments
        .peek()
        .is_some_and(|a| a == "--verify-desktop" || a == "--verify-media")
    {
        return Vec::new();
    }
    let mut literal = false;
    arguments
        .filter_map(|argument| {
            if !literal && argument == "--" {
                literal = true;
                None
            } else if !literal && argument.to_string_lossy().starts_with("--") {
                None
            } else {
                let path = PathBuf::from(argument);
                Some(std::path::absolute(&path).unwrap_or(path))
            }
        })
        .collect()
}

pub fn is_project(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("sveproject"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_preserves_unicode_spaces_order_and_excludes_verification_arguments() {
        let parse = |args: &[&str]| paths(args.iter().map(OsString::from));
        assert_eq!(
            parse(&["--software-preview", "C:/録画/source one.mp4", "D:/two.mkv"]),
            vec![
                PathBuf::from("C:/録画/source one.mp4"),
                PathBuf::from("D:/two.mkv")
            ]
        );
        assert!(parse(&["--verify-desktop", "source.mp4", "report"]).is_empty());
        assert!(parse(&["--verify-media", "source.mp4"]).is_empty());
        assert_eq!(
            parse(&["--", "--recording.mp4"]),
            vec![std::env::current_dir().unwrap().join("--recording.mp4")]
        );
        assert_eq!(
            parse(&["local recording.mp4"]),
            vec![std::env::current_dir().unwrap().join("local recording.mp4")]
        );
        assert!(is_project(std::path::Path::new("edit.SVEPROJECT")));
    }
}
