#!/usr/bin/env python3
import json
import os
import socket
import struct
import subprocess
import sys
import time

PROTOCOL = 777
MAX_PACKET = 1_048_576


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
    if not 0 < size <= MAX_PACKET:
        raise ValueError(f"invalid packet size {size}")
    result = bytearray()
    while len(result) < size:
        part = sock.recv(size - len(result))
        if not part:
            raise EOFError("server closed during packet")
        result.extend(part)
    return bytes(result)


def handshake(host, port, state, protocol=PROTOCOL):
    encoded_host = host.encode()
    return (
        b"\x00"
        + varint(protocol)
        + varint(len(encoded_host))
        + encoded_host
        + struct.pack(">H", port)
        + varint(state)
    )


def open_handshake(host, port, state, protocol=PROTOCOL):
    sock = socket.create_connection((host, port), timeout=3)
    sock.settimeout(3)
    send_frame(sock, handshake(host, port, state, protocol))
    return sock


def login_start(name):
    encoded = name.encode()
    return b"\x00" + varint(len(encoded)) + encoded + bytes(range(16))


def client_information():
    language = b"en_us"
    return (
        b"\x00"
        + varint(len(language))
        + language
        + bytes([8])
        + varint(0)
        + b"\x01"
        + bytes([0])
        + varint(1)
        + b"\x00\x00"
        + varint(0)
    )


def decode_text_component(packet):
    offset = 1
    length = 0
    shift = 0
    while True:
        byte = packet[offset]
        offset += 1
        length |= (byte & 0x7F) << shift
        if not byte & 0x80:
            break
        shift += 7
    return json.loads(packet[offset : offset + length].decode())


def decode_nbt_text_component(packet):
    if len(packet) < 4 or packet[1] != 8:
        raise AssertionError(f"expected NBT String Tag, got {packet[:8]!r}")
    length = int.from_bytes(packet[2:4], "big")
    if len(packet) != 4 + length:
        raise AssertionError(f"NBT String Tag length mismatch: {len(packet)} != {4 + length}")
    return packet[4:].decode("utf-8")

def expect_disconnect(packet, packet_id, needle=None):
    if not packet or packet[0] != packet_id:
        raise AssertionError(f"expected disconnect packet id {packet_id}, got {packet[:8]!r}")
    if needle is not None:
        text = decode_text_component(packet).get("text", "")
        if needle not in text:
            raise AssertionError(f"disconnect reason {text!r} does not contain {needle!r}")


def status_test(host, port):
    with open_handshake(host, port, 1) as sock:
        send_frame(sock, b"\x00")
        response = read_frame(sock)
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
        status = json.loads(response[offset : offset + length])
        if status["version"] != {"name": "26.3", "protocol": PROTOCOL}:
            raise AssertionError(f"unexpected server version: {status['version']}")
        print("status response: 26.3 / protocol 777")

        ping = b"\x01" + bytes(range(8))
        send_frame(sock, ping)
        echo = read_frame(sock)
        if echo != ping:
            raise AssertionError(f"ping echo mismatch: {echo!r} != {ping!r}")
        print("ping/pong echo: 8 bytes")


def login_test(host, port):
    with open_handshake(host, port, 2) as sock:
        send_frame(sock, login_start("Test"))
        response = read_frame(sock)
        if os.environ.get("MC_OFFLINE_MODE") == "1":
            if not response or response[0] != 2:
                raise AssertionError(f"expected Login Success, got {response[:8]!r}")
            if response[1:17] != bytes(range(16)):
                raise AssertionError("Login Success UUID does not match Login Start")
            print("offline Login Success received")
            send_frame(sock, b"\x03")
            send_frame(sock, client_information())
            packet = read_frame(sock)
            if packet[0] == 2:
                reason = decode_nbt_text_component(packet)
                if "Configuration" not in reason:
                    raise AssertionError(
                        f"unexpected Configuration Disconnect reason: {reason!r}"
                    )
                print("Login Acknowledged handled; NBT Configuration Disconnect received")
            else:
                while packet[0] != 0x0F:
                    if packet[0] == 2:
                        raise AssertionError("Configuration replay disconnected early")
                    packet = read_frame(sock)
                send_frame(sock, b"\x07" + varint(0))
                while True:
                    packet = read_frame(sock)
                    if packet[0] == 2:
                        raise AssertionError("Configuration replay disconnected before finish")
                    if packet[0] == 3:
                        break
                send_frame(sock, b"\x03")
                play_login = read_frame(sock)
                if not play_login or play_login[0] != 0x32:
                    raise AssertionError(
                        f"expected Play Login packet, got {play_login[:8]!r}"
                    )
                print("Configuration replay completed; Play Login received")
        else:
            expect_disconnect(response, 0, "Offline login is disabled")
            print("offline login disabled; expected Login Disconnect received")


def wrong_protocol_test(host, port):
    with open_handshake(host, port, 2, protocol=PROTOCOL - 1) as sock:
        send_frame(sock, login_start("Test"))
        expect_disconnect(read_frame(sock), 0, "protocol")
        print("wrong protocol rejected with Login Disconnect")


def invalid_username_test(host, port):
    with open_handshake(host, port, 2) as sock:
        send_frame(sock, login_start("Bad Name"))
        expect_disconnect(read_frame(sock), 0, "Invalid Login Start")
        print("invalid username rejected with Login Disconnect")


def public_bind_refusal_test(binary, env):
    refusal_env = env.copy()
    refusal_env.pop("MC_ALLOW_PUBLIC_OFFLINE", None)
    process = subprocess.Popen(
        [binary, "0.0.0.0:0"],
        env=refusal_env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )
    try:
        _, stderr = process.communicate(timeout=3)
    except subprocess.TimeoutExpired:
        process.kill()
        _, stderr = process.communicate()
        raise AssertionError("public offline bind was not refused")
    if process.returncode == 0:
        raise AssertionError("public offline bind unexpectedly succeeded")
    if b"refusing to bind offline mode" not in stderr:
        raise AssertionError("public offline bind refusal reason was not written")
    print("public offline bind refused without override")


def public_bind_override_test(binary, env):
    override_env = env.copy()
    override_env["MC_ALLOW_PUBLIC_OFFLINE"] = "1"
    process = subprocess.Popen(
        [binary, "0.0.0.0:0"],
        env=override_env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )
    time.sleep(0.1)
    if process.poll() is not None:
        raise AssertionError("public offline bind with override unexpectedly exited")
    process.terminate()
    try:
        stderr = process.communicate(timeout=2)[1]
    except subprocess.TimeoutExpired:
        process.kill()
        stderr = process.communicate()[1]
    if b"WARNING: offline mode is unauthenticated" not in stderr:
        raise AssertionError("offline public-bind warning was not written to console")
    print("public offline bind allowed with explicit override")


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
                raise RuntimeError(
                    f"server exited at startup: {process.stderr.read().decode(errors='replace')}"
                )
            try:
                with socket.create_connection(("127.0.0.1", port), timeout=0.1):
                    break
            except OSError:
                time.sleep(0.05)
        else:
            raise TimeoutError("server did not open its listener")
        status_test("127.0.0.1", port)
        login_test("127.0.0.1", port)
        wrong_protocol_test("127.0.0.1", port)
        invalid_username_test("127.0.0.1", port)
        if offline_mode:
            public_bind_refusal_test(binary, server_env)
            public_bind_override_test(binary, server_env)
    finally:
        process.terminate()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


if __name__ == "__main__":
    main()
