"""Loads the Files (Nautilus) and Caja extensions with a stand-in for gi, and
checks when they offer "Print" and exactly what they would run."""

import importlib.util
import sys
import types
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).parent
AWKWARD = ['/p/-a "b" 100% Ü.jpg', "/p/c d.png"]


class Item:
    def __init__(self, **properties):
        self.properties = properties
        self.activate = None

    def connect(self, signal, handler):
        assert signal == "activate"
        self.activate = handler


class Glib:
    class SpawnFlags:
        SEARCH_PATH = 4

    def __init__(self):
        self.calls = []

    def spawn_async(self, argv, flags=0):
        self.calls.append((argv, flags))


class File:
    def __init__(self, path, mime="image/jpeg"):
        self.path, self.mime = path, mime

    def get_location(self):
        return types.SimpleNamespace(get_path=lambda: self.path)

    def get_mime_type(self):
        return self.mime


def load(filename, toolkit):
    glib = Glib()
    repository = types.ModuleType("gi.repository")
    repository.GLib = glib
    repository.GObject = types.SimpleNamespace(GObject=type("GObject", (), {}))
    manager = types.SimpleNamespace(MenuProvider=type("MenuProvider", (), {}), MenuItem=Item)
    setattr(repository, toolkit, manager)
    gi = types.ModuleType("gi")
    gi.repository = repository
    sys.modules["gi"], sys.modules["gi.repository"] = gi, repository
    spec = importlib.util.spec_from_file_location(filename[:-3], HERE / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.PrintPictures(), glib


# (file, toolkit, arguments that come before the list of files)
EXTENSIONS = [
    ("nautilus_print_pictures.py", "Nautilus", ()),
    ("caja_print_pictures.py", "Caja", ("window",)),
]


class Menus(unittest.TestCase):
    def test_print_runs_the_program_with_every_path_intact(self):
        for filename, toolkit, before in EXTENSIONS:
            with self.subTest(toolkit):
                provider, glib = load(filename, toolkit)
                files = [File(AWKWARD[0]), File(AWKWARD[1], "image/png")]
                [item] = provider.get_file_items(*before, files)
                self.assertEqual(item.properties["label"], "Print")
                item.activate(item)
                self.assertEqual(glib.calls, [(["print-pictures", *AWKWARD], 4)])

    def test_nothing_is_offered_unless_all_are_local_pictures(self):
        selections = {
            "nothing": [],
            "a text file": [File("/p/a.jpg"), File("/p/b.txt", "text/plain")],
            "no local path": [File("/p/a.jpg"), File(None)],
        }
        for filename, toolkit, before in EXTENSIONS:
            for name, files in selections.items():
                with self.subTest(f"{toolkit}: {name}"):
                    provider, _ = load(filename, toolkit)
                    self.assertEqual(provider.get_file_items(*before, files), [])


if __name__ == "__main__":
    unittest.main()
