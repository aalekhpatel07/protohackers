import typing
import dataclasses
import structlog


logger = structlog.get_logger(__name__)

type Peer = typing.Tuple[str, int]


@dataclasses.dataclass
class Established:
    peer: Peer


@dataclasses.dataclass
class DroppedByPeer:
    peer: Peer


@dataclasses.dataclass
class ShouldDropPeer:
    peer: Peer


@dataclasses.dataclass
class BadFrameRead:
    peer: Peer


@dataclasses.dataclass
class StateMachine[T]:
    peer: Peer
    data: T


type Message[T] = Established | DroppedByPeer | ShouldDropPeer | BadFrameRead | StateMachine[T]
