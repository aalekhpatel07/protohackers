package messages

import (
	"io"
	"speed_daemon/data_types"
)

type IAmDispatcher struct {
	NumRoads uint8
	Roads    []uint16
}

func (message IAmDispatcher) Kind() MessageType {
	return IAmDispatcherKind
}
func (message IAmDispatcher) IsClientOrigin() bool {
	return true
}

func (message IAmDispatcher) Serialize(writer io.Writer) error {
	err := data_types.WriteUInt8(message.NumRoads, writer)
	if err != nil {
		return err
	}
	for idx := range message.Roads {
		road := message.Roads[idx]
		err = data_types.WriteUInt16(road, writer)
		if err != nil {
			return err
		}
	}
	return nil
}

func DeserializeIAmDispatcher(reader io.Reader) (IAmDispatcher, error) {
	numRoads, err := data_types.ParseUInt8(reader)
	if err != nil {
		return IAmDispatcher{}, err
	}
	roads := make([]uint16, 0)

	for i := 0; i < int(numRoads); i++ {
		road, err := data_types.ParseUInt16(reader)
		if err != nil {
			return IAmDispatcher{}, err
		}
		roads = append(roads, road)
	}
	return IAmDispatcher{NumRoads: numRoads, Roads: roads}, nil
}
