#[test]
fn proxy_evidence_cannot_be_assembled_or_settled_twice() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/proxy_parts.rs");
    cases.compile_fail("tests/ui/proxy_receipt.rs");
    cases.compile_fail("tests/ui/proxy_double_settlement.rs");
}
