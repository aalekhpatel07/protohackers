import pytest
import asyncio
from solutions.speed_daemon.messages import (
    Message,
    Plate,
)
from solutions.speed_daemon.state import (
    SpeedDaemon,
    TicketDispatcher,
    Camera
)


@pytest.mark.asyncio
async def test_single_car():
    state = SpeedDaemon[int]()
    state.add_camera(1, 9822, 8100, 100)
    state.add_camera(2, 9822, 8090, 100)
    state.record_car('ZM78NBG', 366166, 1)
    state.add_dispatcher(1, [49822])
    state.record_car('ZM78NBG', 365866, 2)

    # state.add_camera(2, 10, 3, 12.0)
    pass
