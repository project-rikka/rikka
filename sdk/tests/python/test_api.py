import gc
import threading
import unittest
import weakref

import rikka


class Tracker:
    pass


def packed(version: str) -> int:
    major, minor, patch = (int(part) for part in version.split("."))
    return major << 22 | minor << 12 | patch


class Library(unittest.TestCase):
    def test_version_matches_binding(self) -> None:
        self.assertEqual(rikka.version(), packed(rikka.__version__))
        self.assertEqual(rikka.last_error(), "")


class Values(unittest.TestCase):
    def test_enums_reject_unknown_values(self) -> None:
        with self.assertRaises(ValueError):
            rikka.Align(99)

    def test_open_enums_hold_any_key(self) -> None:
        self.assertEqual(rikka.KeyCode("s"), ord("s"))
        self.assertEqual(rikka.KeyCode(0x1F600), 0x1F600)
        self.assertIs(rikka.KeyCode(rikka.KeyCode.ENTER.value), rikka.KeyCode.ENTER)

    def test_flags_combine(self) -> None:
        chord = rikka.Modifiers.CONTROL | rikka.Modifiers.SHIFT
        self.assertIn(rikka.Modifiers.SHIFT, chord)
        self.assertEqual(int(chord), 0x3)

    def test_none_is_an_index(self) -> None:
        self.assertEqual(rikka.NONE, -1)


class Documents(unittest.TestCase):
    def test_text_round_trips_utf8(self) -> None:
        document = rikka.TextDocument("日本語 text")
        self.assertEqual(document.text, "日本語 text")
        self.assertEqual(document.length, len("日本語 text".encode()))

        document.text = "terminated ✓\0inside"
        self.assertEqual(document.text, "terminated ✓\0inside")

        document.text = ""
        self.assertEqual(document.length, 0)
        self.assertEqual(document.text, "")

    def test_edits_undo_and_redo(self) -> None:
        document = rikka.TextDocument("first")
        self.assertFalse(document.is_modified)
        self.assertFalse(document.can_undo)

        before = document.revision
        document.text = "second"
        self.assertNotEqual(document.revision, before)
        self.assertTrue(document.is_modified)

        self.assertTrue(document.undo())
        self.assertEqual(document.text, "first")
        self.assertTrue(document.redo())
        document.mark_saved()
        self.assertFalse(document.is_modified)

        document.select_all()
        document.insert("new")
        self.assertEqual(document.text, "new")
        self.assertEqual(document.selected_text, "")

        document.reset("fresh")
        self.assertFalse(document.can_undo)
        self.assertFalse(document.undo())

    def test_strings_are_checked(self) -> None:
        with self.assertRaises(TypeError):
            rikka.TextDocument(b"bytes")  # type: ignore[arg-type]

    def test_documents_dropped_on_other_threads_are_released_on_their_own(self) -> None:
        documents = [rikka.TextDocument(f"draft {index}") for index in range(8)]
        worker = threading.Thread(target=documents.clear)
        worker.start()
        worker.join()
        gc.collect()
        self.assertEqual(rikka.TextDocument("next").text, "next")


class Builders(unittest.TestCase):
    def test_builders_release_their_closures(self) -> None:
        tracker = Tracker()
        watched = weakref.ref(tracker)
        builder = rikka.WindowBuilder(lambda view, kept=tracker: None, title="Alone", size=(320, 200))
        builder.on_start(lambda context, kept=tracker: None)
        builder.add_shortcut("s", rikka.Modifiers.CONTROL, lambda context, kept=tracker: None)
        del tracker
        gc.collect()
        self.assertIsNotNone(watched())
        del builder
        gc.collect()
        self.assertIsNone(watched())

    def test_decorators_return_their_function(self) -> None:
        builder = rikka.WindowBuilder(lambda view: None)

        @builder.add_shortcut(rikka.KeyCode("q"), rikka.Modifiers.CONTROL)
        def quit(context: rikka.Context) -> None:
            context.request(rikka.WindowRequest.CLOSE)

        @builder.on_start
        def started(context: rikka.Context) -> None:
            pass

        self.assertTrue(callable(quit))
        self.assertTrue(callable(started))

    def test_handing_over_consumes_the_builder(self) -> None:
        app = rikka.App("org.rikka.sdk.test", single_instance=False)
        window = rikka.WindowBuilder(lambda view: None)
        app.add_window(window)
        with self.assertRaises(ValueError):
            window.set_title("Again")
        with self.assertRaises(TypeError):
            app.add_window("not a window")  # type: ignore[arg-type]

    def test_classes_without_a_default_constructor_refuse_one(self) -> None:
        with self.assertRaises(TypeError):
            rikka.Image()
        with self.assertRaises(TypeError):
            rikka.Context()


class Images(unittest.TestCase):
    def test_images_are_made_three_ways(self) -> None:
        rgba = rikka.Image.from_rgba(2, 3, bytearray([0x7F] * 2 * 3 * 4))
        self.assertIsInstance(rgba, rikka.Image)
        self.assertEqual(rgba, rgba)
        self.assertIsInstance(rikka.Image.from_encoded(b"\x89PNG"), rikka.Image)
        self.assertIsInstance(rikka.Image.from_file("/nonexistent.png"), rikka.Image)


class Media(unittest.TestCase):
    def test_missing_sounds_fail_and_keep_settings(self) -> None:
        sound = rikka.Sound("/nonexistent.ogg")
        self.assertIs(sound.state, rikka.PlaybackState.FAILED)
        self.assertIn("/nonexistent.ogg", sound.error_message)
        sound.volume = 0.5
        sound.looping = True
        self.assertEqual(sound.volume, 0.5)
        self.assertTrue(sound.looping)


class Opening(unittest.TestCase):
    def test_opening_a_relative_url_raises(self) -> None:
        with self.assertRaises(rikka.Error) as raised:
            rikka.open_url("example.org")
        self.assertIs(raised.exception.status, rikka.Status.FAILED)


class Running(unittest.TestCase):
    def test_running_without_a_display_raises(self) -> None:
        tracker = Tracker()
        watched = weakref.ref(tracker)
        app = rikka.App("org.rikka.sdk.test")
        app.add_window(rikka.WindowBuilder(lambda view, kept=tracker: None, title="Never shown"))
        del tracker
        with self.assertRaises(rikka.Error) as raised:
            app.run()
        self.assertIs(raised.exception.status, rikka.Status.NO_DISPLAY)
        self.assertNotEqual(str(raised.exception), "")
        gc.collect()
        self.assertIsNone(watched())
        with self.assertRaises(ValueError):
            app.run()


if __name__ == "__main__":
    unittest.main()
