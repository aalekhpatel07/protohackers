package messages

import (
	"io"
	"speed_daemon/data_types"
)

type Plate struct {
	Plate     string
	Timestamp uint32
}

func (message Plate) Kind() MessageType {
	return PlateKind
}
func (message Plate) IsClientOrigin() bool {
	return true
}

func (message Plate) Serialize(writer io.Writer) error {
	err := data_types.WriteStr(message.Plate, writer)
	if err != nil {
		return err
	}
	return data_types.WriteUInt32(message.Timestamp, writer)
}

func DeserializePlate(reader io.Reader) (Plate, error) {
	plate, err := data_types.ParseStr(reader)
	if err != nil {
		return Plate{}, err
	}
	timestamp, err := data_types.ParseUInt32(reader)
	if err != nil {
		return Plate{}, err
	}
	return Plate{
		Plate:     plate,
		Timestamp: timestamp,
	}, nil
}
