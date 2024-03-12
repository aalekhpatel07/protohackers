import contextlib
import typing
import sqlalchemy
from sqlalchemy import (
    Column,
    DOUBLE,
    Integer,
    String,
    ForeignKey,
    create_engine,
    select,
)
from sqlalchemy.orm import (
    DeclarativeBase,
    Mapped,
    mapped_column,
    relationship,
    sessionmaker,
    Session as SessionCore
)

engine = create_engine(
    # "sqlite:////home/infinity/dev/protohackers/python-solutions/tests/example.db",
    "sqlite://",
    pool_recycle=3600,
    isolation_level="SERIALIZABLE",
    # echo=True
)

Session = sessionmaker(engine)


@contextlib.contextmanager
def session_scope() -> typing.Generator[SessionCore, None, None]:
    session = Session()
    try:
        session.begin()
        yield session
    except:  # noqa
        session.rollback()  # noqa
        raise
    else:
        session.commit()  # noqa


class Base(DeclarativeBase):
    pass


class GetOrCreateMixin:
    @classmethod
    def get_or_create(cls, session, **kwargs) -> typing.Tuple[typing.Self, bool]:
        stmt = select(cls).filter_by(**kwargs)
        instance = session.execute(stmt).first()
        if instance is not None:
            return instance[0], False
        instance = cls(**kwargs)  # noqa
        session.add(instance)
        return instance, True


class Road(Base, GetOrCreateMixin):
    __tablename__ = 'road'
    id: Mapped[int] = mapped_column(primary_key=True)
    dispatchers: Mapped[typing.List["TicketDispatcher"]] = relationship(
        # "ticket_dispatcher",
        back_populates="roads",
        secondary="road_dispatchers",
        # cascade="all, delete-orphan",
    )
    cameras: Mapped[typing.List["Camera"]] = relationship(
        back_populates="road"
    )


class RoadDispatcher(Base):
    __tablename__ = "road_dispatchers"
    id: Mapped[int] = mapped_column(primary_key=True)
    road_id: Mapped[int] = mapped_column(ForeignKey("road.id"))
    ticket_dispatcher_id: Mapped[int] = mapped_column(ForeignKey("ticket_dispatcher.id"))


class Camera(Base, GetOrCreateMixin):
    __tablename__ = 'camera'
    id: Mapped[int] = mapped_column(primary_key=True)
    client: Mapped[str] = mapped_column(String(32))

    road_id: Mapped[int] = mapped_column(ForeignKey("road.id"))
    road: Mapped["Road"] = relationship(back_populates="cameras")

    observations: Mapped[typing.List["SpeedObservation"]] = relationship(back_populates="camera")

    mile: Mapped[int] = mapped_column(Integer())
    limit: Mapped[float] = mapped_column(DOUBLE())


class TicketDispatcher(Base, GetOrCreateMixin):
    __tablename__ = "ticket_dispatcher"
    id: Mapped[int] = mapped_column(primary_key=True)
    client: Mapped[str] = mapped_column(String(32))
    roads: Mapped[typing.List["Road"]] = relationship(
        # "road",
        back_populates="dispatchers",
        secondary="road_dispatchers",
        # cascade="all, delete-orphan",
    )

    @staticmethod
    def all_dispatchers(session):
        stmt = select(TicketDispatcher)

        for row in session.execute(stmt).fetchall():
            yield row[0]


class SpeedObservation(Base, GetOrCreateMixin):
    __tablename__ = "speed_observation"
    id: Mapped[int] = mapped_column(primary_key=True)
    plate: Mapped[str] = mapped_column(String(255))
    timestamp: Mapped[int] = mapped_column(Integer())

    camera_id: Mapped[int] = mapped_column(ForeignKey("camera.id"))
    camera: Mapped["Camera"] = relationship(back_populates="observations")


Base.metadata.create_all(engine)
