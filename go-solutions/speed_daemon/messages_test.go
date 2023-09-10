package main

import (
	"bytes"
	"github.com/stretchr/testify/assert"
	"speed_daemon/messages"
	"testing"
)

func TestSerdeTicket(t *testing.T) {

	testCases := []struct {
		serialized []byte
		ticket     messages.Ticket
	}{
		{serialized: []byte{0x21, 0x04, 0x55, 0x4e, 0x31, 0x58, 0x00, 0x42, 0x00, 0x64, 0x00, 0x01, 0xe2, 0x40, 0x00, 0x6e, 0x00, 0x01, 0xe3, 0xa8, 0x27, 0x10}, ticket: messages.Ticket{
			Plate:      "UN1X",
			Road:       66,
			Mile1:      100,
			Timestamp1: 123456,
			Mile2:      110,
			Timestamp2: 123816,
			Speed:      10000,
		}},
		{serialized: []byte{0x21, 0x07, 0x52, 0x45, 0x30, 0x35, 0x42, 0x4b, 0x47, 0x01, 0x70, 0x04, 0xd2, 0x00, 0x0f, 0x42, 0x40, 0x04, 0xd3, 0x00, 0x0f, 0x42, 0x7c, 0x17, 0x70}, ticket: messages.Ticket{
			Plate:      "RE05BKG",
			Road:       368,
			Mile1:      1234,
			Timestamp1: 1000000,
			Mile2:      1235,
			Timestamp2: 1000060,
			Speed:      6000,
		}},
	}

	for idx := range testCases {
		serialized := testCases[idx].serialized
		ticket := testCases[idx].ticket

		// test ser.
		asSerialized := bytes.NewBuffer(make([]byte, 0))
		err := ticket.Serialize(asSerialized)

		if err != nil {
			t.Fail()
		}
		assert.Equal(t, serialized[1:], asSerialized.Bytes())

		// test de.
		toDeserialize := bytes.NewBuffer(serialized[1:])
		deserialized, err := messages.DeserializeTicket(toDeserialize)
		if err != nil {
			t.Fail()
		}
		assert.Equal(t, deserialized, ticket)
	}

}

func TestSerdeMessage(t *testing.T) {

	testCases := []struct {
		serialized []byte
		ticket     messages.Ticket
	}{
		{serialized: []byte{0x21, 0x04, 0x55, 0x4e, 0x31, 0x58, 0x00, 0x42, 0x00, 0x64, 0x00, 0x01, 0xe2, 0x40, 0x00, 0x6e, 0x00, 0x01, 0xe3, 0xa8, 0x27, 0x10}, ticket: messages.Ticket{
			Plate:      "UN1X",
			Road:       66,
			Mile1:      100,
			Timestamp1: 123456,
			Mile2:      110,
			Timestamp2: 123816,
			Speed:      10000,
		}},
		{serialized: []byte{0x21, 0x07, 0x52, 0x45, 0x30, 0x35, 0x42, 0x4b, 0x47, 0x01, 0x70, 0x04, 0xd2, 0x00, 0x0f, 0x42, 0x40, 0x04, 0xd3, 0x00, 0x0f, 0x42, 0x7c, 0x17, 0x70}, ticket: messages.Ticket{
			Plate:      "RE05BKG",
			Road:       368,
			Mile1:      1234,
			Timestamp1: 1000000,
			Mile2:      1235,
			Timestamp2: 1000060,
			Speed:      6000,
		}},
	}

	for idx := range testCases {
		serialized := testCases[idx].serialized
		ticket := testCases[idx].ticket

		message := messages.MessageImpl{
			Payload: ticket,
		}

		// test ser.
		asSerialized := bytes.NewBuffer(make([]byte, 0))
		err := message.Serialize(asSerialized)

		if err != nil {
			t.Fail()
		}
		assert.Equal(t, serialized, asSerialized.Bytes())

		// test de.
		toDeserialize := bytes.NewBuffer(serialized)
		deserialized, err := messages.Deserialize(toDeserialize)
		if err != nil {
			t.Fail()
		}
		assert.Equal(t, deserialized.Payload, ticket)
		assert.Equal(t, deserialized.Payload.Kind(), messages.TicketKind)
	}
}

