package messages

import (
	"errors"
	"fmt"
	"io"
	"speed_daemon/data_types"
)

type MessageType uint8

const (
	ErrorKind         MessageType = 0x10
	PlateKind         MessageType = 0x20
	TicketKind        MessageType = 0x21
	WantHeartbeatKind MessageType = 0x40
	HeartbeatKind     MessageType = 0x41
	IAmCameraKind     MessageType = 0x80
	IAmDispatcherKind MessageType = 0x81
)

type Message interface {
	Kind() MessageType
	IsClientOrigin() bool
}

type MessageImpl struct {
	Payload Message
}

type Ser interface {
	Serialize(writer io.Writer) error
}

func (message MessageImpl) Serialize(writer io.Writer) error {
	err := data_types.WriteUInt8(uint8(message.Payload.Kind()), writer)
	if err != nil {
		return err
	}
	// eh.. ADT alternative. looks sus but we kinda need that type switch.
	// https://eli.thegreenplace.net/2018/go-and-algebraic-data-types/
	switch payload := message.Payload.(type) {
	case Error:
		err = payload.Serialize(writer)
	case Plate:
		err = payload.Serialize(writer)
	case Ticket:
		err = payload.Serialize(writer)
	case WantHeartbeat:
		err = payload.Serialize(writer)
	case Heartbeat:
		err = payload.Serialize(writer)
	case IAmCamera:
		err = payload.Serialize(writer)
	case IAmDispatcher:
		err = payload.Serialize(writer)
	default:
		return errors.New(fmt.Sprintf("Unrecognized payload: %s", payload))
	}

	return err
}

func Deserialize(reader io.Reader) (MessageImpl, error) {

	kind, err := data_types.ParseUInt8(reader)
	if err != nil {
		return MessageImpl{}, errors.New("failed to parse message kind")
	}
	var payload Message

	switch kind {
	case uint8(ErrorKind):
		payload, err = DeserializeError(reader)
	case uint8(PlateKind):
		payload, err = DeserializePlate(reader)
	case uint8(TicketKind):
		payload, err = DeserializeTicket(reader)
	case uint8(WantHeartbeatKind):
		payload, err = DeserializeWantHeartbeat(reader)
	case uint8(HeartbeatKind):
		payload, err = DeserializeHeartbeat(reader)
	case uint8(IAmCameraKind):
		payload, err = DeserializeIAmCamera(reader)
	case uint8(IAmDispatcherKind):
		payload, err = DeserializeIAmDispatcher(reader)
	default:
		return MessageImpl{}, errors.New(fmt.Sprintf("Failed to parse message kind: %d", kind))
	}
	if err != nil {
		return MessageImpl{}, err
	}
	return MessageImpl{Payload: payload}, err
}
