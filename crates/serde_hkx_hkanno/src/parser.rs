use std::borrow::Cow;

use havok_types::NULL_STR;
use winnow::{
    ascii::{float, line_ending, space0, space1, till_line_ending, Caseless},
    combinator::{opt, preceded, repeat},
    ModalResult, Parser as _,
};

use crate::{Annotation, AnnotationTrack, Hkanno};

/// Error type returned when parsing hkanno text fails.
///
/// This error represents syntactic or structural issues in the input text,
/// such as unexpected tokens, malformed numbers, or incomplete lines.
///
/// It does **not** represent semantic validation errors (e.g. mismatched
/// counts), which are intentionally not enforced by this parser.
#[derive(Debug)]
pub struct HkannoParseError {
    /// Human-readable description of the parse failure.
    pub message: String,
}

impl core::error::Error for HkannoParseError {}
impl core::fmt::Display for HkannoParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Parse Error: {}", self.message)
    }
}

/// Parses an entire hkanno text document into an [`Hkanno`] structure.
///
/// This is the main entry point for parsing hkanno-formatted text.
///
/// # Behavior
///
/// - Skips all leading comment lines (`# ...`)
/// - Parses zero or more annotation tracks
/// - Borrows string data directly from `input` where possible
///
/// # Note
/// Global animation fields such as frame count and duration are currently
/// left at default values (`0` / `0.0`) and are expected to be filled
/// elsewhere if needed.
///
/// # Lifetimes
///
/// The returned [`Hkanno`] borrows from the input string. The caller must
/// ensure that `input` outlives the returned value.
///
/// # Errors
///
/// Returns [`HkannoParseError`] if the input does not conform to the expected
/// hkanno text format.
pub fn parse_hkanno_str(input: &str) -> Result<Hkanno<'_>, HkannoParseError> {
    match hkanno.parse(input) {
        Ok(h) => Ok(h),
        Err(e) => Err(HkannoParseError {
            message: e.to_string(),
        }),
    }
}

fn hkanno<'a>(input: &mut &'a str) -> ModalResult<Hkanno<'a>> {
    ignore_blank_lines.parse_next(input)?;

    let hkanno = Hkanno {
        ptr: 0,
        num_original_frames: 0,
        duration: 0.0,
        annotation_tracks: repeat(0.., track).parse_next(input)?,
    };
    ignore_blank_lines.parse_next(input)?;
    winnow::ascii::multispace0.parse_next(input)?;
    Ok(hkanno)
}

fn ignore_blank_lines(input: &mut &str) -> ModalResult<()> {
    repeat(0.., ignore_blank_line).parse_next(input)
}

fn ignore_blank_line(input: &mut &str) -> ModalResult<()> {
    winnow::seq! {
        _: space0,
        _: opt(("#", till_line_ending)),
        _: line_ending,
    }
    .map(|_| ())
    .parse_next(input)
}

/// Parses a single annotation track
fn track<'a>(input: &mut &'a str) -> ModalResult<AnnotationTrack<'a>> {
    winnow::seq! {
        AnnotationTrack {
            _: ignore_blank_lines,
            track_name: track_name_line,
            _: ignore_blank_lines,
            annotations: repeat(0.., preceded(ignore_blank_lines, annotation_line)),
            _: ignore_blank_lines,
        }
    }
    .parse_next(input)
}

/// Parses a track name line like `trackName: Example Track`
fn track_name_line<'a>(input: &mut &'a str) -> ModalResult<Option<Cow<'a, str>>> {
    let (track_name,)=  winnow::seq! {
        _: space0,
        _: Caseless("trackName"),
        _: space0,
        _: ":",
        _: space0,
        till_line_ending.map(|name: &str| { if name == NULL_STR { None } else { Some(Cow::Borrowed(name)) } }
        ),
        _: line_ending,
    }
    .parse_next(input)?;

    Ok(track_name)
}

/// Parses a single annotation line like `0.5555 annotation`
fn annotation_line<'a>(input: &mut &'a str) -> ModalResult<Annotation<'a>> {
    winnow::seq! {
        Annotation {
            _: space0,
            time: float,
            _: space1,
            text: till_line_ending.map(|name: &str| { if name == NULL_STR { None } else { Some(Cow::Borrowed(name)) } }),
            _: line_ending,
        }
    }
    .parse_next(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(input: &str) -> Hkanno<'_> {
        parse_hkanno_str(input).unwrap_or_else(|e| panic!("parse should succeed: {e}"))
    }

    fn parse_err(input: &str) {
        assert!(
            parse_hkanno_str(input).is_err(),
            "parse should fail, but succeeded"
        );
    }

    #[test]
    fn empty_input_is_ok() {
        let hkanno = parse_ok("");
        assert!(hkanno.annotation_tracks.is_empty());
    }

    #[test]
    fn comment_only_is_ok() {
        let mut input = r#"
            # this is a comment
            # another comment
            "#;
        assert!(ignore_blank_lines(&mut input).is_ok());
    }

    #[test]
    fn single_track_single_annotation() {
        let hkanno = parse_ok(
            r#"
            trackName: Test Track
            0.5 hello
            "#,
        );

        assert_eq!(hkanno.annotation_tracks.len(), 1);

        let track = &hkanno.annotation_tracks[0];
        assert_eq!(track.track_name.as_deref(), Some("Test Track"));

        assert_eq!(track.annotations.len(), 1);
        let anno = &track.annotations[0];
        assert_eq!(anno.time, 0.5);
        assert_eq!(anno.text.as_deref(), Some("hello"));
    }

    #[test]
    fn multiple_annotations_with_blank_lines() {
        let hkanno = parse_ok(
            r#"
            trackName: Track

            0.1 a

            0.2 b
            0.3 c

            "#,
        );

        let track = &hkanno.annotation_tracks[0];
        assert_eq!(track.annotations.len(), 3);
    }

    #[test]
    fn multiple_tracks() {
        let hkanno = parse_ok(
            r#"
            trackName: A
            0.1 a

            trackName: B
            0.2 b
            "#,
        );

        assert_eq!(hkanno.annotation_tracks.len(), 2);
        assert_eq!(hkanno.annotation_tracks[0].track_name.as_deref(), Some("A"));
        assert_eq!(hkanno.annotation_tracks[1].track_name.as_deref(), Some("B"));
    }

    #[test]
    fn trackname_is_case_insensitive() {
        let hkanno = parse_ok(
            r#"
            TRACKNAME: Upper
            0.0 x
            "#,
        );

        let track = &hkanno.annotation_tracks[0];
        assert_eq!(track.track_name.as_deref(), Some("Upper"));
    }

    #[test]
    fn null_string_track_name() {
        let hkanno = parse_ok(
            r#"
            trackName: ␀
            0.0 test
            "#,
        );

        let track = &hkanno.annotation_tracks[0];
        assert!(track.track_name.is_none());
    }

    #[test]
    fn null_string_annotation_text() {
        let hkanno = parse_ok(
            r#"
            trackName: T
            1.0 ␀
            "#,
        );

        let anno = &hkanno.annotation_tracks[0].annotations[0];
        assert!(anno.text.is_none());
    }

    #[test]
    fn missing_track_name_is_error() {
        parse_err(
            r#"
            0.0 orphan
            "#,
        );
    }

    #[test]
    fn malformed_float_is_error() {
        parse_err(
            r#"
            trackName: T
            abc text
            "#,
        );
    }

    #[test]
    fn annotation_without_text_is_error() {
        parse_err(
            r#"
            trackName: T
            0.5
            "#,
        );
    }
}
