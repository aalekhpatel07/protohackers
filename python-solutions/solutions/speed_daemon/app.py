import logging
import sys
import typing

from common.server import (
    Server,
)
from common.message import (
    Message as ServerMessage,
    Established,
    BadFrameRead,
    DroppedByPeer,
    ShouldDropPeer,
    StateMachine,
)
from common.frame import (
    Peer,
    Frame,
)
from .state_machine.models import (
    User,
    Camera,
    Dispatcher,
    Observation,
    Ticket as TicketModel,
    database,
)
from .messages import (
    Message,
    Ticket,
    IAmDispatcher,
    IAmCamera,
    WantHeartbeat,
    Heartbeat,
    Plate,
    Error,
)
from datetime import datetime, UTC
import structlog
import asyncio
from peewee import fn, Expression


logger = structlog.get_logger(__name__)


def div(lhs, rhs):
    return Expression(lhs, '/', rhs)


class App:

    # Messages for us sent by server.
    message_receiver: asyncio.Queue[ServerMessage[Message]]

    # Messages we want to send to the server.
    message_sender: asyncio.Queue[ServerMessage[Message]]

    server: Server

    def __init__(self):

        message_to_server = asyncio.Queue()
        message_from_server = asyncio.Queue()

        self.message_receiver = message_from_server
        self.message_sender = message_to_server

        self.server = Server(
            message_sender=message_from_server,
            message_receiver=message_to_server,
            frame_cls=Message
        )

    async def handle_messages_from_server(self):
        while True:
            logger.debug("Waiting for ServerMessages")
            match await self.message_receiver.get():
                case Established(peer):
                    await self.handle_peer_connected(peer)
                case DroppedByPeer(peer):
                    await self.handle_peer_disconnected(peer)
                case BadFrameRead(peer):
                    await self.handle_bad_frame_read(peer)
                case StateMachine(peer, message):
                    await self.handle_state_machine_message(peer, message)
                case other:
                    raise ValueError(f"Unknown ServerMessage type: {other!r}")

    async def handle_peer_connected(self, peer: Peer):  # noqa
        with database.atomic():
            user = User.get_or_create(
                peer=f"{peer[0]}:{peer[1]}",
                connected_at=datetime.now(UTC),
                disconnected_at=None
            )
        logger.info(f"Created user: {user!r}")

    async def handle_peer_disconnected(self, peer: Peer):  # noqa
        logger.info("Handling peer disconnection.", peer=peer)
        return await self.close_peer(peer)

    async def close_peer(self, peer: Peer):
        with database.atomic():
            user = self.get_user(peer)
            if user is None:
                return
            if user.disconnected_at is None:
                user.disconnected_at = datetime.now(UTC)
                user.save()
                logger.info(f"Deactivated user: {user!r}")

    def get_user(self, peer: Peer) -> typing.Optional[User]:  # noqa
        return (
            User
            .get_or_none(
                User.peer == f"{peer[0]}:{peer[1]}",
                User.disconnected_at.is_null(True)
            )
        )

    def get_camera(self, peer: Peer) -> typing.Optional[Camera]:
        user = self.get_user(peer)
        if user is None:
            return
        return (
            Camera
            .get_or_none(
                user=user
            )
        )

    def get_dispatcher(self, peer: Peer) -> typing.Optional[Dispatcher]:
        user = self.get_user(peer)
        if user is None:
            return
        return (
            Dispatcher
            .get_or_none(
                user=user
            )
        )

    def set_dispatcher(self, peer: Peer, **kwargs) -> Dispatcher:
        with database.atomic():
            return Dispatcher.create(
                user=self.get_user(peer),
                **kwargs
            )

    def set_camera(self, peer: Peer, **kwargs) -> Camera:
        with database.atomic():
            return Camera.create(
                user=self.get_user(peer),
                **kwargs
            )

    async def handle_bad_frame_read(self, peer: Peer):
        await self.message_sender.put(
            StateMachine(peer=peer, data=Message.from_data(Error(msg="BAD_FRAME")))
        )
        await self.message_sender.put(ShouldDropPeer(peer=peer))
        await self.close_peer(peer)

    async def schedule_heartbeats(self, peer: Peer, interval: int):
        if interval == 0:
            return
        while True:
            await self.message_sender.put(
                StateMachine(peer=peer, data=Message.from_data(Heartbeat()))
            )
            await asyncio.sleep(interval / 10)
            user = self.get_user(peer)
            if user is None or user.disconnected_at is not None:
                return

    async def handle_state_machine_message(self, peer: Peer, message: Message):
        match message.data:
            case WantHeartbeat(interval):
                user = self.get_user(peer)
                # It is an error to request heartbeats after already requesting it once.
                if user.heartbeat_requested_at is not None:
                    await self.message_sender.put(StateMachine(peer=peer, data=Message.from_data(Error(msg="ALREADY_IDENTIFIED"))))
                    await self.message_sender.put(ShouldDropPeer(peer=peer))
                    return

                # Otherwise schedule the heartbeat sender.
                asyncio.ensure_future(self.schedule_heartbeats(peer, interval))
                with database.atomic():
                    user.heartbeat_requested_at = datetime.now(tz=UTC)
                    user.save()

            case IAmCamera(road, mile, limit):
                # If peer is already a camera or a dispatcher, then this is an error.
                if any((self.get_camera(peer), self.get_dispatcher(peer))):
                    await self.message_sender.put(StateMachine(peer=peer, data=Message.from_data(Error(msg="ALREADY_IDENTIFIED"))))
                    await self.message_sender.put(ShouldDropPeer(peer=peer))
                    return
                instance = self.set_camera(peer, road=road, mile=mile, limit=limit)
                logger.info(f"Peer identified to be a camera: {instance!r}", peer=peer)
                return

            case IAmDispatcher(_num_roads, roads):
                # If peer is already a camera or a dispatcher, then this is an error.
                if any((self.get_camera(peer), self.get_dispatcher(peer))):
                    await self.message_sender.put(StateMachine(peer=peer, data=Message.from_data(Error(msg="ALREADY_IDENTIFIED"))))
                    await self.message_sender.put(ShouldDropPeer(peer=peer))
                    return
                instance = self.set_dispatcher(peer, roads=roads)
                logger.info(f"Peer identified to be a ticket dispatcher: {instance!r}", peer=peer)
                tickets = self.check_for_deliverable_tickets(instance)
                for ticket in tickets:
                    if not await self.try_deliver_ticket(ticket):
                        logger.warn("No dispatcher found for ticket", ticket=ticket)
            case Plate(plate, timestamp):
                # Only camera can send these messages.
                camera = self.get_camera(peer)
                if camera is None:
                    await self.message_sender.put(StateMachine(peer=peer, data=Message.from_data(Error(msg="NOT_A_CAMERA"))))
                    await self.message_sender.put(ShouldDropPeer(peer=peer))
                    return
                with database.atomic():
                    observation = Observation(
                        camera=camera,
                        plate=plate,
                        timestamp=datetime.fromtimestamp(timestamp, tz=UTC)
                    )
                    tickets = self.check_for_new_tickets(observation)
                    for ticket in tickets:
                        if not await self.try_deliver_ticket(ticket):
                            logger.warn("No dispatcher found for ticket", ticket=ticket)
            case other:
                await self.message_sender.put(StateMachine(peer=peer, data=Message.from_data(Error(msg="UNKNOWN_MESSAGE"))))
                await self.message_sender.put(ShouldDropPeer(peer=peer))
                logger.error(f"Unknown message: {other!r}", peer=peer)
                return

    @staticmethod
    def average_speed_between_observations(ob1: Observation, ob2: Observation):
        if ob2.timestamp > ob1.timestamp:
            duration = (ob2.timestamp - ob1.timestamp).total_seconds()
            distance = ob2.camera.mile - ob1.camera.mile
        else:
            duration = (ob1.timestamp - ob2.timestamp).total_seconds()
            distance = ob1.camera.mile - ob2.camera.mile
        return distance / (duration / 3600)

    @staticmethod
    def order_observations_by_timestamp(ob1: Observation, ob2: Observation):
        if ob2.timestamp < ob1.timestamp:
            return ob2, ob1
        return ob1, ob2

    def check_for_new_tickets(self, observation: Observation):
        # If adding this observation would cause creation of a ticket,
        # then there must exist other observations for this car on that road.
        with database.atomic():
            existing_observations = (
                Observation
                .select()
                .join(Camera)
                .where(
                    Observation.plate == observation.plate,
                    Camera.road == observation.camera.road
                )
                .order_by(Observation.timestamp.asc())
            )
            if existing_observations.first() is None:
                logger.info(
                    "This is the first observation for this car on this road.",
                    plate=observation.plate,
                    road=observation.camera.road,
                    timestamp=observation.timestamp,
                )
                observation.save()
                return []

            logger.debug("Must check other observations if this creates a ticket.")
            tickets = []

            # If a ticket for this car exists on the day this observation was recorded,
            # then don't generate any more tickets for this observation.
            day = int(observation.timestamp.timestamp() / 86400)
            query = (
                TicketModel
                .select()
                .where(
                    TicketModel.plate == observation.plate,
                    fn.Floor(div(TicketModel.timestamp1.to_timestamp(), 86400)) <= day,
                    fn.Floor(div(TicketModel.timestamp2.to_timestamp(), 86400)) >= day
                )
            )
            query_str, params = query.sql()
            logger.info(query_str % tuple(params), plate=observation.plate, road=observation.camera.road)

            ticket_for_this_car_exists_on_this_day = query.first()
            if ticket_for_this_car_exists_on_this_day:
                logger.info(
                    "Ticket for this car exists on this day",
                    plate=observation.plate,
                    day=day,
                    ticket=ticket_for_this_car_exists_on_this_day
                )
                logger.info("This observation won't generate any notices then.")
                return []

            with database.atomic():
                for ob in existing_observations:
                    speed_mph = self.average_speed_between_observations(observation, ob)
                    # all observations are from the same road,
                    # so they all have same speed limit.
                    if speed_mph >= observation.camera.limit:
                        older, newer = self.order_observations_by_timestamp(observation, ob)
                        logger.debug(
                            "Found an existing observation for this car "
                            "that violates the speed limit on this road.",
                            plate=observation.plate,
                            road=ob.camera.road,
                            limit=observation.camera.limit,
                            mile1=older.camera.mile,
                            timestamp1=older.timestamp.timestamp(),
                            mile2=newer.camera.mile,
                            timestamp2=newer.timestamp.timestamp(),
                        )
                        # exceeding the limit, give ticket.
                        ticket = TicketModel.create(
                            plate=observation.plate,
                            road=observation.camera.road,
                            mile1=older.camera.mile,
                            timestamp1=older.timestamp,
                            mile2=newer.camera.mile,
                            timestamp2=newer.timestamp,
                            speed=speed_mph,
                            delivered_to=None,
                            delivered_at=None,
                        )
                        tickets.append(ticket)
            return tickets

    async def try_deliver_ticket(self, ticket: TicketModel) -> bool:
        with database.atomic():
            dispatcher = (
                Dispatcher
                .select()
                .where(
                    Dispatcher.roads.contains(ticket.road)
                ).first()
            )
            if dispatcher is None:
                return False

            addr, port = dispatcher.user.peer.split(":")
            peer = (addr, int(port))

            ticket.delivered_at = datetime.now(UTC)
            ticket.delivered_to = dispatcher
            ticket.save()

        await self.message_sender.put(
            StateMachine(
                peer=peer,
                data=Message.from_data(
                    Ticket(
                        plate=ticket.plate,
                        road=ticket.road,
                        mile1=ticket.mile1,
                        timestamp1=int(ticket.timestamp1.timestamp()),
                        mile2=ticket.mile2,
                        timestamp2=int(ticket.timestamp2.timestamp()),
                        speed=int(round(ticket.speed * 100))
                    )
                )
            )
        )
        return True

    @staticmethod
    def check_for_deliverable_tickets(dispatcher: Dispatcher):
        # If there are any pending tickets for a dispatcher,
        # then return them.
        tickets = []
        with database.atomic():
            deliverable_tickets = (
                TicketModel
                .select()
                .where(
                    TicketModel.delivered_to.is_null(True),
                    TicketModel.road.in_(dispatcher.roads)
                )
            )
            for ticket in deliverable_tickets:
                ticket.delivered_to = dispatcher
                ticket.delivered_at = datetime.now(UTC)
                ticket.save()
                tickets.append(ticket)
        return tickets

    async def run(self):
        asyncio.ensure_future(self.handle_messages_from_server())
