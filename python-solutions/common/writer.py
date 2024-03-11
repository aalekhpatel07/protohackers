import asyncio
import typing


def write_str(writer: asyncio.StreamWriter, data: str):
    writer.write(len(data).to_bytes(1, byteorder='big'))
    writer.write(data.encode('ascii'))


def write_u8(writer: asyncio.StreamWriter, data: int):
    if not (0x0 < data <= 0xff):
        raise ValueError(f"Could not interpret {data!r} as an u8")
    writer.write(data.to_bytes(1, byteorder='big'))


def write_u16(writer: asyncio.StreamWriter, data: int):
    if not (0x0 < data <= 0xffff):
        raise ValueError(f"Could not interpret {data!r} as an u16")
    writer.write(data.to_bytes(2, byteorder='big'))


def write_u32(writer: asyncio.StreamWriter, data: int):
    if not (0x0 < data <= 0xffffffff):
        raise ValueError(f"Could not interpret {data!r} as an u32")
    writer.write(data.to_bytes(4, byteorder='big'))
