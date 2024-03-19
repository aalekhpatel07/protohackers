import abc
import functools
import socket
import typing
import asyncio
import structlog
from .message import Peer


logger = structlog.get_logger(__name__)


class InvalidFrame(Exception):
    """Raised when  an invalid frame.

    """


class Frame(abc.ABC):

    @abc.abstractmethod
    def write(self, writer: asyncio.StreamWriter) -> None:
        """Try to write a complete frame into an underlying StreamWriter.

        :param writer: The underlying stream of bytes to write into.
        :return:
        """

    @classmethod
    @abc.abstractmethod
    async def read(cls, reader: asyncio.StreamReader) -> typing.Self:
        """Try to read a complete frame from an underlying StreamReader.

        :param reader: The underlying stream of bytes to read from.
        :return: A complete frame if we were able to read it.

        :raises InvalidFrame: If we read any malformed frame.
        """


class Framer:
    # To read bytes from a TCPStream.
    reader: asyncio.StreamReader
    # To write bytes into a TCPStream.
    writer: asyncio.StreamWriter

    # To read complete frames
    frame_reader: asyncio.Queue[Frame]
    # write complete frames
    frame_writer: asyncio.Queue[typing.Optional[Frame]]

    def __init__(self,
                 reader: asyncio.StreamReader,
                 writer: asyncio.StreamWriter,
                 frame_reader: asyncio.Queue[Frame],
                 frame_writer: asyncio.Queue[typing.Optional[Frame]],
                 connection_closed_by_peer: asyncio.Event,
                 bad_frame_read: asyncio.Event,
                 frame_cls,
                 ):
        self.reader = reader
        self.writer = writer
        self.frame_reader = frame_reader
        self.frame_writer = frame_writer
        self.connection_closed_by_peer = connection_closed_by_peer
        self.bad_frame_read = bad_frame_read
        self.frame_cls = frame_cls

    @functools.cached_property
    def peer(self) -> Peer:
        return self.writer.get_extra_info('peername')

    async def write_frames(self):
        logger.debug("Started listening for frames to write")
        while True:
            maybe_frame = await self.frame_writer.get()
            if maybe_frame is None:
                logger.info("Received a signal to stop writing any more frames. Stopping writer.")
                return
            logger.debug(f"Got frame to write: {maybe_frame!r}")
            try:
                maybe_frame.write(self.writer)
                await self.writer.drain()
            except socket.error as exc:
                logger.error("Couldn't write frame. Client went away!")
                self.connection_closed_by_peer.set()
                return
            logger.debug(f"Wrote frame {maybe_frame}")

    async def read_frames(self):
        logger.debug("Started listening for incoming frames")
        while not self.connection_closed_by_peer.is_set():
            logger.debug("Waiting for a complete frame.")
            maybe_frame = await self.frame_cls.read(self.reader)
            if maybe_frame is None:
                logger.debug("Received EOF from peer. Terminating frame reader.")
                self.connection_closed_by_peer.set()
                return

            logger.debug(f"Just read a complete frame: {maybe_frame!r}")
            await self.frame_reader.put(maybe_frame)

        logger.debug("Received a signal to stop reading any more frames. Stopping reader.")

    async def run_to_completion(self):
        try:
            async with asyncio.TaskGroup() as tg:
                write_frames = tg.create_task(self.write_frames())
                read_frames = tg.create_task(self.read_frames())
        except ExceptionGroup as exc_grp:
            logger.error(f"Exception in run_to_completion: {exc_grp}")
        finally:
            logger.info(
                f"write_frames: {write_frames!r}\n"
                f"read_frames: {write_frames!r}",
            )
            if write_frames.exception() is not None:
                logger.error(f"Got an exception while writing frames: {write_frames.exception()}")
                self.connection_closed_by_peer.set()
                return

            # If we couldn't read a complete frame because of a bad frame
            # then peer didn't close the connection yet.
            maybe_exception = read_frames.exception()
            if maybe_exception and isinstance(maybe_exception, InvalidFrame):
                self.bad_frame_read.set()
                return
            elif maybe_exception is not None:
                logger.error(f"Got an exception while reading frames: {read_frames.exception()}")

            self.connection_closed_by_peer.set()
