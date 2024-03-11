import abc
import asyncio
import socket
import typing
import dataclasses

from .messages import (
    Frame,
    Framer,
    Heartbeat,
    Message,
    MessageType,
    Error,
    WantHeartbeat,
    Plate,
)
from .state import SpeedDaemon
import structlog


logger = structlog.get_logger(__name__)


class Connection:

    peer: typing.Tuple[str, int]
    want_heartbeats: asyncio.Event
    stop_heartbeats: asyncio.Event
    heartbeat_interval: float

    def __init__(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter, connection_closed_event: asyncio.Event, daemon: SpeedDaemon):
        addr, port = writer.get_extra_info('peername')
        self.peer = (addr, port)
        self.logger = logger.bind(peer=f"{addr}:{port}")

        self.reader = reader
        self.writer = writer
        self.daemon = daemon

        self.frame_sender: asyncio.Queue[Frame] = asyncio.Queue()
        self.frame_receiver: asyncio.Queue[Frame] = asyncio.Queue()

        self.heartbeat_interval = 0
        self.want_heartbeats = asyncio.Event()
        self.stop_heartbeats = asyncio.Event()

        # To notify ConnectionManager that the connection has been closed.
        self.connection_closed_event = connection_closed_event

        self.framer = Framer(self, self.frame_receiver, self.frame_sender)

    def set_heartbeat_interval(self, value: float):
        self.heartbeat_interval = value
        self.want_heartbeats.set()

    async def close(self):
        self.stop_heartbeats.set()
        self.writer.close()
        self.connection_closed_event.set()
        try:
            await self.writer.wait_closed()
        except socket.error:
            return
        self.logger.info("Closing connection")

    async def schedule_heartbeats(self):
        await self.want_heartbeats.wait()  # wait for start signal.
        if self.heartbeat_interval == 0:
            self.logger.debug("Heartbeat interval is 0, so we won't send any heartbeat messages.")
            return
        while not self.stop_heartbeats.is_set():
            self.logger.debug(f"Sleeping for {self.heartbeat_interval:.2f}s before sending heartbeat.")
            await asyncio.sleep(self.heartbeat_interval)
            await self.frame_sender.put(Heartbeat().as_frame())

    async def handle(self):
        # self.logger.debug(f"Client joined SpeedDaemon")
        run_forever, schedule_heartbeats = await asyncio.gather(
            self.framer.run_forever(),
            self.schedule_heartbeats(),
            return_exceptions=False
        )
        # try:
        #     async with asyncio.TaskGroup() as tg:
        #         run_forever = tg.create_task(self.framer.run_forever())
        #         schedule_heartbeats = tg.create_task(self.schedule_heartbeats())
        # finally:
        self.logger.debug(f"run forever: {run_forever}")
        self.logger.debug(f"schedule_heartbeats: {schedule_heartbeats}")
        await self.close()
