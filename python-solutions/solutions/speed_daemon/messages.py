import abc
import asyncio
import socket
import typing
import dataclasses
import enum
from .exceptions import (
    InvalidFrame,
)
import functools
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


@dataclasses.dataclass
class Frame:
    kind: 'MessageType'
    data: 'Message'

    async def write(self, writer: asyncio.StreamWriter):
        write_u8(writer, self.kind)
        if writer.is_closing():
            return
        self.data.to_writer(writer)
        # await writer.drain()  # TODO: Think about if this is necessary.

    def __str__(self):
        return f"<Frame kind={self.kind.name}>"

    __repr__ = __str__


class Framer:
    reader: asyncio.StreamReader
    writer: asyncio.StreamWriter
    frame_reader: asyncio.Queue[typing.Optional[Frame]]
    frame_writer: asyncio.Queue[typing.Optional[Frame]]
    peer: str

    def __init__(self,
                 connection,
                 frame_reader: asyncio.Queue[typing.Optional[Frame]],
                 frame_writer: asyncio.Queue[typing.Optional[Frame]],
                 ):
        self.reader = connection.reader
        self.writer = connection.writer
        self.peer = self.writer.get_extra_info('peername')
        self.frame_reader = frame_reader
        self.frame_writer = frame_writer
        self.logger = logger.bind(peer=f"{self.peer[0]}:{self.peer[1]}")
        self.connection = connection

    async def write_frames(self):
        # self.logger.debug("Started listening for frames to write")
        while 1:
            maybe_frame = await self.frame_writer.get()
            # self.logger.debug(f"Got frame to write: {maybe_frame!r}")
            if maybe_frame is None:
                self.logger.info("Received a signal to stop writing any more frames. Stopping writer.")
                return
            frame: Frame = maybe_frame
            try:
                await frame.write(self.writer)
                # self.logger.debug(f"Wrote frame {frame}")
            except Exception as exc:  # noqa
                self.logger.exception(exc)
                return

    async def read_frame(self) -> typing.Optional[Frame]:
        self.logger.debug("Waiting to read header.")
        header = await self.reader.read(1)
        self.logger.debug("Read header")
        if not len(header):
            # Received EOF.
            return None
        try:
            message_type = MessageType(int.from_bytes(header, byteorder="big"))
        except ValueError as exc:
            raise InvalidFrame(header) from exc

        self.logger.debug(f"Found message type: {message_type!r}")
        match message_type:
            case MessageType.Error:
                message = await Error.from_reader(self.reader)
            case MessageType.Plate:
                message = await Plate.from_reader(self.reader)
            case MessageType.Ticket:
                message = await Ticket.from_reader(self.reader)
            case MessageType.WantHeartbeat:
                message = await WantHeartbeat.from_reader(self.reader)
            case MessageType.Heartbeat:
                message = await Heartbeat.from_reader(self.reader)
            case MessageType.IAmDispatcher:
                message = await IAmDispatcher.from_reader(self.reader)
            case MessageType.IAmCamera:
                message = await IAmCamera.from_reader(self.reader)
            case _:
                raise NotImplementedError(f"Unimplemented message type {message_type!r}")
        self.logger.debug(f"Found message: {message}")
        return Frame(message_type, message)

    async def read_frames(self):
        self.logger.debug("Started listening for incoming frames")
        while 1:
            try:
                self.logger.debug("Waiting for a complete frame.")
                maybe_frame = await self.read_frame()
                self.logger.debug(f"Just read a complete frame: {maybe_frame}")
                if maybe_frame is None:
                    self.logger.debug("Received EOF from client. Terminating connection gracefully.")
                    await self.connection.close()
                    return None
                await self.frame_reader.put(maybe_frame)
            except InvalidFrame as exc:
                self.logger.error(f"Received an invalid frame: {exc}, kicking client!")
                return await self.frame_writer.put(Error(msg=f"invalid frame: {exc}").as_frame())

    async def run_forever(self):
        read_frames, write_frames = await asyncio.gather(
            self.read_frames(),
            self.write_frames(),
            return_exceptions=True
        )
        # async with asyncio.TaskGroup() as tg:
        #     read_frames = tg.create_task(self.read_frames())
        #     write_frames = tg.create_task(self.write_frames())

        logger.debug(f"read frames: {read_frames}")
        logger.debug(f"write frames: {write_frames}")


class MessageType(enum.IntEnum):
    Error = 0x10
    Plate = 0x20
    Ticket = 0x21
    WantHeartbeat = 0x40
    Heartbeat = 0x41
    IAmCamera = 0x80
    IAmDispatcher = 0x81


