import pytest
from solutions.speed_daemon.state import (
    SpeedDaemon,
    TicketDispatcher,
    Camera,
    SpeedObservation,
)
from solutions.speed_daemon.models import (
    session_scope,
    Session,
)


@pytest.fixture
def session():
    with session_scope() as sess:
        yield sess


def test_single_car(session):
    state = SpeedDaemon(session)

    state.add_camera("1", 9822, 8100, 100)
    state.add_camera("2", 9822, 8090, 100)
    state.record_car('ZM78NBG', 366166, "1")
    state.add_dispatcher("3", [49822])
    state.record_car('ZM78NBG', 365866, "1")

    dispatchers = state.get_dispatchers()
    assert len(dispatchers) == 1
    assert dispatchers[0].client == "3"
    assert dispatchers[0].roads[0].id == 49822

    assert session.query(Camera).count() == 2
    assert session.query(Camera.mile).order_by(Camera.client).all() == [(8100,), (8090,)]
    assert session.query(Camera.limit).order_by(Camera.client).all() == [(100,), (100,)]

    assert session.query(SpeedObservation).count() == 2

    state.remove_camera("1")
    assert session.query(Camera).count() == 1

    state.remove_dispatcher("3")
    assert session.query(TicketDispatcher).count() == 0