func TestSerdeIAmCameraMessage(t *testing.T) {

	testCases := []struct {
		serialized []byte
		message    messages.IAmCamera
	}{
		{serialized: []byte{0x80, 0x00, 0x42, 0x00, 0x64, 0x00, 0x3c}, message: messages.IAmCamera{
			Road:  66,
			Mile:  100,
			Limit: 60,
		}},
		{serialized: []byte{0x80, 0x01, 0x70, 0x04, 0xd2, 0x00, 0x28}, message: messages.IAmCamera{
			Road:  368,
			Mile:  1234,
			Limit: 40,
		}},
	}

	for idx := range testCases {
		serialized := testCases[idx].serialized
		message := testCases[idx].message

		messageImpl := messages.MessageImpl{
			Payload: message,
		}

		// test ser.
		asSerialized := bytes.NewBuffer(make([]byte, 0))
		err := messageImpl.Serialize(asSerialized)

		if err != nil {
			t.Fail()
		}
		assert.Equal(t, serialized, asSerialized.Bytes())

		// test de.
		toDeserialize := bytes.NewBuffer(serialized)
		deserialized, err := messages.Deserialize(toDeserialize)
		if err != nil {
			t.Fail()
		}
		assert.Equal(t, deserialized.Payload, message)
		assert.Equal(t, deserialized.Payload.Kind(), messages.IAmCameraKind)
	}
}

func TestSerdeIAmDispatcherMessage(t *testing.T) {

	testCases := []struct {
		serialized []byte
		message    messages.IAmDispatcher
	}{
		{serialized: []byte{0x81, 0x01, 0x00, 0x42}, message: messages.IAmDispatcher{
			NumRoads: 1,
			Roads:    []uint16{66},
		}},
		{serialized: []byte{0x81, 0x03, 0x00, 0x42, 0x01, 0x70, 0x13, 0x88}, message: messages.IAmDispatcher{
			NumRoads: 3,
			Roads:    []uint16{66, 368, 5000},
		}},
	}

	for idx := range testCases {
		serialized := testCases[idx].serialized
		message := testCases[idx].message

		messageImpl := messages.MessageImpl{
			Payload: message,
		}

		// test ser.
		asSerialized := bytes.NewBuffer(make([]byte, 0))
		err := messageImpl.Serialize(asSerialized)

		if err != nil {
			t.Fail()
		}
		assert.Equal(t, serialized, asSerialized.Bytes())

		// test de.
		toDeserialize := bytes.NewBuffer(serialized)
		deserialized, err := messages.Deserialize(toDeserialize)
		if err != nil {
			t.Fail()
		}
		assert.Equal(t, deserialized.Payload, message)
		assert.Equal(t, deserialized.Payload.Kind(), messages.IAmDispatcherKind)
	}
}

func TestSerdeWantHeartbeat(t *testing.T) {

	testCases := []struct {
		serialized []byte
		message    messages.WantHeartbeat
	}{
		{serialized: []byte{0x40, 0x00, 0x00, 0x00, 0x0a}, message: messages.WantHeartbeat{
			Interval: 10,
		}},
		{serialized: []byte{0x40, 0x00, 0x00, 0x04, 0xdb}, message: messages.WantHeartbeat{
			Interval: 1243,
		}},
	}

	for idx := range testCases {
		serialized := testCases[idx].serialized
		message := testCases[idx].message

		messageImpl := messages.MessageImpl{
			Payload: message,
		}

		// test ser.
		asSerialized := bytes.NewBuffer(make([]byte, 0))
		err := messageImpl.Serialize(asSerialized)

		if err != nil {
			t.Fail()
		}
		assert.Equal(t, serialized, asSerialized.Bytes())

		// test de.
		toDeserialize := bytes.NewBuffer(serialized)
		deserialized, err := messages.Deserialize(toDeserialize)
		if err != nil {
			t.Fail()
		}
		assert.Equal(t, deserialized.Payload, message)
		assert.Equal(t, deserialized.Payload.Kind(), messages.WantHeartbeatKind)
	}
}

