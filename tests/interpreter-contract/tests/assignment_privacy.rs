#[test]
fn assignment_evidence_cannot_be_assembled_or_settled_twice() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/assignment_receipt.rs");
    cases.compile_fail("tests/ui/assignment_parts.rs");
    cases.compile_fail("tests/ui/assignment_double_settlement.rs");
}
