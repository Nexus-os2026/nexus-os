"""SB3E-R75 unit controls for A5's expansion checks (pure functions, no cargo): a lexer refusal is never silent, the C2
pin is an ordered list, an uncertain unit is pinned whole, and token-aware items ignore braces inside raw strings."""
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "route"))
import route_a5 as A  # noqa: E402

SHIP = "mod m {\n    fn f() {\n        let x = 1;\n    }\n    fn g() {}\n}\n"
LANE = "mod m {\n    fn f() {\n        let x = 1;\n        seam();\n    }\n    fn g() {}\n    fn h() {}\n    fn k() {}\n}\n"


class ExpandedControls(unittest.TestCase):
    def test_c_string_refusal_is_reported_not_dropped(self):
        ids, _, bad = A.idents('fn pad() -> &CStr { c"\\xff" }\nfn cell_for_test() {}\nfn end() -> &str { "" }\n')
        self.assertTrue(bad)
        self.assertNotIn("cell_for_test", ids)  # the desynchronized tokens: why a refusal must be a finding
        self.assertEqual(A.idents('fn pad() { c"x" }\nfn cell_for_test() {}\n')[2], ())

    def test_raw_string_braces_keep_the_whole_fn(self):
        text = 'mod m {\n    fn f() {\n        let _d = r#"\n    }\n}\nfn decoy() {\n"#;\n        let y = 2;\n    }\n}\n'
        blocks = A.pin_blocks(text, text.replace("y = 2", "y = 3"))
        self.assertEqual([(b[0], b[2]) for b in blocks], [("ship", "fn f ( )"), ("lane", "fn f ( )")])

    def test_uncertain_unit_is_pinned_whole(self):
        lane = LANE + "fn probe() -> P<{ 1 }> {}\n"
        self.assertEqual([b[0] for b in A.pin_blocks(SHIP, lane)], ["whole-ship", "whole-lane"])

    def test_pin_is_an_ordered_list(self):
        blocks = A.pin_blocks(SHIP, LANE)
        self.assertEqual([b[2] for b in blocks], ["fn f ( )", "fn h ( )", "fn k ( )"])
        with tempfile.TemporaryDirectory() as repo:
            os.makedirs(os.path.join(repo, A.PINS))
            path = os.path.join(repo, A.PINS, "u.pin")
            ship, lane = {("u", "s.rs", False): SHIP}, {("u", "s.rs", False): LANE}
            for order, want in ((blocks, []), ([blocks[0], blocks[2], blocks[1]], [{"check": "C2", "unit": "u", "reason": "pin text differs"}])):
                open(path, "w", encoding="utf-8").write(A.render_pin("u", "s.rs", order))
                self.assertEqual(A.parse_pin(open(path, encoding="utf-8").read()), order)
                self.assertEqual(A.c2(repo, ship, lane), want)


if __name__ == "__main__":
    unittest.main()
