import dataclasses
import typing
import structlog
from .models import (
    session_scope,
    Road,
    TicketDispatcher,
    Camera,
    RoadDispatcher,
    SpeedObservation,
)

logger = structlog.get_logger(__name__)


class SpeedDaemon:

    def __init__(self, session):
        self.session = session

    def get_dispatchers(self) -> typing.List[TicketDispatcher]:
        return list(TicketDispatcher.all_dispatchers(self.session))

    def add_dispatcher(self, client: str, roads: typing.List[int]):
        dispatcher, added = TicketDispatcher.get_or_create(self.session, client=client)
        if added:
            logger.info("Added Ticket Dispatcher: ", client=client, roads=roads)
        road_instances: typing.List[Road] = []
        for road in roads:
            road_ins, added = Road.get_or_create(self.session, id=road)
            if added:
                logger.info("Added Road", client=client, road=road_ins.id)
            road_instances.append(road_ins)
        self.session.add_all(
            [
                RoadDispatcher(road_id=road.id, ticket_dispatcher_id=dispatcher.id)
                for road in road_instances
            ]
        )

    def remove_dispatcher(self, client: str):
        entry = self.session.query(TicketDispatcher).filter_by(client=client)
        if entry.first() is not None:
            logger.info("Removing Ticket Dispatcher: ", client=client, dispatcher_id=entry.first().id)
        entry.delete()
        # if dispatcher_id in self.dispatchers:
        #     prev = self.dispatchers.pop(dispatcher_id)
        #     logger.info(f"Removed dispatcher", id=prev.id, value=prev)
        # else:
        #     logger.warn(f"Dispatcher not found", id=dispatcher_id)

    def add_camera(self, client: str, road: int, mile: int, limit: float):
        road, added = Road.get_or_create(self.session, id=road)
        if added:
            logger.info(f"Added Road", id=road.id, client=client)
        camera, added = Camera.get_or_create(self.session, client=client, limit=limit, mile=mile, road=road)

        if added:
            logger.info(f"Added Camera", id=camera.id, client=camera.client, limit=limit, mile=mile, road=road.id)

    def remove_camera(self, client: str):
        self.session.query(Camera).filter_by(client=client).delete()
        # if camera_id in self.cameras:
        #     prev = self.cameras.pop(camera_id)
        #     logger.info(f"Removed camera", id=prev.id, value=prev)
        # else:
        #     logger.warn(f"Camera not found", id=camera_id)

    def record_car(self, plate: str, timestamp: int, client: str):
        camera = self.session.query(Camera).filter_by(client=client).one()
        instance, created = SpeedObservation.get_or_create(self.session, plate=plate, timestamp=timestamp, camera_id=camera.id)
        if created:
            logger.info("Added Observation", plate=plate, timestamp=timestamp, client=client, camera_id=camera.id)

    def check_for_tickets(self):

        return
