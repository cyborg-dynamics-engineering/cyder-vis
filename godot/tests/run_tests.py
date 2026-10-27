"""Run headless Godot checks, failing on script errors even if Godot exits zero."""
import os
from pathlib import Path
import subprocess
import sys


def main():
    project = Path(__file__).resolve().parents[1]
    executable = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("GODOT_BIN", "godot")
    # Allow deferred extension documentation to finish before the import exits.
    # https://github.com/godotengine/godot/issues/111048
    commands = [["--editor", "--import", "--frame-delay", "1000"]]
    commands += [["--script", f"res://tests/{test.name}", "--quit-after", "120"]
                 for test in sorted(project.joinpath("tests").glob("test_*.gd"))]
    commands += [["--quit-after", "10"]]
    for args in commands:
        result = subprocess.run([executable, "--headless", "--path", str(project), *args],
                                capture_output=True, text=True, encoding="utf-8", timeout=60)
        output = result.stdout + result.stderr
        if result.returncode or any(error in output for error in
                                    ["SCRIPT ERROR", "Failed to load", "GDExtension dynamic library not found"]):
            print(output)
            raise SystemExit(1)
        if "--script" in args and "tests passed" not in output:
            print(output)
            raise SystemExit("Test did not finish")
        print("PASS", " ".join(args))
    if os.name == "nt":
        subprocess.run([sys.executable, str(project / "tests/hardware_timestamps.py"), executable], check=True)


if __name__ == "__main__":
    main()
