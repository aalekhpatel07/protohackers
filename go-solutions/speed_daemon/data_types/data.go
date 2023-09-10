package data_types

import (
	"encoding/binary"
	"errors"
	"fmt"
	"io"
)

func ParseUInt8(reader io.Reader) (uint8, error) {
	var res uint8
	err := binary.Read(reader, binary.BigEndian, &res)
	if err != nil {
		return 0, err
	}
	return res, nil
}

func WriteUInt8(num uint8, writer io.Writer) error {
	return binary.Write(writer, binary.BigEndian, num)
}

func ParseUInt16(reader io.Reader) (uint16, error) {
	var res uint16
	err := binary.Read(reader, binary.BigEndian, &res)
	if err != nil {
		return 0, err
	}
	return res, nil
}

func WriteUInt16(num uint16, writer io.Writer) error {
	return binary.Write(writer, binary.BigEndian, num)
}

func ParseUInt32(reader io.Reader) (uint32, error) {
	var res uint32
	err := binary.Read(reader, binary.BigEndian, &res)
	if err != nil {
		return 0, err
	}
	return res, nil
}
func WriteUInt32(num uint32, writer io.Writer) error {
	return binary.Write(writer, binary.BigEndian, num)
}

func ParseStr(reader io.Reader) (string, error) {
	strLen, err := ParseUInt8(reader)
	if err != nil {
		return "", err
	}
	strBuffer := make([]byte, strLen)
	err = binary.Read(reader, binary.BigEndian, &strBuffer)
	if err != nil {
		return "", err
	}
	return string(strBuffer), nil
}

func WriteStr(s string, writer io.Writer) error {
	strlen := len(s)
	err := WriteUInt8(uint8(strlen), writer)
	if err != nil {
		return err
	}
	if strlen == 0 {
		return nil
	}

	bytesWritten, err := writer.Write([]byte(s))
	if err != nil {
		return err
	}
	if bytesWritten != strlen {
		errorDetail := fmt.Sprintf("failed to write the entire string. (written/total): (%d/%d)", bytesWritten, strlen)
		return errors.New(errorDetail)
	}
	return nil
}
