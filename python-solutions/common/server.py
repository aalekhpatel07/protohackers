import collections
import typing
import asyncio
from .message import (
    Message,
    Established,
    StateMachine,
    DroppedByPeer,
    ShouldDropPeer,
    BadFrameRead,
    Peer,
)
from .frame import (
    Framer,
    Frame,
)
import structlog


logger = structlog.get_logger(__name__)


# async def _guarded_cb(reader: asyncio.StreamReader, writer: asyncio.StreamWriter):
#     addr, port = writer.get_extra_info('peername')
#     if opts.allowed_ips and addr not in opts.allowed_ips:
#         logger.warn(
#             f"Rejecting connection attempt from {addr!r} "
#             f"because it is not an allowed ip.",
#             allowed_ips=opts.allowed_ips
#         )
#         writer.close()
#         return await writer.wait_closed()
#     return await actual_handler(reader, writer)


class Server:
    # Messages for a StateMachine.
    message_sender: asyncio.Queue[Message[Frame]]
    # Messages from a StateMachine.
    message_receiver: asyncio.Queue[Message[Frame]]

    peers: typing.DefaultDict[Peer, Framer]
    peers_lock: asyncio.Lock

    allowed_ips: typing.Iterable[str]

    def __init__(self,
                 message_sender: asyncio.Queue[Message[Frame]],
                 message_receiver: asyncio.Queue[Message[Frame]],
                 frame_cls,
                 allowed_ips: typing.Iterable[str] = None,
                 ):
        self.message_sender = message_sender
        self.message_receiver = message_receiver
        self.frame_cls = frame_cls
        if allowed_ips is None:
            self.allowed_ips = [
                "127.0.0.1",  # for testing against local clients.
                "206.189.113.124"  # The real protohackers client: https://protohackers.com/security
            ]

        self.peers = collections.defaultdict()
        self.peers_lock = asyncio.Lock()

        # Ensure that messages from the StateMachine
        # are sent to the corresponding peers.
        asyncio.ensure_future(self.handle_outbound_messages())

    async def try_drop_peer(self, peer: Peer) -> bool:
        async with self.peers_lock:
            if peer not in self.peers:
                return False
            framer = self.peers[peer]
            await framer.frame_writer.put(None)
            return True

    async def try_send_to_peer(self, peer: Peer, frame: Frame) -> bool:
        async with self.peers_lock:
            if peer not in self.peers:
                return False
            framer = self.peers[peer]
            await framer.frame_writer.put(frame)
            return True

    async def handle_outbound_messages(self):
        """Handle messages coming from the StateMachine
        running this connection server.

        :return:
        """
        while True:
            match await self.message_receiver.get():
                case ShouldDropPeer(peer):
                    peer_dropped = await self.try_drop_peer(peer)
                    if peer_dropped:
                        logger.info(
                            f"Requested to shut down the frame writer for peer: {peer}",
                            peer=peer,
                        )
                    else:
                        logger.error(
                            f"Unknown peer: {peer} when requested to shut down the frame writer for.",
                            peer=peer
                        )
                case StateMachine(peer, data):
                    sent_to_peer = await self.try_send_to_peer(peer, data)
                    if sent_to_peer:
                        logger.debug(
                            f"Sent StateMachine message to peer: ({data!r}) -> {peer!r}",
                            data=data,
                            peer=peer,
                        )
                    else:
                        logger.error(
                            f"Unknown peer: {peer} when requested to send a StateMachine message to.",
                            peer=peer,
                            data=data,
                        )
                case msg:
                    raise ValueError(f"{msg} is not a valid outbound message.")

    async def _check_allowed_ips(self, writer: asyncio.StreamWriter):
        addr, port = writer.get_extra_info('peername')
        if not self.allowed_ips:
            return
        if addr not in self.allowed_ips:
            logger.warn(
                f"Rejecting connection attempt from {addr!r} "
                f"because it is not an allowed ip.",
                allowed_ips=self.allowed_ips
            )
            writer.close()
            return await writer.wait_closed()

    async def handle_connection(self,
                                reader: asyncio.StreamReader,
                                writer: asyncio.StreamWriter,
                                ):
        await self._check_allowed_ips(writer)
        incoming_frames: asyncio.Queue[Frame] = asyncio.Queue()
        outgoing_frames: asyncio.Queue[Frame] = asyncio.Queue()

        # When this gets set, we'll let the state machine
        # know that the connection was closed.
        connection_closed_by_peer = asyncio.Event()

        # When framer sets this, we'll let the state machine know
        # that a bad frame was read.
        bad_frame_read = asyncio.Event()

        framer = Framer(
            reader,
            writer,
            frame_reader=incoming_frames,
            frame_writer=outgoing_frames,
            connection_closed_by_peer=connection_closed_by_peer,
            bad_frame_read=bad_frame_read,
            frame_cls=self.frame_cls,
        )

        async with self.peers_lock:
            self.peers[framer.peer] = framer
        logger.info(f"Connection established {framer.peer}", peer=framer.peer)

        # Let the state machine know that the peer established a connection.
        await self.message_sender.put(Established(peer=framer.peer))
        logger.info("Notified state machine of a new connection")

        # Let the state machine know the peer dropped connection
        # when it is time.

        async def notify_state_machine_of_connection_closed():
            await connection_closed_by_peer.wait()
            logger.debug(f"Connection was closed by peer.", peer=framer.peer)
            await self.message_sender.put(DroppedByPeer(framer.peer))

        asyncio.ensure_future(notify_state_machine_of_connection_closed())

        # Let the state machine know a bad frame was read when that happens.
        async def notify_state_machine_of_invalid_frame():
            # Framer couldn't make sense of the data.
            await bad_frame_read.wait()
            logger.warning(f"Invalid frame from {framer.peer}", peer=framer.peer)
            await self.message_sender.put(BadFrameRead(peer=framer.peer))

        asyncio.ensure_future(notify_state_machine_of_invalid_frame())

        # Forward all messages to StateMachine.
        async def propagate_incoming_frames():
            while True:
                frame = await incoming_frames.get()
                await self.message_sender.put(StateMachine(peer=framer.peer, data=frame))

        asyncio.ensure_future(propagate_incoming_frames())
        await framer.run_to_completion()
