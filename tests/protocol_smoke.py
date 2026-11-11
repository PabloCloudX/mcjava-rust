#!/usr/bin/env python3
import json
import socket
import struct
import subprocess
import sys
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


def login_rejection_test(host, port):
    login_start = b"\x00" + varint(4) + b"Test" + bytes(range(16))
    response = request(host, port, 2, login_start)
    if not response or response[0] != 0:
        raise AssertionError(f"expected Login Disconnect, got {response[:8]!r}")
    length, shift, offset = 0, 0, 1
    while True:
        byte = response[offset]
        offset += 1
        length |= (byte & 0x7F) << shift
        shift += 7
        if not byte & 0x80:
            break
    reason = json.loads(response[offset:offset + length])
    if "not implemented" not in reason.get("text", "").lower():
        raise AssertionError(f"unexpected login result: {reason}")
    print("Login Start parsed; expected explicit Login Disconnect received")


def main():
    binary = sys.argv[1]
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    process = subprocess.Popen([binary, f"127.0.0.1:{port}"], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
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
        login_rejection_test("127.0.0.1", port)
    finally:
        process.terminate()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


if __name__ == "__main__":
    main()