func TestSerdeHeartbeat(t *testing.T) {

	testCases := []struct {
		serialized []byte
		message    messages.Heartbeat
	}{
		{serialized: []byte{0x41}, message: messages.Heartbeat{}},
	}

	for idx := range testCases {
		serialized := testCases[idx].serialized
		message := testCases[idx].message

		messageImpl := messages.MessageImpl{
			Payload: message,
		}

		// test ser.
		asSerialized := bytes.NewBuffer(make([]byte, 0))
		err := messageImpl.Serialize(asSerialized)

		if err != nil {
			t.Fail()
		}
		assert.Equal(t, serialized, asSerialized.Bytes())

		// test de.
		toDeserialize := bytes.NewBuffer(serialized)
		deserialized, err := messages.Deserialize(toDeserialize)
		if err != nil {
			t.Fail()
		}
		assert.Equal(t, deserialized.Payload, message)
		assert.Equal(t, deserialized.Payload.Kind(), messages.HeartbeatKind)
	}
}

func TestSerdePlate(t *testing.T) {

	testCases := []struct {
		serialized []byte
		message    messages.Plate
	}{
		{serialized: []byte{0x20, 0x04, 0x55, 0x4e, 0x31, 0x58, 0x00, 0x00, 0x03, 0xe8}, message: messages.Plate{
			Plate:     "UN1X",
			Timestamp: 1000,
		}},
		{serialized: []byte{0x20, 0x07, 0x52, 0x45, 0x30, 0x35, 0x42, 0x4b, 0x47, 0x00, 0x01, 0xe2, 0x40}, message: messages.Plate{
			Plate:     "RE05BKG",
			Timestamp: 123456,
		}},
	}

	for idx := range testCases {
		serialized := testCases[idx].serialized
		message := testCases[idx].message

		messageImpl := messages.MessageImpl{
			Payload: message,
		}

		// test ser.
		asSerialized := bytes.NewBuffer(make([]byte, 0))
		err := messageImpl.Serialize(asSerialized)

		if err != nil {
			t.Fail()
		}
		assert.Equal(t, serialized, asSerialized.Bytes())

		// test de.
		toDeserialize := bytes.NewBuffer(serialized)
		deserialized, err := messages.Deserialize(toDeserialize)
		if err != nil {
			t.Fail()
		}
		assert.Equal(t, deserialized.Payload, message)
		assert.Equal(t, deserialized.Payload.Kind(), messages.PlateKind)
	}
}

func TestSerdeError(t *testing.T) {

	testCases := []struct {
		serialized []byte
		message    messages.Error
	}{
		{serialized: []byte{0x10, 0x03, 0x62, 0x61, 0x64}, message: messages.Error{
			Msg: "bad",
		}},
		{serialized: []byte{0x10, 0x0b, 0x69, 0x6c, 0x6c, 0x65, 0x67, 0x61, 0x6c, 0x20, 0x6d, 0x73, 0x67}, message: messages.Error{
			Msg: "illegal msg",
		}},
	}

	for idx := range testCases {
		serialized := testCases[idx].serialized
		message := testCases[idx].message

		messageImpl := messages.MessageImpl{
			Payload: message,
		}

		// test ser.
		asSerialized := bytes.NewBuffer(make([]byte, 0))
		err := messageImpl.Serialize(asSerialized)

		if err != nil {
			t.Fail()
		}
		assert.Equal(t, serialized, asSerialized.Bytes())

		// test de.
		toDeserialize := bytes.NewBuffer(serialized)
		deserialized, err := messages.Deserialize(toDeserialize)
		if err != nil {
			t.Fail()
		}
		assert.Equal(t, deserialized.Payload, message)
		assert.Equal(t, deserialized.Payload.Kind(), messages.ErrorKind)
	}
}
