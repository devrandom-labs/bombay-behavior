"""Regression cases for assertion-side effect detection."""

import unittest

from scripts.check_assertion_effects import violations


class AssertionEffectsTests(unittest.TestCase):
    def test_mutating_helper_inside_nested_assertion_is_rejected(self):
        source = "assert!(matches!(release(&mut actor), Ok(_)));"
        self.assertEqual(list(violations(source)), [1])

    def test_consuming_method_inside_equality_is_rejected(self):
        source = "assert_eq!(items.into_iter().collect::<Vec<_>>(), [1]);"
        self.assertEqual(list(violations(source)), [1])

    def test_required_call_before_assertion_is_accepted(self):
        source = "let result = release(&mut actor);\nassert_eq!(result.sends.len(), 1);"
        self.assertEqual(list(violations(source)), [])

    def test_temporary_mutable_source_comparison_is_accepted(self):
        source = "assert_eq!(source, &mut Source(3));"
        self.assertEqual(list(violations(source)), [])

    def test_parenthesis_character_does_not_hide_a_later_call(self):
        source = "assert_eq!('(', items.into_iter().next().unwrap());"
        self.assertEqual(list(violations(source)), [1])


if __name__ == "__main__":
    unittest.main()
