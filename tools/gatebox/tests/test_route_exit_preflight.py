"""SB3E-R72 D7 controls: route_exit's local-repository trust, in scratch repositories. Replace ref and clean filter are
bypass demonstrations (c8403d89's preflight passed them, Fable SHOULD 8); a dirty ignored file is a preservation control
that must also leave `.git/objects` unchanged (Fable SHOULD 9)."""
import os
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "route"))
import route_exit  # noqa: E402

GIT = ["git", "-c", "user.name=t", "-c", "user.email=t@example.invalid", "-c", "init.defaultBranch=main"]


def size(path):
    return sum(os.path.getsize(os.path.join(a, f)) for a, _, fs in os.walk(path) for f in fs)


class LocalStateControls(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = os.path.join(self.tmp.name, "r")
        self.g = lambda *a: subprocess.run(GIT + ["-C", self.repo, *a], check=True, capture_output=True, text=True).stdout.strip()
        subprocess.run(GIT + ["init", "-q", self.repo], check=True)
        for text in ("one\n", "two\n"):
            open(os.path.join(self.repo, "a.rs"), "w").write(text)
            self.g("add", "a.rs")
            self.g("commit", "-q", "-m", text)
        self.head, self.first = self.g("rev-parse", "HEAD"), self.g("rev-parse", "HEAD~1")

    def tearDown(self):
        self.tmp.cleanup()

    def trusted(self):
        return route_exit.trusted_tree(self.repo, os.path.join(self.tmp.name, "index"))

    def test_clean_repository_is_trusted(self):
        self.assertEqual(self.trusted(), ([], True, route_exit.git(self.repo, "rev-parse", "HEAD^{tree}")))

    def test_replace_ref_is_refused_and_not_followed(self):
        self.g("replace", self.head, self.first)
        self.g("checkout", "-q", self.first, "--", "a.rs")
        self.g("reset", "-q")
        # With --no-replace-objects HEAD is the real commit again, so the other commit's files are dirty too.
        self.assertEqual(self.trusted(), (["replacement refs"], False, None))
        self.g("pack-refs", "--all")
        self.assertIn("replacement refs", route_exit.local_state(self.repo))
        real = subprocess.run(["git", "--no-replace-objects", "-C", self.repo, "rev-parse", "HEAD^{tree}"],
                              capture_output=True, text=True).stdout.strip()
        self.assertEqual(route_exit.git(self.repo, "rev-parse", "HEAD^{tree}"), real)

    def test_clean_filter_is_refused(self):
        self.g("config", "filter.keep.clean", "git show HEAD:%f")
        self.assertEqual(self.trusted(), (["filter configuration"], True, None))

    def test_info_attributes_is_refused_at_the_resolved_path(self):
        path = os.path.join(self.repo, self.g("rev-parse", "--git-path", "info/attributes"))
        os.makedirs(os.path.dirname(path), exist_ok=True)
        open(path, "w").write("*.rs filter=keep\n")
        self.assertIn("info/attributes", route_exit.local_state(self.repo))

    def test_config_failure_is_not_none(self):
        open(os.path.join(self.repo, ".git", "config"), "a").write("[broken\n")
        self.assertIn("filter configuration", route_exit.local_state(self.repo))

    def test_dirty_ignored_file_fails_without_object_growth(self):
        open(os.path.join(self.repo, ".git", "info", "exclude"), "a").write("target/\n")
        os.makedirs(os.path.join(self.repo, "target"))
        open(os.path.join(self.repo, "target", "big.bin"), "wb").write(os.urandom(1 << 20))
        before = size(os.path.join(self.repo, ".git", "objects"))
        self.assertEqual(self.trusted(), ([], False, None))
        self.assertEqual(size(os.path.join(self.repo, ".git", "objects")), before)


if __name__ == "__main__":
    unittest.main()
