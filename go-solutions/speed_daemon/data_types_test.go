package main

import (
	"bytes"
	"github.com/stretchr/testify/assert"
	"speed_daemon/data_types"
	"testing"
)

func TestParseUInt8(t *testing.T) {

	testCases := []struct {
		input    []byte
		expected uint8
	}{
		{[]byte{0x20}, 32},
		{[]byte{0xe3}, 227},
	}

	for idx := range testCases {
		input := testCases[idx].input
		expected := testCases[idx].expected
		buffer := bytes.NewBuffer(input)
		res, err := data_types.ParseUInt8(buffer)
		if err != nil {
			t.Fail()
		}
		assert.Equal(t, res, uint8(expected))
	}
	for idx := range testCases {
		expectedBytes := testCases[idx].input
		input := testCases[idx].expected
		buffer := bytes.NewBuffer(make([]byte, 0))
		err := data_types.WriteUInt8(input, buffer)
		if err != nil {
			t.Fail()
		}
		assert.Equal(t, expectedBytes, buffer.Bytes())
	}
}

func TestParseUInt16(t *testing.T) {

	testCases := []struct {
		input    []byte
		expected uint16
	}{
		{[]byte{0x00, 0x20}, 32},
		{[]byte{0x12, 0x45}, 4677},
		{[]byte{0xa8, 0x23}, 43043},
	}

	for idx := range testCases {
		input := testCases[idx].input
		expected := testCases[idx].expected
		buffer := bytes.NewBuffer(input)
		res, err := data_types.ParseUInt16(buffer)
		if err != nil {
			t.Fail()
		}
		assert.Equal(t, res, expected)
	}

	for idx := range testCases {
		expectedBytes := testCases[idx].input
		input := testCases[idx].expected
		buffer := bytes.NewBuffer(make([]byte, 0))
		err := data_types.WriteUInt16(input, buffer)
		if err != nil {
			t.Fail()
		}
		assert.Equal(t, expectedBytes, buffer.Bytes())
	}
}

func TestParseUInt32(t *testing.T) {

	testCases := []struct {
		input    []byte
		expected uint32
	}{
		{[]byte{0x00, 0x00, 0x00, 0x20}, 32},
		{[]byte{0x00, 0x00, 0x12, 0x45}, 4677},
		{[]byte{0xa6, 0xa9, 0xb5, 0x67}, 2796139879},
	}

	for idx := range testCases {
		input := testCases[idx].input
		expected := testCases[idx].expected
		buffer := bytes.NewBuffer(input)
		res, err := data_types.ParseUInt32(buffer)
		if err != nil {
			t.Fail()
		}
		assert.Equal(t, res, expected)
	}

	for idx := range testCases {
		expectedBytes := testCases[idx].input
		input := testCases[idx].expected
		buffer := bytes.NewBuffer(make([]byte, 0))
		err := data_types.WriteUInt32(input, buffer)
		if err != nil {
			t.Fail()
		}
		assert.Equal(t, expectedBytes, buffer.Bytes())
	}
}

func TestParseStr(t *testing.T) {

	testCases := []struct {
		input    []byte
		expected string
	}{
		{[]byte{0x00}, ""},
		{[]byte{0x03, 0x66, 0x6f, 0x6f}, "foo"},
		{[]byte{0x08, 0x45, 0x6c, 0x62, 0x65, 0x72, 0x65, 0x74, 0x68}, "Elbereth"},
	}

	for idx := range testCases {
		input := testCases[idx].input
		expected := testCases[idx].expected
		buffer := bytes.NewBuffer(input)
		res, err := data_types.ParseStr(buffer)
		if err != nil {
			t.Fail()
		}
		assert.Equal(t, res, expected)
	}

	for idx := range testCases {
		expectedBytes := testCases[idx].input
		inputStr := testCases[idx].expected
		buffer := bytes.NewBuffer(make([]byte, 0))
		err := data_types.WriteStr(inputStr, buffer)
		if err != nil {
			t.Fail()
		}
		assert.Equal(t, expectedBytes, buffer.Bytes())
	}
}
