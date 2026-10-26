"""FileChooser portal: GDS single-file picker with GTK protocol fallback."""

import asyncio
import json
import logging
import os
import signal
import sys
from urllib.parse import quote_from_bytes

from dbus_next import Message, MessageType, Variant
from dbus_next.aio import MessageBus
from dbus_next.constants import NameFlag, PropertyAccess, RequestNameReply
from dbus_next.service import ServiceInterface, dbus_property, method

BUS_NAME = "org.freedesktop.impl.portal.desktop.gds"
GTK_NAME = "org.freedesktop.impl.portal.desktop.gtk"
OBJECT_PATH = "/org/freedesktop/portal/desktop"
CHOOSER = "org.freedesktop.impl.portal.FileChooser"
REQUEST = "org.freedesktop.impl.portal.Request"


def option(options, name, default=None):
    return options[name].value if name in options else default


def picker_arguments(options):
    """Return CLI arguments and original filters, or None for GTK fallback.

    MIME filters and patterns containing CLI separators cannot be represented
    losslessly by the current picker. Preserve them through GTK instead.
    """
    directory_mode = option(options, "directory", False)
    if option(options, "choices", []):
        return None
    filters = [] if directory_mode else list(option(options, "filters", []))
    current = None if directory_mode else option(options, "current_filter")
    if current:
        filters = [current] + [item for item in filters if item != current]
    arguments = ["--pick-directory" if directory_mode else "--pick-file"]
    if option(options, "multiple", False):
        arguments.append("--multiple")
    for label, patterns in filters:
        if not patterns or ":" in label:
            return None
        globs = []
        for kind, pattern in patterns:
            # globset adds brace alternation that XDG globs do not specify.
            if (kind != 0 or not pattern or pattern != pattern.strip()
                    or any(c in pattern for c in ":,{}")):
                return None
            globs.append(pattern)
        arguments.extend(["--filter", f"{label}:{','.join(globs)}"])
    folder = option(options, "current_folder")
    if folder:
        directory = os.fsdecode(bytes(folder).rstrip(b"\0"))
        if os.path.isdir(directory):
            arguments.extend(["--", directory])
    return arguments, filters


def selection_results(output, filters, directory=False, multiple=False):
    """Decode the picker JSON, preserving non-UTF-8 Unix paths in file URIs."""
    payload = json.loads(output)
    selections = payload.get("selections", [payload])
    if not selections or (not multiple and len(selections) != 1):
        raise ValueError("picker returned an invalid selection count")
    uris = []
    for selection in selections:
        path = selection.get("path")
        raw = os.fsencode(path) if path is not None else bytes(selection["path_os"]["Unix"])
        valid_type = os.path.isdir(raw) if directory else os.path.isfile(raw)
        if not os.path.isabs(raw) or not valid_type:
            raise ValueError("picker returned a missing path or the wrong file type")
        uris.append("file://" + quote_from_bytes(raw, safe="/"))
    results = {
        "uris": Variant("as", uris),
        "writable": Variant("b", all(not item["read_only"] for item in selections)),
    }
    if filters:
        index = selections[0]["filter_index"]
        if not isinstance(index, int) or not 0 <= index < len(filters):
            raise ValueError("picker returned an invalid filter index")
        results["current_filter"] = Variant("(sa(us))", filters[index])
    return results


class Request(ServiceInterface):
    def __init__(self, task):
        super().__init__(REQUEST)
        self.task = task

    @method()
    def Close(self):
        self.task.cancel()


class FileChooser(ServiceInterface):
    def __init__(self, bus, executable):
        super().__init__(CHOOSER)
        self.bus = bus
        self.executable = executable
        self.tasks = set()

    @dbus_property(access=PropertyAccess.READ)
    def version(self) -> 'u':
        return 4

    async def gtk(self, member, handle, app_id, parent_window, title, options):
        try:
            reply = await self.bus.call(Message(
                destination=GTK_NAME, path=OBJECT_PATH, interface=CHOOSER,
                member=member, signature="osssa{sv}",
                body=[handle, app_id, parent_window, title, options],
            ))
            if reply.message_type == MessageType.ERROR:
                raise RuntimeError(f"GTK {member}: {reply.error_name}: {reply.body}")
            return reply.body
        except asyncio.CancelledError:
            reply = await self.bus.call(Message(
                destination=GTK_NAME, path=handle, interface=REQUEST, member="Close",
            ))
            if reply.message_type == MessageType.ERROR:
                logging.warning("GTK request close failed: %s", reply.body)
            raise

    async def pick(self, arguments, filters, directory=False, multiple=False):
        process = None
        try:
            process = await asyncio.create_subprocess_exec(
                self.executable, *arguments, stdout=asyncio.subprocess.PIPE,
            )
            output, _ = await process.communicate()
            if process.returncode == 1:
                return [1, {}]
            if process.returncode != 0:
                raise RuntimeError(f"gds-files exited with status {process.returncode}")
            return [0, selection_results(output, filters, directory, multiple)]
        finally:
            if process is not None and process.returncode is None:
                process.terminate()
                try:
                    await asyncio.wait_for(process.wait(), timeout=3)
                except asyncio.TimeoutError:
                    process.kill()
                    await process.wait()

    async def dispatch(self, member, handle, app_id, parent_window, title, options):
        task = asyncio.current_task()
        request = Request(task)
        self.tasks.add(task)
        self.bus.export(handle, request)
        try:
            translated = picker_arguments(options) if member == "OpenFile" else None
            if translated is not None:
                try:
                    return await self.pick(*translated,
                        directory=option(options, "directory", False),
                        multiple=option(options, "multiple", False))
                except (OSError, ValueError, KeyError, TypeError, RuntimeError):
                    logging.exception("GDS picker failed; falling back to GTK")
            return await self.gtk(member, handle, app_id, parent_window, title, options)
        except asyncio.CancelledError:
            return [1, {}]
        except Exception:
            logging.exception("FileChooser request failed")
            return [2, {}]
        finally:
            self.bus.unexport(handle, request)
            self.tasks.discard(task)

    @method()
    async def OpenFile(self, handle: 'o', app_id: 's', parent_window: 's',
                       title: 's', options: 'a{sv}') -> 'ua{sv}':
        return await self.dispatch("OpenFile", handle, app_id, parent_window, title, options)

    @method()
    async def SaveFile(self, handle: 'o', app_id: 's', parent_window: 's',
                       title: 's', options: 'a{sv}') -> 'ua{sv}':
        return await self.dispatch("SaveFile", handle, app_id, parent_window, title, options)

    @method()
    async def SaveFiles(self, handle: 'o', app_id: 's', parent_window: 's',
                        title: 's', options: 'a{sv}') -> 'ua{sv}':
        return await self.dispatch("SaveFiles", handle, app_id, parent_window, title, options)


async def main():
    bus = await MessageBus().connect()
    chooser = FileChooser(bus, sys.argv[1])
    bus.export(OBJECT_PATH, chooser)
    result = await bus.request_name(BUS_NAME, NameFlag.DO_NOT_QUEUE)
    if result != RequestNameReply.PRIMARY_OWNER:
        raise RuntimeError(f"{BUS_NAME} is already running")
    stopped = asyncio.Event()
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(sig, stopped.set)
    try:
        await stopped.wait()
    finally:
        tasks = list(chooser.tasks)
        for task in tasks:
            task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        bus.disconnect()


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    asyncio.run(main())
