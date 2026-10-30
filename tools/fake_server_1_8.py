#!/usr/bin/env python3
"""A tiny Minecraft 1.8 server (protocol 47) for ViaHorizon's end-to-end test.

It answers the status ping, logs players in (offline, or online with a real
RSA key exchange and AES/CFB8 encryption, without asking Mojang), and sends
what a 1.8 server sends on join: Join Game, the spawn, abilities, the
position, then keep-alives. Every handshake and login it sees is printed, so
the test can check what ViaProxy sent it.

    python3 tools/fake_server_1_8.py PORT [--online]
"""
import json
import os
import socket
import struct
import sys
import threading
import time
import uuid

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import padding, rsa
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms

try:  # CFB8 moved to the "decrepit" ciphers in cryptography 47.
    from cryptography.hazmat.decrepit.ciphers.modes import CFB8
except ImportError:
    from cryptography.hazmat.primitives.ciphers.modes import CFB8


def varint(v):
    out = b""
    v &= 0xFFFFFFFF
    while True:
        b = v & 0x7F
        v >>= 7
        if v:
            out += bytes([b | 0x80])
        else:
            return out + bytes([b])


def string(s):
    b = s.encode()
    return varint(len(b)) + b


class Conn:
    def __init__(self, sock):
        self.sock = sock
        self.enc = None
        self.dec = None
        self.buf = b""

    def raw(self, n):
        while len(self.buf) < n:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise EOFError
            if self.dec:
                chunk = self.dec.update(chunk)
            self.buf += chunk
        out, self.buf = self.buf[:n], self.buf[n:]
        return out

    def read_varint(self):
        v = 0
        for i in range(5):
            b = self.raw(1)[0]
            v |= (b & 0x7F) << (7 * i)
            if not b & 0x80:
                return v
        raise ValueError("varint too long")

    def packet(self):
        n = self.read_varint()
        data = self.raw(n)
        r = Reader(data)
        return r.varint(), r

    def send(self, pid, body=b""):
        payload = varint(pid) + body
        frame = varint(len(payload)) + payload
        if self.enc:
            frame = self.enc.update(frame)
        self.sock.sendall(frame)


class Reader:
    def __init__(self, data):
        self.data = data
        self.at = 0

    def varint(self):
        v = 0
        for i in range(5):
            b = self.data[self.at]
            self.at += 1
            v |= (b & 0x7F) << (7 * i)
            if not b & 0x80:
                return v
        raise ValueError

    def string(self):
        n = self.varint()
        s = self.data[self.at:self.at + n].decode()
        self.at += n
        return s

    def bytes(self, n):
        b = self.data[self.at:self.at + n]
        self.at += n
        return b

    def ushort(self):
        return struct.unpack(">H", self.bytes(2))[0]

    def long(self):
        return struct.unpack(">q", self.bytes(8))[0]


KEY = rsa.generate_private_key(public_exponent=65537, key_size=1024)
PUBLIC = KEY.public_key().public_bytes(serialization.Encoding.DER, serialization.PublicFormat.SubjectPublicKeyInfo)


def log(*a):
    print(*a, flush=True)


def handle(sock, online):
    c = Conn(sock)
    pid, r = c.packet()
    proto, host, port, nxt = r.varint(), r.string(), r.ushort(), r.varint()
    log("HANDSHAKE", json.dumps({"protocol": proto, "host": host, "port": port, "next": nxt}))
    if nxt == 1:
        c.packet()  # status request
        status = {"version": {"name": "1.8.9", "protocol": 47}, "players": {"max": 20, "online": 0},
                  "description": {"text": "A fake 1.8 server"}}
        c.send(0, string(json.dumps(status)))
        pid, r = c.packet()
        c.send(1, struct.pack(">q", r.long()))
        return
    pid, r = c.packet()
    name = r.string()
    log("LOGIN", name)
    if online:
        token = os.urandom(4)
        c.send(1, string("") + varint(len(PUBLIC)) + PUBLIC + varint(4) + token)
        pid, r = c.packet()
        secret = KEY.decrypt(r.bytes(r.varint()), padding.PKCS1v15())
        echoed = KEY.decrypt(r.bytes(r.varint()), padding.PKCS1v15())
        log("ENCRYPTION", "ok" if echoed == token else "bad token")
        cipher = Cipher(algorithms.AES(secret), CFB8(secret))
        c.enc, c.dec = cipher.encryptor(), cipher.decryptor()
    player = str(uuid.uuid3(uuid.NAMESPACE_DNS, "OfflinePlayer:" + name))
    c.send(2, string(player) + string(name))
    # Join Game: entity, survival, overworld, normal, 20 players, "default", full debug.
    c.send(0x01, struct.pack(">iBbBB", 1, 0, 0, 2, 20) + string("default") + b"\x00")
    c.send(0x05, struct.pack(">q", (0 << 38) | (64 << 26) | 0))  # spawn position
    c.send(0x39, struct.pack(">bff", 0, 0.05, 0.1))  # abilities
    c.send(0x08, struct.pack(">dddffb", 0.5, 64.0, 0.5, 0.0, 0.0, 0))  # position and look
    log("JOINED", name)
    sock.settimeout(0.5)
    n = 0
    while True:
        try:
            pid, r = c.packet()
            n += 1
            if n <= 3:
                log("PACKET", hex(pid))
        except socket.timeout:
            pass
        c.send(0x00, varint(int(time.time()) & 0x7FFFFFFF))  # keep alive


def main():
    port = int(sys.argv[1])
    online = "--online" in sys.argv
    server = socket.socket()
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(("127.0.0.1", port))
    server.listen()
    log("LISTENING", port, "online" if online else "offline")

    def serve(sock):
        try:
            handle(sock, online)
        except (EOFError, ConnectionError, OSError):
            pass
        finally:
            sock.close()

    while True:
        sock, _ = server.accept()
        threading.Thread(target=serve, args=(sock,), daemon=True).start()


if __name__ == "__main__":
    main()
