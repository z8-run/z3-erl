use kernel::{ir::span, logic::expr, vc::vc};
use std::collections::BTreeSet;

#[test]
fn proof_identity_survives_relocation_but_not_a_changed_obligation() {
    let condition = |file: &str, line, goal| {
        vc::new(
            "Counter.step/1".into(),
            "post",
            span {
                file: file.into(),
                line,
            },
            vec![],
            goal,
            BTreeSet::new(),
        )
    };
    let first = condition("/alice/project/lib/counter.ex", 10, expr::yes());
    let moved = condition("/bob/project/lib/counter.ex", 20, expr::yes());
    let changed = condition("/bob/project/lib/counter.ex", 20, expr::no());
    assert_eq!(first.id, moved.id);
    assert_ne!(first.id, changed.id);
    assert_ne!(first.span.file, moved.span.file);
}
