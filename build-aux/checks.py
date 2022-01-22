#!/usr/bin/env python3
# Source: https://gitlab.gnome.org/GNOME/fractal/blob/master/hooks/pre-commit.hook

import os
import subprocess
import sys
import time
from argparse import Namespace
from pathlib import Path
from typing import List, Optional
from xml.etree import ElementTree

ERR = "\033[1;31m"
POS = "\033[32m"
NEG = "\033[31m"
ENDC = "\033[0m"

FAILED = f"{ERR}FAILED{ENDC}"
OK = f"{POS}ok{ENDC}"
ERROR = f"{NEG}error{ENDC}"


class MissingDependencyError(Exception):
    def __init__(self, whats_missing: str, install_command=None):
        self.whats_missing = whats_missing
        self.install_command = install_command

    def __str__(self):
        return f"{ERROR}: Missing dependency `{self.whats_missing}`"

    def suggestion(self) -> str:
        message = f"Please install `{self.whats_missing}` first "

        if self.install_command is not None:
            message += f"by running `{self.install_command}`"

        return message


class FailedCheckError(Exception):
    def __init__(self, error_message=None, suggestion_message=None):
        self.error_message = error_message
        self.suggestion_message = suggestion_message

    def message(self) -> Optional[str]:
        if self.error_message is not None:
            return f"{ERROR}: {self.error_message}"
        else:
            return None

    def suggestion(self) -> str:
        message = "Please fix the above issues"

        if self.suggestion_message is not None:
            message += f", {self.suggestion_message}"

        return message


class Check:
    def version(self) -> Optional[str]:
        return None

    def subject(self) -> str:
        raise NotImplementedError

    def run(self):
        raise NotImplementedError


class Rustfmt(Check):
    """Run rustfmt to enforce code style."""

    def version(self):
        return get_output(["cargo", "fmt", "--version"])

    def subject(self):
        return "code style"

    def run(self):
        if not self._does_cargo_fmt_exist():
            raise MissingDependencyError(
                "cargo fmt", install_command="rustup component add rustfmt"
            )

        if run(["cargo", "fmt", "--all", "--", "--check"]) != 0:
            raise FailedCheckError(
                suggestion_message="either manually or by running `cargo fmt --all`"
            )

    def _does_cargo_fmt_exist(self) -> bool:
        try:
            run(["cargo", "fmt", "--version"], capture_output=True)
        except FileNotFoundError:
            return False
        else:
            return True


class Typos(Check):
    """Run typos to check for spelling mistakes."""

    def version(self):
        return get_output(["typos", "--version"])

    def subject(self):
        return "spelling mistakes"

    def run(self):
        if not self._does_typos_exist():
            raise MissingDependencyError(
                "typos", install_command="cargo install typos-cli"
            )

        if run(["typos", "--color", "always"]) != 0:
            raise FailedCheckError(
                suggestion_message="either manually or by running `typos -w`"
            )

    def _does_typos_exist(self) -> bool:
        try:
            run(["typos", "--version"], capture_output=True)
        except FileNotFoundError:
            return False
        else:
            return True


class Potfiles(Check):
    """Check if files in po/POTFILES.in are correct.

    This checks, in that order:
        - All files exist
        - All files with translatable strings are present and only those
        - Files are sorted alphabetically

    This assumes the following:
        - POTFILES is located at 'po/POTFILES.in'
        - UI (Glade) files are located in 'data/resources/ui' and use 'translatable="yes"'
        - Rust files are located in 'src' and use '*gettext' methods or macros
    """

    all_potfiles: List[Path] = []

    rust_potfiles: List[Path] = []
    ui_potfiles: List[Path] = []

    def __init__(self):
        with open("po/POTFILES.in") as potfiles:
            for line in potfiles.readlines():
                file = Path(line.strip())

                self.all_potfiles.append(file)

                if file.suffix == ".ui":
                    self.ui_potfiles.append(file)
                elif file.suffix == ".rs":
                    self.rust_potfiles.append(file)

    def subject(self):
        return "po/POTFILES.in"

    def run(self):
        for file in self.all_potfiles:
            if not file.exists():
                raise FailedCheckError(error_message=f"File `{file}` does not exist")

        ui_potfiles, ui_files = self._remove_common_files(
            self.ui_potfiles, self._ui_files_with_translatable_yes()
        )
        rust_potfiles, rust_files = self._remove_common_files(
            self.rust_potfiles, self._rust_files_with_gettext()
        )

        n_potfiles = len(rust_potfiles) + len(ui_potfiles)
        if n_potfiles != 0:
            message = [
                f"Found {n_potfiles} file{'s'[:n_potfiles^1]} in POTFILES.in without translatable strings:"
            ]

            for file in rust_potfiles:
                message.append(str(file))

            for file in ui_potfiles:
                message.append(str(file))

            raise FailedCheckError(error_message="\n".join(message))

        n_files = len(rust_files) + len(ui_files)
        if n_files != 0:
            message = [
                f"Found {n_files} file{'s'[:n_potfiles^1]} with translatable strings not present in POTFILES.in:"
            ]

            for file in rust_files:
                message.append(str(file))

            for file in ui_files:
                message.append(str(file))

            raise FailedCheckError(error_message="\n".join(message))

        for file, sorted_file in zip(self.all_potfiles, sorted(self.all_potfiles)):
            if file != sorted_file:
                raise FailedCheckError(
                    error_message=f"Found file `{file}` before `{sorted_file}` in POTFILES.in"
                )

    def _remove_common_files(self, set_a: List[Path], set_b: List[Path]):
        for file_a in list(set_a):
            for file_b in list(set_b):
                if file_a == file_b:
                    set_a.remove(file_b)
                    set_b.remove(file_b)
        return set_a, set_b

    def _ui_files_with_translatable_yes(self) -> List[Path]:
        output = get_output(
            "grep -lIr 'translatable=\"yes\"' data/resources/ui/*", shell=True
        )
        return list(map(lambda s: Path(s), output.splitlines()))

    def _rust_files_with_gettext(self) -> List[Path]:
        output = get_output(r"grep -lIrE 'gettext[!]?\(' src/*", shell=True)
        return list(map(lambda s: Path(s), output.splitlines()))


