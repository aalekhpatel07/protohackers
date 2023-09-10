package messages

import (
	"io"
	"speed_daemon/data_types"
)

type IAmCamera struct {
	Road  uint16
	Mile  uint16
	Limit uint16
}

func (message IAmCamera) Kind() MessageType {
	return IAmCameraKind
}
func (message IAmCamera) IsClientOrigin() bool {
	return true
}

func (message IAmCamera) Serialize(writer io.Writer) error {
	err := data_types.WriteUInt16(message.Road, writer)
	if err != nil {
		return err
	}
	err = data_types.WriteUInt16(message.Mile, writer)
	if err != nil {
		return err
	}
	err = data_types.WriteUInt16(message.Limit, writer)
	if err != nil {
		return err
	}
	return nil
}

func DeserializeIAmCamera(reader io.Reader) (IAmCamera, error) {
	road, err := data_types.ParseUInt16(reader)
	if err != nil {
		return IAmCamera{}, err
	}
	mile, err := data_types.ParseUInt16(reader)
	if err != nil {
		return IAmCamera{}, err
	}
	limit, err := data_types.ParseUInt16(reader)
	if err != nil {
		return IAmCamera{}, err
	}
	return IAmCamera{Road: road, Mile: mile, Limit: limit}, nil
}
