package messages

import (
	"io"
	"speed_daemon/data_types"
)

type Ticket struct {
	Plate      string
	Road       uint16
	Mile1      uint16
	Timestamp1 uint32
	Mile2      uint16
	Timestamp2 uint32
	Speed      uint16
}

func (message Ticket) Kind() MessageType {
	return TicketKind
}
func (message Ticket) IsClientOrigin() bool {
	return false
}

func (message Ticket) Serialize(writer io.Writer) error {
	err := data_types.WriteStr(message.Plate, writer)
	if err != nil {
		return err
	}

	err = data_types.WriteUInt16(message.Road, writer)
	if err != nil {
		return err
	}

	err = data_types.WriteUInt16(message.Mile1, writer)
	if err != nil {
		return err
	}
	err = data_types.WriteUInt32(message.Timestamp1, writer)
	if err != nil {
		return err
	}
	err = data_types.WriteUInt16(message.Mile2, writer)
	if err != nil {
		return err
	}
	err = data_types.WriteUInt32(message.Timestamp2, writer)
	if err != nil {
		return err
	}
	err = data_types.WriteUInt16(message.Speed, writer)
	if err != nil {
		return err
	}
	return nil
}

func DeserializeTicket(reader io.Reader) (Ticket, error) {
	plate, err := data_types.ParseStr(reader)
	if err != nil {
		return Ticket{}, err
	}
	road, err := data_types.ParseUInt16(reader)
	if err != nil {
		return Ticket{}, err
	}
	mile1, err := data_types.ParseUInt16(reader)
	if err != nil {
		return Ticket{}, err
	}

	timestamp1, err := data_types.ParseUInt32(reader)
	if err != nil {
		return Ticket{}, err
	}
	mile2, err := data_types.ParseUInt16(reader)
	if err != nil {
		return Ticket{}, err
	}
	timestamp2, err := data_types.ParseUInt32(reader)
	if err != nil {
		return Ticket{}, err
	}
	speed, err := data_types.ParseUInt16(reader)
	if err != nil {
		return Ticket{}, err
	}
	return Ticket{
		Plate:      plate,
		Road:       road,
		Mile1:      mile1,
		Mile2:      mile2,
		Timestamp2: timestamp2,
		Timestamp1: timestamp1,
		Speed:      speed,
	}, nil
}
