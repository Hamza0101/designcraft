use super::{ChapterBatch, MAX_BATCH_CHAPTERS, MAX_BOOK_CHAPTER_BYTES, MAX_BOOK_PDF_BYTES};
use crate::Session;
use serde_json::json;

#[test]
fn a_commit_failure_reports_the_completed_prefix_and_cleans_staging() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.designcraft");
    let second = dir.path().join("second.designcraft");
    let third = dir.path().join("third.designcraft");
    let mut s = Session::new();
    s.execute("file.new", &json!({})).unwrap();
    let old = (*s.doc().unwrap().doc).clone();
    let mut changed = old.clone();
    changed.sections.first_mut().unwrap().start_number = Some(9);
    let old_bytes = designcraft_format::save(&old).unwrap();
    for path in [&first, &second, &third] {
        std::fs::write(path, &old_bytes).unwrap();
    }
    let mut batch = ChapterBatch::new(3, "book.paginate").unwrap();
    for path in [&first, &second, &third] {
        batch.stage(path.to_str().unwrap(), &changed).unwrap();
    }
    let staging = batch.directory.path().to_path_buf();
    let mut calls = 0;
    let error = batch
        .commit_with(|path, staged| {
            calls += 1;
            if path == second { Err(std::io::Error::other("synthetic disk full")) } else { crate::io::replace_with(path, staged) }
        })
        .unwrap_err()
        .to_string();
    assert_eq!(calls, 2);
    assert!(error.contains("synthetic disk full"), "{error}");
    assert!(error.contains(second.to_str().unwrap()), "{error}");
    let completed = error.split("completed chapter paths:").nth(1).unwrap();
    // The completed list uses debug formatting, which escapes Windows path separators.
    assert_eq!(completed.trim(), format!("{:?}", [first.to_str().unwrap()]), "{error}");
    assert_eq!(designcraft_format::load(&std::fs::read(first).unwrap()).unwrap().sections.first().unwrap().start_number, Some(9));
    assert_eq!(std::fs::read(second).unwrap(), old_bytes);
    assert_eq!(std::fs::read(third).unwrap(), old_bytes);
    assert!(!staging.exists(), "temporary chapter snapshots must be removed on a partial failure");
}

#[test]
fn chapter_record_limit_is_checked_before_staging_allocation() {
    let error = match ChapterBatch::new(MAX_BATCH_CHAPTERS + 1, "book.paginate") {
        Err(e) => e,
        Ok(_) => panic!("oversized book batch accepted"),
    };
    assert!(error.to_string().contains("4096"));
}

/// The export's budget is what the chapters may add up to, and the peak is three times it: the
/// chapters, the merged output, and the copy the merge makes of the chapter it is reading. If
/// the chapters' budget is ever raised, the figure it is a third of has to move with it, or the
/// export silently goes back to reaching three times what it allows (noted in review of #1597).
#[test]
fn the_chapter_budget_leaves_room_for_the_merge_it_pays_for() {
    assert!(MAX_BOOK_CHAPTER_BYTES * 3 <= MAX_BOOK_PDF_BYTES, "three times the budget is within the peak");
    assert!((MAX_BOOK_CHAPTER_BYTES + 1) * 3 > MAX_BOOK_PDF_BYTES, "and it is as much as a third allows, not less");
    assert_eq!(MAX_BOOK_CHAPTER_BYTES / (1 << 20), 170, "and the refusal names 170 MB");
}
