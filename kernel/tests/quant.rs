use kernel::logic::{expr as e, sort};
use std::collections::BTreeMap;

#[test]
fn substitution_cannot_capture_an_inserted_free_variable() {
    let x = e::var("x", sort::term);
    let y = e::var("y", sort::term);
    let formula = e::quantify(true, vec![("x".into(), sort::term)], e::eq(x.clone(), y));
    let changed = formula.replace(&BTreeMap::from([("y".into(), x)]));
    let mut free = BTreeMap::new();
    changed.symbols(&mut free, &mut BTreeMap::new());
    assert_eq!(free, BTreeMap::from([("x".into(), sort::term)]));
    assert!(matches!(changed,e::quant {vars,..} if vars[0].0 != "x"));
}
