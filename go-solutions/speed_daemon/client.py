#!/usr/bin/env python3

import socket

HOST = "127.0.0.1"
PORT = 9001

send = b"\x81\x03\x00\x42\x01\x70\x13\x88"


def main():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.connect((HOST, PORT))
        s.sendall(send)
        data = s.recv(1024)
        print(data)


if __name__ == '__main__':
    main()

