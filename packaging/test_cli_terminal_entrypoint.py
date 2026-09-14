"""Portable controls for the opt-in Windows terminal runner's import boundary."""
from pathlib import Path
import subprocess
import sys
import unittest


SCRIPT = Path(__file__).with_name("test_cli_workflow_terminal.py")


class TerminalEntrypointTests(unittest.TestCase):
    def test_import_needs_no_arguments_or_terminal_dependencies(self):
        # A clean subprocess also prevents previously imported optional modules
        # from hiding a regression. Do not start a terminal or create fixtures.
        control = """
import importlib.abc
import runpy
import sys
class NoTerminalDependencies(importlib.abc.MetaPathFinder):
    def find_spec(self, fullname, path=None, target=None):
        if fullname.split('.')[0] in {'winpty', 'pyte'}:
            raise AssertionError('terminal dependency imported during discovery')
sys.meta_path.insert(0, NoTerminalDependencies())
script = sys.argv[1]
sys.argv = ['unittest']
module = runpy.run_path(script, run_name='discovery_import')
assert callable(module['main'])
"""
        result = subprocess.run(
            [sys.executable, "-I", "-c", control, str(SCRIPT)],
            capture_output=True, text=True, timeout=15,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "")

    def test_help_and_missing_binary_are_portable(self):
        for arguments, expected in [(["--help"], 0), ([], 2)]:
            with self.subTest(arguments=arguments):
                result = subprocess.run(
                    [sys.executable, "-I", str(SCRIPT), *arguments],
                    capture_output=True, text=True, timeout=15,
                )
                self.assertEqual(result.returncode, expected, result.stderr)
                self.assertIn("--binary", result.stdout + result.stderr)
                self.assertNotIn("ModuleNotFoundError", result.stderr)


if __name__ == "__main__":
    unittest.main()
