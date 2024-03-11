import dataclasses
import typing
from .connection import (
    Connection,
)
from collections import defaultdict
from .messages import (
    InvalidFrame,
    MessageType,
    Message,
    WantHeartbeat,
    Heartbeat,
    IAmCamera,
    IAmDispatcher,
    Error,
    Plate,
)
from .state import (
    SpeedDaemon,
)
import asyncio
import structlog

type Peer = typing.Tuple[str, int]
type Connections = typing.DefaultDict[Peer, Connection]


logger = structlog.get_logger(__name__)


class ConnectionManager:
    connections: Connections
    daemon: SpeedDaemon

    def __init__(self, connections: Connections = None):
        if connections is None:
            connections = defaultdict()
        self.connections = connections
        self.daemon = SpeedDaemon()

    def register_connection(self, connection: Connection):
        if connection.peer in self.connections:
            return
        self.connections[connection.peer] = connection
        logger.debug(f"Active connections:", total=len(self.connections), connections=self.connections)

    def deregister_connection(self, connection: Connection):
        if connection.peer in self.connections:
            del self.connections[connection.peer]
        logger.debug("Active connections:", total=len(self.connections), connections=self.connections)

    async def handle_connection(
        self,
        reader: asyncio.StreamReader,
        writer: asyncio.StreamWriter,
    ):
        connection_closed_event = asyncio.Event()
        conn: Connection = Connection(reader, writer, connection_closed_event, self.daemon)

        asyncio.ensure_future(conn.handle())
        addr, port = writer.get_extra_info('peername')
        log = logger.bind(peer=f'{addr}:{port}')

        client_is_camera = asyncio.Event()
        client_is_ticket_dispatcher = asyncio.Event()

        log.info('Client connected...')

        async def ensure_deregistration():
            log.debug("Waiting for connection closed event to be set.")
            await connection_closed_event.wait()
            log.debug("Client disconnected! We'll remove the connection from our active connections.")
            self.deregister_connection(connection=conn)
            if client_is_camera.is_set():
                self.daemon.remove_camera(conn.peer)
            elif client_is_ticket_dispatcher.is_set():
                self.daemon.remove_dispatcher(conn.peer)

        asyncio.ensure_future(ensure_deregistration())

        try:
            while True:
                frame = await conn.frame_receiver.get()
                log.debug(f'Received frame: {frame}')
                # First frame should always be one of IAmCamera, IAmDispatcher, or a WantHeartbeat.
                match frame.kind:
                    case MessageType.IAmCamera:
                        data: IAmCamera = frame.data  # noqa
                        if client_is_camera.is_set() or client_is_ticket_dispatcher.is_set():
                            await conn.frame_sender.put(Error(msg="already identified as camera").as_frame())
                            return await conn.close()
                        self.register_connection(conn)
                        self.daemon.add_camera(conn.peer, road=data.road, mile=data.mile, limit=data.limit)
                        client_is_camera.set()
                    case MessageType.IAmDispatcher:
                        data: IAmDispatcher = frame.data  # noqa
                        if client_is_camera.is_set() or client_is_ticket_dispatcher.is_set():
                            await conn.frame_sender.put(Error(msg="already identified as dispatcher").as_frame())
                            return await conn.close()
                        self.register_connection(conn)
                        self.daemon.add_dispatcher(conn.peer, roads=data.roads)
                        client_is_ticket_dispatcher.set()
                    case MessageType.WantHeartbeat:
                        data: WantHeartbeat = frame.data  # noqa
                        if conn.want_heartbeats.is_set():
                            await conn.frame_sender.put(Error(msg="already requested heartbeats").as_frame())
                            return await conn.close()
                        conn.set_heartbeat_interval(data.interval / 10)
                    case MessageType.Plate:
                        data: Plate = frame.data  # noqa
                        log.debug(f"Plate message received: {data}")
                        if not client_is_camera.is_set():
                            await conn.frame_sender.put(Error(msg="only cameras can send plates").as_frame())
                            return await conn.close()
                        self.daemon.record_car(data.plate, data.timestamp, conn.peer)
                    case other:
                        await conn.frame_sender.put(Error(msg=f"received unexpected message type: {other!r}").as_frame())
                        return await conn.close()
        except InvalidFrame as exc:
            log.error(f"Could not determine client kind: {exc}")
            await conn.frame_sender.put(Error(msg=f"invalid frame: {exc}").as_frame())
            return await conn.close()
        except ValueError as exc:
            log.error(f"Could not determine client kind: {exc}")
            await conn.frame_sender.put(Error(msg=f"invalid frame: {exc}").as_frame())
            return await conn.close()
