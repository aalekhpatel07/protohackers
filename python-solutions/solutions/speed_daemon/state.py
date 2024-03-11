import dataclasses
import typing
from collections import defaultdict
import structlog


logger = structlog.get_logger(__name__)


# type ID = typing.Tuple[str, int]


@dataclasses.dataclass
class Camera[ID]:
    id: ID
    road: int
    mile: int
    limit: float


@dataclasses.dataclass
class TicketDispatcher[ID]:
    id: ID
    roads: typing.List[int] = dataclasses.field(default_factory=list)


@dataclasses.dataclass
class SpeedDaemon[ID]:
    dispatchers: typing.Dict[ID, TicketDispatcher] = dataclasses.field(default_factory=dict)
    cameras: typing.Dict[ID, Camera] = dataclasses.field(default_factory=dict)

    def add_dispatcher(self, dispatcher_id: ID, roads: typing.List[int]):
        self.dispatchers[dispatcher_id] = TicketDispatcher(dispatcher_id, roads)
        logger.info(f"Added dispatcher", id=dispatcher_id, value=self.dispatchers[dispatcher_id])

    def remove_dispatcher(self, dispatcher_id: ID):
        if dispatcher_id in self.dispatchers:
            prev = self.dispatchers.pop(dispatcher_id)
            logger.info(f"Removed dispatcher", id=prev.id, value=prev)
        else:
            logger.warn(f"Dispatcher not found", id=dispatcher_id)

    def add_camera(self, camera_id: ID, road: int, mile: int, limit: float):
        self.cameras[camera_id] = Camera(camera_id, road, mile, limit)
        logger.info(f"Added camera", id=camera_id, value=self.cameras[camera_id])

    def remove_camera(self, camera_id: ID):
        if camera_id in self.cameras:
            prev = self.cameras.pop(camera_id)
            logger.info(f"Removed camera", id=prev.id, value=prev)
        else:
            logger.warn(f"Camera not found", id=camera_id)

    def record_car(self, plate: str, timestamp: int, camera_id: ID):
        self.cars_by_camera[camera_id][timestamp] = plate
        logger.info("Recorded car", camera_id=camera_id, car_timestamp=timestamp, plate=plate)

    def check_for_tickets(self):

        return
