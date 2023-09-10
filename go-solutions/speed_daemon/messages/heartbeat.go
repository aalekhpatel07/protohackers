package messages

import (
	"io"
	"speed_daemon/data_types"
)

type WantHeartbeat struct {
	Interval uint32
}

func (message WantHeartbeat) Kind() MessageType {
	return WantHeartbeatKind
}
func (message WantHeartbeat) IsClientOrigin() bool {
	return true
}

func (message WantHeartbeat) Serialize(writer io.Writer) error {
	return data_types.WriteUInt32(message.Interval, writer)
}

func DeserializeWantHeartbeat(reader io.Reader) (WantHeartbeat, error) {
	interval, err := data_types.ParseUInt32(reader)
	if err != nil {
		return WantHeartbeat{}, err
	}
	return WantHeartbeat{Interval: interval}, nil
}

type Heartbeat struct{}

func (message Heartbeat) Kind() MessageType {
	return HeartbeatKind
}
func (message Heartbeat) IsClientOrigin() bool {
	return false
}

func (message Heartbeat) Serialize(writer io.Writer) error {
	return nil
}

func DeserializeHeartbeat(_ io.Reader) (Heartbeat, error) {
	return Heartbeat{}, nil
}