class Message(abc.ABC):

    @classmethod
    async def from_reader(cls, reader: asyncio.StreamReader) -> typing.Self:
        try:
            return await cls.try_from_reader(reader)
        except asyncio.IncompleteReadError as exc:
            raise InvalidFrame from exc
        except ValueError as exc:
            raise InvalidFrame from exc

    @classmethod
    @abc.abstractmethod
    async def try_from_reader(cls, reader: asyncio.StreamReader) -> typing.Self:
        ...

    @abc.abstractmethod
    def to_writer(self, writer: asyncio.StreamWriter):
        ...

    @abc.abstractmethod
    def msg_type(self) -> MessageType:
        ...

    def as_frame(self) -> Frame:
        return Frame(kind=self.msg_type(), data=self)


@dataclasses.dataclass
class Error(Message):
    msg: str

    @classmethod
    async def try_from_reader(cls, reader: asyncio.StreamReader) -> typing.Self:
        msg = await read_str(reader)
        return cls(msg=msg)

    def to_writer(self, writer):
        write_str(writer, self.msg)

    def msg_type(self) -> MessageType:
        return MessageType.Error


@dataclasses.dataclass
class Plate(Message):

    plate: str
    timestamp: int

    @classmethod
    async def try_from_reader(cls, reader: asyncio.StreamReader) -> typing.Self:
        plate = await read_str(reader)
        timestamp = await read_u32(reader)
        return cls(plate=plate, timestamp=timestamp)

    def to_writer(self, writer):
        write_str(writer, self.plate)
        write_u32(writer, self.timestamp)

    def msg_type(self) -> MessageType:
        return MessageType.Plate


@dataclasses.dataclass
class Ticket(Message):
    plate: str
    road: int
    mile1: int
    timestamp1: int
    mile2: int
    timestamp2: int
    speed: int

    @classmethod
    async def try_from_reader(cls, reader: asyncio.StreamReader) -> typing.Self:
        plate = await read_str(reader)
        road = await read_u16(reader)
        mile1 = await read_u16(reader)
        timestamp1 = await read_u32(reader)
        mile2 = await read_u16(reader)
        timestamp2 = await read_u32(reader)
        speed = await read_u16(reader)
        return cls(plate, road, mile1, timestamp1, mile2, timestamp2, speed)

    def to_writer(self, writer: asyncio.StreamWriter):
        write_str(writer, self.plate)
        write_u16(writer, self.road)
        write_u16(writer, self.mile1)
        write_u32(writer, self.timestamp1)
        write_u16(writer, self.mile2)
        write_u32(writer, self.timestamp2)
        write_u16(writer, self.speed)

    def msg_type(self) -> MessageType:
        return MessageType.Ticket


@dataclasses.dataclass
class WantHeartbeat(Message):
    interval: int

    @classmethod
    async def try_from_reader(cls, reader: asyncio.StreamReader) -> typing.Self:
        interval = await read_u32(reader)
        return cls(interval=interval)

    def to_writer(self, writer):
        write_u32(writer, self.interval)

    def msg_type(self) -> MessageType:
        return MessageType.WantHeartbeat


@dataclasses.dataclass
class Heartbeat(Message):

    @classmethod
    async def try_from_reader(cls, reader: asyncio.StreamReader) -> typing.Self:
        return cls()

    def to_writer(self, writer):
        pass

    def msg_type(self) -> MessageType:
        return MessageType.Heartbeat


@dataclasses.dataclass
class IAmCamera(Message):
    road: int
    mile: int
    limit: int

    @classmethod
    async def try_from_reader(cls, reader: asyncio.StreamReader) -> typing.Self:
        road = await read_u16(reader)
        mile = await read_u16(reader)
        limit = await read_u16(reader)
        return cls(road, mile, limit)

    def to_writer(self, writer):
        write_u16(writer, self.road)
        write_u16(writer, self.mile)
        write_u16(writer, self.limit)

    def msg_type(self) -> MessageType:
        return MessageType.IAmCamera


@dataclasses.dataclass
class IAmDispatcher(Message):
    num_roads: int
    roads: typing.List[int]

    @classmethod
    async def try_from_reader(cls, reader: asyncio.StreamReader) -> typing.Self:
        size = await read_u8(reader)
        contents = []
        for _ in range(size):
            contents.append(await read_u16(reader))
        return cls(num_roads=size, roads=contents)

    def to_writer(self, writer):
        writer.write(self.num_roads.to_bytes(1, byteorder='big'))
        for road in self.roads:
            write_u16(writer, road)

    def msg_type(self) -> MessageType:
        return MessageType.IAmDispatcher
