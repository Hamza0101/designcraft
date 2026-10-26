//! The PostScript name of a variable font's named instance (Adobe TN #5902).
//!
//! `name` ID 6 names the font's **default** instance, so every instance of a variable font embeds
//! under the same name: all the weights of Noto Sans KR went into a PDF as `NotoSansKR-Thin`, and
//! Acrobat's font list, preflight and a print shop's checks then report Thin for every weight
//! (#1690). The outlines were right; only the name was wrong.

use skrifa::raw::TableProvider;
use skrifa::{FontRef, MetadataProvider, raw::types::NameId};

/// Longest PostScript name TN #5902 allows.
const MAX_LEN: usize = 63;

/// Axis values count as the same instance within this much (user units).
const AXIS_EPSILON: f32 = 1e-4;

/// The PostScript name for the instance of `font` at `coords`, or `None` when the font names no
/// such instance (a point in between, or a static font).
///
/// TN #5902: a named instance's own `postScriptNameID` wins; otherwise the name is the family
/// prefix (`name` ID 25, else the typographic family, else the family) joined to the instance's
/// subfamily with a hyphen, keeping only ASCII letters and digits and no more than 63 characters.
pub fn instance_postscript_name(font: &FontRef<'_>, coords: &[([u8; 4], f32)]) -> Option<String> {
    let axes = font.axes();
    let instance = font.named_instances().iter().find(|ni| {
        let mut values = axes.iter().zip(ni.user_coords());
        coords.len() == axes.len()
            && values.all(|(axis, value)| coords.iter().any(|(tag, want)| *tag == axis.tag().to_be_bytes() && (want - value).abs() <= AXIS_EPSILON))
    })?;
    if let Some(id) = instance.postscript_name_id()
        && let Some(name) = string(font, id)
    {
        let kept = keep(&name);
        if !kept.is_empty() {
            return Some(truncate(kept));
        }
    }
    let prefix = [NameId::VARIATIONS_POSTSCRIPT_NAME_PREFIX, NameId::TYPOGRAPHIC_FAMILY_NAME, NameId::FAMILY_NAME]
        .into_iter()
        .find_map(|id| string(font, id))
        .map(|s| keep(&s))
        .filter(|s| !s.is_empty())?;
    let subfamily = string(font, instance.subfamily_name_id()).map(|s| keep(&s)).filter(|s| !s.is_empty())?;
    Some(truncate(format!("{prefix}-{subfamily}")))
}

/// A `name` table string, preferring the Windows record a PDF reader looks at.
fn string(font: &FontRef<'_>, id: NameId) -> Option<String> {
    let name = font.name().ok()?;
    let records = name.name_record();
    let pick = |want: u16| records.iter().find(|r| r.name_id() == id && r.platform_id() == want);
    let record = pick(3).or_else(|| pick(1)).or_else(|| records.iter().find(|r| r.name_id() == id))?;
    let text = record.string(name.string_data()).ok()?.chars().collect::<String>();
    (!text.trim().is_empty()).then_some(text)
}

/// ASCII letters and digits of `s`, which is what a PostScript name may hold.
fn keep(s: &str) -> String {
    s.chars().filter(char::is_ascii_alphanumeric).collect()
}

/// `s` at most [`MAX_LEN`] characters, cut on a character boundary (the name is ASCII here, so
/// this is a byte cut, but the font's own string need not have been).
fn truncate(s: String) -> String {
    match s.char_indices().nth(MAX_LEN) {
        Some((at, _)) => s[..at].to_string(),
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A variable font named like Noto Sans KR: a `wght` axis and four named instances, with the
    /// family prefix in name ID 25.
    fn variable(entries: &[(u16, &str)], instances: &[(u16, Option<u16>, Vec<f32>)]) -> Vec<u8> {
        let base = crate::testing::font_with_names(entries, &['A']).expect("a font with these names");
        crate::testing::with_fvar(&base, &[(*b"wght", 100.0, 100.0, 900.0, 256)], instances).expect("an fvar")
    }

    fn at(font: &[u8], weight: f32) -> Option<String> {
        let f = skrifa::FontRef::new(font).expect("the fixture parses");
        instance_postscript_name(&f, &[(*b"wght", weight)])
    }

    /// `name` ID 6 names the default instance, so every weight embedded under it: the four
    /// weights of Noto Sans KR all went into the PDF as `NotoSansKR-Thin` (#1690).
    #[test]
    fn each_named_instance_has_its_own_postscript_name() {
        // 1 family, 6 the default instance's PostScript name, 25 the variations prefix,
        // 257.. the instances' subfamily names.
        let font = variable(
            &[(1, "Noto Sans KR"), (6, "NotoSansKR-Thin"), (25, "NotoSansKR"), (257, "Thin"), (258, "Regular"), (259, "Bold"), (260, "Black")],
            &[(257, None, vec![100.0]), (258, None, vec![400.0]), (259, None, vec![700.0]), (260, None, vec![900.0])],
        );
        assert_eq!(at(&font, 100.0).as_deref(), Some("NotoSansKR-Thin"));
        assert_eq!(at(&font, 400.0).as_deref(), Some("NotoSansKR-Regular"));
        assert_eq!(at(&font, 700.0).as_deref(), Some("NotoSansKR-Bold"));
        assert_eq!(at(&font, 900.0).as_deref(), Some("NotoSansKR-Black"));
        // A point between the named instances is not a named instance.
        assert_eq!(at(&font, 550.0), None);
    }

    /// An instance that names its own PostScript name uses it, whatever the prefix says.
    #[test]
    fn an_instances_own_postscript_name_wins() {
        let font = variable(
            &[(1, "Pretendard Variable"), (6, "PretendardVariable-Regular"), (25, "PretendardVariable"), (257, "Bold"), (300, "Pretendard-Heavy")],
            &[(257, Some(300), vec![700.0])],
        );
        assert_eq!(at(&font, 700.0).as_deref(), Some("PretendardHeavy"));
    }

    /// Without ID 25 the typographic family is the prefix, then the family; and a name a
    /// PostScript name cannot hold keeps only its letters and digits.
    #[test]
    fn the_prefix_falls_back_and_the_name_is_sanitised() {
        let typographic = variable(&[(1, "Fixture"), (16, "Fixture Display"), (257, "Semi Bold")], &[(257, None, vec![600.0])]);
        assert_eq!(at(&typographic, 600.0).as_deref(), Some("FixtureDisplay-SemiBold"));

        let family_only = variable(&[(1, "Fixture (Beta)"), (257, "Extra Light")], &[(257, None, vec![200.0])]);
        assert_eq!(at(&family_only, 200.0).as_deref(), Some("FixtureBeta-ExtraLight"));
    }

    /// TN #5902 allows 63 characters.
    #[test]
    fn a_long_name_is_cut_to_the_limit() {
        let long = "L".repeat(80);
        let font = variable(&[(1, "Fixture"), (25, &long), (257, "Regular")], &[(257, None, vec![400.0])]);
        let name = at(&font, 400.0).expect("a name");
        assert_eq!(name.len(), MAX_LEN, "cut to the limit: {name}");
        assert!(name.starts_with("LLL"));
    }

    /// A static font has no named instances, so it keeps its own `name` ID 6.
    #[test]
    fn a_static_font_has_no_instance_name() {
        let base = crate::testing::font_with_names(&[(1, "Fixture"), (6, "Fixture-Regular")], &['A']).unwrap();
        let f = skrifa::FontRef::new(&base).unwrap();
        assert_eq!(instance_postscript_name(&f, &[]), None);
    }
}
