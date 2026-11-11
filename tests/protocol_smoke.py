#!/usr/bin/env python3
import json
import os
import socket
import struct
import subprocess
import sys
import time
import time


def varint(value):
    value &= 0xFFFFFFFF
    encoded = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        encoded.append(byte | (0x80 if value else 0))
        if not value:
            return bytes(encoded)


def send_frame(sock, packet):
    sock.sendall(varint(len(packet)) + packet)


def read_varint(sock):
    value = shift = 0
    while shift < 35:
        byte = sock.recv(1)
        if not byte:
            raise EOFError("server closed during packet VarInt")
        byte = byte[0]
        value |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return value
        shift += 7
    raise ValueError("packet VarInt exceeds five bytes")


def read_frame(sock):
    size = read_varint(sock)
    if not 0 < size <= 1_048_576:
        raise ValueError(f"invalid packet size {size}")
    result = bytearray()
    while len(result) < size:
        part = sock.recv(size - len(result))
        if not part:
            raise EOFError("server closed during packet")
        result.extend(part)
    return bytes(result)


def handshake(host, port, state):
    encoded_host = host.encode()
    return b"\x00" + varint(777) + varint(len(encoded_host)) + encoded_host + struct.pack(">H", port) + varint(state)


def request(host, port, state, packet):
    with socket.create_connection((host, port), timeout=3) as sock:
        sock.settimeout(3)
        send_frame(sock, handshake(host, port, state))
        send_frame(sock, packet)
        return read_frame(sock)


def status_test(host, port):
    response = request(host, port, 1, b"\x00")
    if response[0] != 0:
        raise AssertionError(f"unexpected status response packet id {response[0]}")
    length, shift, offset = 0, 0, 1
    while True:
        byte = response[offset]
        offset += 1
        length |= (byte & 0x7F) << shift
        shift += 7
        if not byte & 0x80:
            break
    status = json.loads(response[offset:offset + length])
    if status["version"] != {"name": "26.3", "protocol": 777}:
        raise AssertionError(f"unexpected server version: {status['version']}")
    print("status response: 26.3 / protocol 777")


def login_test(host, port):
    login_start = b"\x00" + varint(4) + b"Test" + bytes(range(16))
    with socket.create_connection((host, port), timeout=3) as sock:
        sock.settimeout(3)
        send_frame(sock, handshake(host, port, 2))
        send_frame(sock, login_start)
        response = read_frame(sock)
        if os.environ.get("MC_OFFLINE_MODE") == "1":
            if not response or response[0] != 2:
                raise AssertionError(f"expected Login Success, got {response[:8]!r}")
            if response[1:17] != bytes(range(16)):
                raise AssertionError("Login Success UUID does not match Login Start")
            print("offline Login Success received")
            send_frame(sock, b"\x03")
            config_disconnect = read_frame(sock)
            if not config_disconnect or config_disconnect[0] != 2:
                raise AssertionError(f"expected Configuration Disconnect, got {config_disconnect[:8]!r}")
            print("Login Acknowledged handled; expected Configuration Disconnect received")
        else:
            if not response or response[0] != 0:
                raise AssertionError(f"expected Login Disconnect, got {response[:8]!r}")
            print("offline login disabled; expected Login Disconnect received")


def main():
    binary = sys.argv[1]
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    server_env = os.environ.copy()
    offline_mode = server_env.get("MC_OFFLINE_MODE") == "1"
    process = subprocess.Popen(
        [binary, f"127.0.0.1:{port}"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        env=server_env,
    )
    try:
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise RuntimeError(f"server exited at startup: {process.stderr.read().decode(errors='replace')}")
            try:
                with socket.create_connection(("127.0.0.1", port), timeout=0.1):
                    break
            except OSError:
                time.sleep(0.05)
        else:
            raise TimeoutError("server did not open its listener")
        status_test("127.0.0.1", port)
        login_test("127.0.0.1", port)
        if offline_mode:
            public_bind = subprocess.Popen(
                [binary, "0.0.0.0:0"],
                env=server_env,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
            )
            time.sleep(0.1)
            if public_bind.poll() is None:
                public_bind.terminate()
                public_bind.wait(timeout=2)
                raise AssertionError("offline mode unexpectedly allowed public bind")
    finally:
        process.terminate()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


if __name__ == "__main__":
    main()
