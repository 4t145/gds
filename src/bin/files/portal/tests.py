"""Run with dbus-run-session -- python src/bin/files/portal/tests.py."""

import asyncio
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from portal import (BUS_NAME, CHOOSER, GTK_NAME, OBJECT_PATH, REQUEST,
                    FileChooser, picker_arguments, selection_results)
from dbus_next import Message, MessageType, Variant
from dbus_next.aio import MessageBus


class TranslationTests(unittest.TestCase):
    def test_current_filter_and_non_utf8_folder(self):
        with tempfile.TemporaryDirectory() as root:
            folder = os.fsencode(root) + b"/\xff"
            os.mkdir(folder)
            first, current = ["Text", [[0, "*.txt"]]], ["Code", [[0, "*.rs"]]]
            args, filters = picker_arguments({
                "filters": Variant("a(sa(us))", [first, current]),
                "current_filter": Variant("(sa(us))", current),
                "current_folder": Variant("ay", folder + b"\0"),
            })
            self.assertEqual(filters, [current, first])
            self.assertEqual(args[:5], ["--pick-file", "--filter", "Code:*.rs",
                                       "--filter", "Text:*.txt"])
            self.assertEqual(os.fsencode(args[-1]), folder)

    def test_unsupported_requests_fall_back_without_changing_options(self):
        for options in (
            {"choices": Variant("a(ssa(ss)s)", [["id", "Choice", [], "true"]])},
            {"filters": Variant("a(sa(us))", [["Image", [[1, "image/png"]]]])},
            {"filters": Variant("a(sa(us))", [["Name", [[0, "a,b"]]]])},
        ):
            with self.subTest(options=options):
                self.assertIsNone(picker_arguments(options))

    def test_raw_path_uri_and_selected_filter(self):
        with tempfile.TemporaryDirectory() as root:
            raw = os.fsencode(root) + b"/a #\xff.txt"
            Path(os.fsdecode(raw)).touch()
            filters = [["All", [[0, "*"]]], ["Text", [[0, "*.txt"]]]]
            results = selection_results(json.dumps({
                "path": None, "path_os": {"Unix": list(raw)},
                "read_only": True, "filter_index": 1,
            }), filters)
            self.assertTrue(results["uris"].value[0].endswith("/a%20%23%FF.txt"))
            self.assertFalse(results["writable"].value)
            self.assertEqual(results["current_filter"].value, filters[1])


class RecordingGtk(FileChooser):
    def __init__(self, bus):
        super().__init__(bus, "unused")
        self.calls = []

    async def dispatch(self, member, *args):
        self.calls.append((member, args))
        return [0, {"uris": Variant("as", ["file:///gtk-result"])}]


class PortalTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.executable = self.root / "picker"
        self.file = self.root / "a file.txt"
        self.file.touch()
        self.bus = await MessageBus().connect()
        self.client = await MessageBus().connect()
        self.gtk_bus = await MessageBus().connect()
        self.chooser = FileChooser(self.bus, str(self.executable))
        self.gtk = RecordingGtk(self.gtk_bus)
        self.bus.export(OBJECT_PATH, self.chooser)
        self.gtk_bus.export(OBJECT_PATH, self.gtk)
        await self.bus.request_name(BUS_NAME)
        await self.gtk_bus.request_name(GTK_NAME)

    async def asyncTearDown(self):
        tasks = list(self.chooser.tasks)
        for task in tasks:
            task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        for bus in (self.client, self.bus, self.gtk_bus):
            bus.disconnect()
            await bus.wait_for_disconnect()
        self.temp.cleanup()

    def script(self, body):
        self.executable.write_text(f"#!{sys.executable}\n{body}\n")
        self.executable.chmod(0o755)

    async def call(self, member="OpenFile", options=None, suffix="one"):
        return await asyncio.wait_for(self.client.call(Message(
            destination=BUS_NAME, path=OBJECT_PATH, interface=CHOOSER,
            member=member, signature="osssa{sv}",
            body=[f"{OBJECT_PATH}/request/test/{suffix}", "test.app", "", "Pick", options or {}],
        )), timeout=10)

    async def test_single_file_via_dbus(self):
        self.script("import json\nprint(json.dumps(" + repr({
            "path": str(self.file), "read_only": False, "filter_index": 0,
        }) + "))")
        reply = await self.call()
        self.assertEqual(reply.message_type, MessageType.METHOD_RETURN)
        self.assertEqual(reply.body[0], 0)
        self.assertTrue(reply.body[1]["uris"].value[0].endswith("/a%20file.txt"))
        self.assertTrue(reply.body[1]["writable"].value)
        self.assertEqual(self.gtk.calls, [])

    async def test_backend_version(self):
        reply = await self.client.call(Message(
            destination=BUS_NAME, path=OBJECT_PATH,
            interface="org.freedesktop.DBus.Properties", member="Get",
            signature="ss", body=[CHOOSER, "version"],
        ))
        self.assertEqual(reply.body[0], Variant("u", 4))

    async def test_directory_request_including_zed_multiple_mode(self):
        for multiple in (False, True):
            paths = [self.root] if not multiple else [self.root, self.root / "another folder"]
            if multiple:
                paths[1].mkdir()
            selections = [{"path": str(path), "read_only": False, "filter_index": 0} for path in paths]
            payload = selections[0] if not multiple else {"selections": selections}
            self.script("import json, sys\nassert '--pick-directory' in sys.argv\n"
                        + ("assert '--multiple' in sys.argv\n" if multiple else "")
                        + "print(json.dumps(" + repr(payload) + "))")
            reply = await self.call(options={"directory": Variant("b", True), "multiple": Variant("b", multiple)})
            self.assertEqual(reply.body[0], 0)
            self.assertEqual(len(reply.body[1]["uris"].value), len(paths))
            self.assertEqual(reply.body[1]["uris"].value[0], self.root.as_uri())
            self.assertEqual(self.gtk.calls, [])

    async def test_directory_result_rejects_a_regular_file(self):
        self.script("import json\nprint(json.dumps(" + repr({
            "path": str(self.file), "read_only": False, "filter_index": 0,
        }) + "))")
        reply = await self.call(options={"directory": Variant("b", True)})
        self.assertEqual(reply.body[1]["uris"].value, ["file:///gtk-result"])

    async def test_fallback_preserves_protocol(self):
        options = {"choices": Variant("a(ssa(ss)s)", [["id", "Choice", [], "true"]]), "accept_label": Variant("s", "Use")}
        for member in ("OpenFile", "SaveFile", "SaveFiles"):
            reply = await self.call(member, options)
            self.assertEqual(reply.body[1]["uris"].value, ["file:///gtk-result"])
            self.assertEqual(self.gtk.calls[-1][0], member)
            self.assertEqual(self.gtk.calls[-1][1][-1], options)

    async def test_multiple_files_via_dbus(self):
        second = self.root / "second.txt"
        second.touch()
        payload = {"selections": [
            {"path": str(path), "read_only": False, "filter_index": 0}
            for path in [self.file, second]
        ]}
        self.script("import json, sys\nassert '--pick-file' in sys.argv\nassert '--multiple' in sys.argv\nprint(json.dumps(" + repr(payload) + "))")
        reply = await self.call(options={"multiple": Variant("b", True)})
        self.assertEqual(reply.body[0], 0)
        self.assertEqual(reply.body[1]["uris"].value, [self.file.as_uri(), second.as_uri()])
        self.assertEqual(self.gtk.calls, [])

    async def test_cancel_does_not_open_gtk_but_failure_does(self):
        self.script("raise SystemExit(1)")
        self.assertEqual((await self.call()).body, [1, {}])
        self.assertEqual(self.gtk.calls, [])
        self.script("print('invalid-json')")
        self.assertEqual((await self.call()).body[0], 0)
        self.assertEqual(len(self.gtk.calls), 1)

    async def test_close_terminates_only_its_own_picker(self):
        folders = [self.root / "first", self.root / "second"]
        for folder in folders:
            folder.mkdir()
        self.script("import os, sys, time\nopen(sys.argv[-1] + '/pid', 'w').write(str(os.getpid()))\ntime.sleep(30)")
        pending, other = [asyncio.create_task(self.call(
            options={"current_folder": Variant("ay", os.fsencode(folder) + b"\0")},
            suffix=suffix,
        )) for folder, suffix in zip(folders, ("cancel", "keep"))]
        markers = [folder / "pid" for folder in folders]
        for _ in range(200):
            if all(marker.exists() and marker.read_text() for marker in markers):
                break
            await asyncio.sleep(0.01)
        pid, other_pid = [int(marker.read_text()) for marker in markers]
        reply = await self.client.call(Message(
            destination=BUS_NAME, path=f"{OBJECT_PATH}/request/test/cancel",
            interface=REQUEST, member="Close",
        ))
        self.assertEqual(reply.message_type, MessageType.METHOD_RETURN)
        self.assertEqual((await pending).body, [1, {}])
        with self.assertRaises(ProcessLookupError):
            os.kill(pid, 0)
        self.assertFalse(other.done())
        os.kill(other_pid, 0)
        await self.client.call(Message(
            destination=BUS_NAME, path=f"{OBJECT_PATH}/request/test/keep",
            interface=REQUEST, member="Close",
        ))
        self.assertEqual((await other).body, [1, {}])
        self.assertEqual(self.gtk.calls, [])


if __name__ == "__main__":
    unittest.main()