class Resources(Check):
    """Check if files in data/resources/resources.gresource.xml are sorted alphabetically."""

    def subject(self):
        return "data/resources/resources.gresource.xml"

    def run(self):
        # Do not consider path suffix on sorting
        class File:
            def __init__(self, path: str):
                self.path = Path(path)

            def __str__(self):
                return self.path.__str__()

            def __lt__(self, other):
                return self.path.with_suffix("") < other.path.with_suffix("")

        tree = ElementTree.parse("data/resources/resources.gresource.xml")
        gresource = tree.find("gresource")
        files = [File(element.text) for element in gresource.findall("file")]
        sorted_files = sorted(files)

        for file, sorted_file in zip(files, sorted_files):
            if file != sorted_file:
                raise FailedCheckError(
                    error_message=f"Found file `{file}` before `{sorted_file}` in resources.gresource.xml"
                )


def run(args: List[str], **kwargs) -> int:
    process = subprocess.run(args, **kwargs)
    return process.returncode


def get_output(*args, **kwargs) -> str:
    process = subprocess.run(*args, capture_output=True, **kwargs)
    return process.stdout.decode("utf-8").strip()


def print_check(subject: str, version: Optional[str], remark: str):
    messages = ["check", subject]

    if version is not None:
        messages.append(f"({version})")

    messages.append("...")
    messages.append(remark)

    print(" ".join(messages))


def print_result(total: int, n_successful: int, duration: float):
    n_failed = total - n_successful

    if total == n_successful:
        result = OK
    else:
        result = FAILED

    print(
        f"test result: {result}. {n_successful} passed; {n_failed} failed; finished in {duration:.2f}s"
    )


def main(args: Namespace):
    checks = [
        Typos(),
        Potfiles(),
        Resources(),
    ]

    if not args.skip_rustfmt:
        checks.append(Rustfmt())

    n_checks = len(checks)

    print(f"running {n_checks} checks")

    start_time = time.time()

    successful_checks = []

    for check in checks:
        try:
            check.run()
        except FailedCheckError as e:
            remark = FAILED

            if e.message() is not None:
                print("")
                print(e.message())

            print("")
            print(e.suggestion())
            print("")
        except MissingDependencyError as e:
            remark = FAILED
            print("")
            print(e)
            print("")
            print(e.suggestion())
            print("")
        else:
            remark = OK
            successful_checks.append(check)

        print_check(check.subject(), check.version() if args.verbose else None, remark)

    check_duration = time.time() - start_time

    print("")
    print_result(n_checks, len(successful_checks), check_duration)

    if len(successful_checks) == n_checks:
        sys.exit(os.EX_OK)
    else:
        sys.exit(1)


if __name__ == "__main__":
    from argparse import ArgumentParser

    parser = ArgumentParser(
        description="Run conformity checks on the current Rust project"
    )
    parser.add_argument(
        "-v", "--verbose", action="store_true", help="Use verbose output"
    )
    parser.add_argument(
        "-s",
        "--skip-rustfmt",
        action="store_true",
        help="Whether to skip running rust fmt",
    )
    args = parser.parse_args()

    main(args)
