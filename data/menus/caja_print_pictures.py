# "Print" in the right-click menu of Caja, for pictures only.
from gi.repository import GLib, GObject, Caja


class PrintPictures(GObject.GObject, Caja.MenuProvider):
    def get_file_items(self, window, files):
        paths = [f.get_location().get_path() for f in files]
        if not paths or None in paths or not all(f.get_mime_type().startswith("image/") for f in files):
            return []
        item = Caja.MenuItem(name="PrintPictures::print", label="Print", tip="Print these pictures")
        item.connect("activate", lambda _item: GLib.spawn_async(
            ["print-pictures", *paths], flags=GLib.SpawnFlags.SEARCH_PATH))
        return [item]
