"""Windows protocol-v1 mock: jitter delivery while hardware ticks stay at 1 ms.

Runs the actual extension in Godot, including trace and raw/decoded MF4 capture.
Uses only Python's standard library and never opens or transmits on a CAN bus.
"""
import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import threading
import time


def run_case(executable, driver, capability, enabled, epoch):
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.CreateNamedPipeW.argtypes = [wintypes.LPCWSTR] + [wintypes.DWORD] * 6 + [wintypes.LPVOID]
    kernel.CreateNamedPipeW.restype = wintypes.HANDLE
    kernel.ConnectNamedPipe.argtypes = [wintypes.HANDLE, wintypes.LPVOID]
    kernel.ConnectNamedPipe.restype = wintypes.BOOL
    for operation in [kernel.ReadFile, kernel.WriteFile]:
        operation.argtypes = [wintypes.HANDLE, wintypes.LPVOID, wintypes.DWORD,
                              ctypes.POINTER(wintypes.DWORD), wintypes.LPVOID]
        operation.restype = wintypes.BOOL
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL
    kernel.CancelIoEx.argtypes = [wintypes.HANDLE, wintypes.LPVOID]
    kernel.CancelIoEx.restype = wintypes.BOOL
    name = f"cyder_clock_{os.getpid()}"
    handle = kernel.CreateNamedPipeW(r"\\.\pipe\wincan_" + name + "_v1", 3, 0, 1, 65536, 65536, 0, None)
    if handle == ctypes.c_void_p(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    failures = []

    def read_exact(count):
        result = bytearray()
        while len(result) < count:
            buffer = ctypes.create_string_buffer(count - len(result))
            transferred = wintypes.DWORD()
            if not kernel.ReadFile(handle, buffer, len(buffer), ctypes.byref(transferred), None):
                if ctypes.get_last_error() == 109:
                    raise EOFError
                raise ctypes.WinError(ctypes.get_last_error())
            if not transferred.value:
                raise EOFError
            result.extend(buffer.raw[:transferred.value])
        return bytes(result)

    def write(message):
        data = json.dumps(dict(version=1, **message)).encode("utf-8")
        data = struct.pack("<I", len(data)) + data
        while data:
            buffer = ctypes.create_string_buffer(data)
            transferred = wintypes.DWORD()
            if not kernel.WriteFile(handle, buffer, len(data), ctypes.byref(transferred), None):
                raise ctypes.WinError(ctypes.get_last_error())
            if not transferred.value:
                raise EOFError
            data = data[transferred.value:]

    with tempfile.TemporaryDirectory(prefix="cyder hardware clock ") as directory:
        ready = Path(directory) / "ready"
        status = dict(name=name, driver=driver, device="timestamp-mock", bitrate=500000,
                      desired_up=True, state="up", bus_state="error_active", timestamps=enabled,
                      capabilities=dict(hardware_can_fd=False, can_fd=False, hardware_timestamps=capability,
                                        bus_state=False, software_tx_echo=False),
                      counters=dict(rx_frames=0, tx_frames=0, rx_errors=0, tx_errors=0,
                                    client_drops=0, disconnects=0, bus_off_events=0), active_clients=1, last_error=None,
                      last_rx_timestamp_us=None, hardware_rx_error_counter=None, hardware_tx_error_counter=None)

        def serve():
            try:
                if not kernel.ConnectNamedPipe(handle, None) and ctypes.get_last_error() != 535:
                    raise ctypes.WinError(ctypes.get_last_error())
                sent = False
                while True:
                    length, = struct.unpack("<I", read_exact(4))
                    if not 0 < length <= 65536:
                        raise ValueError("Invalid request size")
                    request = json.loads(read_exact(length))
                    reply = dict(result="hello", client_id=1, status=status) if request["op"] == "hello" else dict(result="status", status=status)
                    write(dict(kind="reply", id=request["id"], reply=reply))
                    if request["op"] == "status" and not sent:
                        deadline = time.monotonic() + 10
                        while not ready.exists():
                            if time.monotonic() > deadline:
                                raise TimeoutError("Godot did not start recording")
                            time.sleep(0.005)
                        offsets = [0, 1000, 2000, 1500, 3000, 4000]
                        for index, delay in enumerate([0.01, 0.08, 0.003, 0.005, 0.05, 0.015]):
                            time.sleep(delay)
                            write(dict(kind="frame", event=dict(sequence=index + 1,
                                frame=dict(id=0x10302, extended=True, dlc=8, data=[0x6f, 0x12, 0x83, 0x3a, 11, 1, 0, 0]),
                                direction="rx", source_client_id=None, timestamp_us=time.time_ns() // 1000,
                                hardware_timestamp_us=(9_000_000 + offsets[index]) if enabled else None)))
                        deadline = time.monotonic() + 10
                        while ready.read_text() != "hardware":
                            if time.monotonic() > deadline:
                                raise TimeoutError("Godot did not check hardware indicator")
                            time.sleep(0.005)
                        write(dict(kind="frame", event=dict(sequence=7,
                            frame=dict(id=0x456, dlc=1, data=[42]), direction="rx", source_client_id=None,
                            timestamp_us=time.time_ns() // 1000, hardware_timestamp_us=None)))
                        sent = True
            except EOFError:
                pass
            except Exception as error:
                failures.append(error)

        server = threading.Thread(target=serve, daemon=True)
        server.start()
        try:
            environment = dict(os.environ, CYDERVIS_TIMESTAMP_INTERFACE=name,
                               CYDERVIS_TIMESTAMP_DIRECTORY=directory, CYDERVIS_TIMESTAMP_READY=str(ready),
                               CYDERVIS_TIMESTAMP_EPOCH=str(epoch), CYDERVIS_TIMESTAMP_ENABLED=str(int(enabled)),
                               CYDERVIS_TIMESTAMP_SUPPORTED=str(int(driver in ("gsusb", "pcan") and capability is True)))
            try:
                result = subprocess.run([executable, "--headless", "--path", str(Path(__file__).resolve().parents[1]),
                                         "--script", "res://tests/hardware_timestamps.gd"],
                                        env=environment, capture_output=True, text=True, encoding="utf-8", timeout=20)
            except subprocess.TimeoutExpired as error:
                raise AssertionError(f"Godot timed out:\n{error.stdout!r}\n{error.stderr!r}") from error
            output = result.stdout + result.stderr
            server.join(timeout=2)
            assert not failures, failures
            assert result.returncode == 0 and "tests passed" in output and "SCRIPT ERROR" not in output, output
            print(f"PASS timestamp indicator and epoch: {driver}, capability={capability}, enabled={enabled}, epoch={epoch}")
        finally:
            kernel.CancelIoEx(handle, None)
            kernel.CloseHandle(handle)


def run(executable):
    for epoch in range(3):
        run_case(executable, "gsusb", True, True, epoch)
    for driver, capability, enabled in [
        ("gsusb", False, True), ("gsusb", None, True), ("slcan", None, True),
        ("virtual", True, True), ("gsusb", True, False), ("pcan", True, True),
    ]:
        run_case(executable, driver, capability, enabled, 1)


if __name__ == "__main__":
    run(sys.argv[1] if len(sys.argv) > 1 else os.environ.get("GODOT_BIN", "godot"))
