import asyncio
import socket
import typing
import dataclasses
import enum
from common.frame import (
    InvalidFrame,
    Frame,
)
import structlog
from common.reader import (
    read_u32,
    read_str,
    read_u8,
    read_u16,
)
from common.writer import (
    write_u8,
    write_u16,
    write_u32,
    write_str,
)


logger = structlog.get_logger(__name__)


class MessageType(enum.IntEnum):
    Error = 0x10
    Plate = 0x20
    Ticket = 0x21
    WantHeartbeat = 0x40
    Heartbeat = 0x41
    IAmCamera = 0x80
    IAmDispatcher = 0x81


@dataclasses.dataclass
class Error(Frame):
    msg: str

    @classmethod
    async def read(cls, reader: asyncio.StreamReader) -> typing.Self:
        msg = await read_str(reader)
        return cls(msg=msg)

    def write(self, writer: asyncio.StreamWriter) -> None:
        write_str(writer, self.msg)


@dataclasses.dataclass
class Plate(Frame):

    plate: str
    timestamp: int

    @classmethod
    async def read(cls, reader: asyncio.StreamReader) -> typing.Self:
        plate = await read_str(reader)
        timestamp = await read_u32(reader)
        return cls(plate=plate, timestamp=timestamp)

    def write(self, writer: asyncio.StreamWriter):
        write_str(writer, self.plate)
        write_u32(writer, self.timestamp)


@dataclasses.dataclass
class Ticket(Frame):
    plate: str
    road: int
    mile1: int
    timestamp1: int
    mile2: int
    timestamp2: int
    speed: int

    @classmethod
    async def read(cls, reader: asyncio.StreamReader) -> typing.Self:
        plate = await read_str(reader)
        road = await read_u16(reader)
        mile1 = await read_u16(reader)
        timestamp1 = await read_u32(reader)
        mile2 = await read_u16(reader)
        timestamp2 = await read_u32(reader)
        speed = await read_u16(reader)
        return cls(plate, road, mile1, timestamp1, mile2, timestamp2, speed)

    def write(self, writer: asyncio.StreamWriter):
        write_str(writer, self.plate)
        write_u16(writer, self.road)
        write_u16(writer, self.mile1)
        write_u32(writer, self.timestamp1)
        write_u16(writer, self.mile2)
        write_u32(writer, self.timestamp2)
        write_u16(writer, self.speed)


@dataclasses.dataclass
class WantHeartbeat(Frame):
    interval: int

    @classmethod
    async def read(cls, reader: asyncio.StreamReader) -> typing.Self:
        interval = await read_u32(reader)
        return cls(interval=interval)

    def write(self, writer: asyncio.StreamWriter):
        write_u32(writer, self.interval)


@dataclasses.dataclass
class Heartbeat(Frame):

    @classmethod
    async def read(cls, _reader: asyncio.StreamReader) -> typing.Self:
        return cls()

    def write(self, writer: asyncio.StreamWriter):
        pass


@dataclasses.dataclass
class IAmCamera(Frame):
    road: int
    mile: int
    limit: int

    @classmethod
    async def read(cls, reader: asyncio.StreamReader) -> typing.Self:
        road = await read_u16(reader)
        mile = await read_u16(reader)
        limit = await read_u16(reader)
        return cls(road, mile, limit)

    def write(self, writer):
        write_u16(writer, self.road)
        write_u16(writer, self.mile)
        write_u16(writer, self.limit)


@dataclasses.dataclass
class IAmDispatcher(Frame):
    num_roads: int
    roads: typing.List[int]

    @classmethod
    async def read(cls, reader: asyncio.StreamReader) -> typing.Self:
        size = await read_u8(reader)
        contents = []
        for _ in range(size):
            contents.append(await read_u16(reader))
        return cls(num_roads=size, roads=contents)

    def write(self, writer: asyncio.StreamWriter):
        writer.write(self.num_roads.to_bytes(1, byteorder='big'))
        for road in self.roads:
            write_u16(writer, road)


@dataclasses.dataclass
class Message(Frame):
    kind: MessageType
    data: Ticket | Heartbeat | WantHeartbeat | IAmCamera | IAmDispatcher | Plate | Error

    @classmethod
    def from_data(cls, data: Ticket | Heartbeat | WantHeartbeat | IAmCamera | IAmDispatcher | Plate | Error) -> typing.Self:
        if isinstance(data, Heartbeat):  # noqa
            return cls(kind=MessageType.Heartbeat, data=data)
        if isinstance(data, WantHeartbeat):
            return cls(kind=MessageType.WantHeartbeat, data=data)
        if isinstance(data, IAmCamera):
            return cls(kind=MessageType.IAmCamera, data=data)
        if isinstance(data, IAmDispatcher):  # noqa
            return cls(kind=MessageType.IAmDispatcher, data=data)
        if isinstance(data, Plate):
            return cls(kind=MessageType.Plate, data=data)
        if isinstance(data, Error):
            return cls(kind=MessageType.Error, data=data)
        if isinstance(data, Ticket):
            return cls(kind=MessageType.Ticket, data=data)

    @classmethod
    async def read(cls, reader: asyncio.StreamReader) -> typing.Self:
        logger.debug("Waiting to read header.")
        header = await reader.read(1)
        logger.debug(f"Read header: {header!r}")
        if not len(header):
            # Received EOF.
            logger.info("Received EOF.")
            return None
        try:
            message_type = MessageType(int.from_bytes(header, byteorder="big"))
        except ValueError as exc:
            raise InvalidFrame(header) from exc
        logger.debug(f"Found message type: {message_type!r}")

        try:
            match message_type:
                case MessageType.Error:
                    message = await Error.read(reader)
                case MessageType.Plate:
                    message = await Plate.read(reader)
                case MessageType.Ticket:
                    message = await Ticket.read(reader)
                case MessageType.WantHeartbeat:
                    message = await WantHeartbeat.read(reader)
                case MessageType.Heartbeat:
                    message = await Heartbeat.read(reader)
                case MessageType.IAmDispatcher:
                    message = await IAmDispatcher.read(reader)
                case MessageType.IAmCamera:
                    message = await IAmCamera.read(reader)
                case _:
                    raise NotImplementedError(f"Unknown/Unimplemented message type {message_type!r}")
        except asyncio.IncompleteReadError as exc:
            raise InvalidFrame() from exc
        except ValueError as exc:
            raise InvalidFrame() from exc
        return cls(kind=message_type, data=message)

    def write(self, writer: asyncio.StreamWriter):
        writer.write(self.kind.to_bytes(length=1, byteorder="big"))
        self.data.write(writer)
