import asyncio


async def read_str(reader: asyncio.StreamReader) -> str:
    size = await reader.readexactly(1)
    size = int.from_bytes(size, byteorder='big')
    msg_bytes = await reader.readexactly(size)
    return msg_bytes.decode('ascii')


async def read_u8(reader: asyncio.StreamReader) -> int:
    contents = await reader.readexactly(1)
    return int.from_bytes(contents, byteorder='big')


async def read_u16(reader: asyncio.StreamReader) -> int:
    contents = await reader.readexactly(2)
    return int.from_bytes(contents, byteorder='big')


async def read_u32(reader: asyncio.StreamReader) -> int:
    contents = await reader.readexactly(4)
    return int.from_bytes(contents, byteorder='big')
