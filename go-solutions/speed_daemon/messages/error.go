package messages

import (
	"io"
	"log"
	"speed_daemon/data_types"
)

type Error struct {
	Msg string
}

func (message Error) Kind() MessageType {
	return ErrorKind
}
func (message Error) IsClientOrigin() bool {
	return false
}

func NewError(msg string) Error {
	return Error{
		Msg: msg,
	}
}

func (message Error) Serialize(writer io.Writer) error {
	log.Printf("Serializing error: %s", message)
	return data_types.WriteStr(message.Msg, writer)
}

func DeserializeError(reader io.Reader) (Error, error) {
	msg, err := data_types.ParseStr(reader)
	if err != nil {
		return Error{}, nil
	}
	return NewError(msg), nil
}
