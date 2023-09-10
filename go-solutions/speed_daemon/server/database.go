package server

import (
	"speed_daemon/clients"
	"speed_daemon/messages"
	"sync"
)

type Road = uint16
type Plate = string

type CarRecord struct {
	Timestamp uint32
	Mile      uint16
}

type Database struct {
	mu            sync.Mutex
	observations  map[Plate]map[Road][]CarRecord
	newTicket     chan<- messages.Ticket
	unsentTickets []messages.Ticket
}

func NewDatabase() (*Database, <-chan messages.Ticket) {
	newTickets := make(chan messages.Ticket)
	db := Database{
		newTicket: newTickets,
	}
	return &db, newTickets
}

func binarySearch(records []CarRecord, timestamp uint32) int {
	low := 0
	high := len(records) - 1
	for {
		if low > high {
			break
		}
		mid := (low + high) >> 1
		midValue := records[mid]
		if midValue.Timestamp < timestamp {
			low = mid + 1
		} else if midValue.Timestamp > timestamp {
			high = mid - 1
		} else {
			return mid
		}
	}
	return -(low + 1)
}

func (database *Database) AcknowledgePlateByCamera(plate *messages.Plate, camera *clients.Camera) {
	database.mu.Lock()
	defer database.mu.Unlock()
	roadObservations, found := database.observations[plate.Plate]
	if !found {
		database.observations[plate.Plate] = make(map[Road][]CarRecord)
		roadObservations, _ = database.observations[plate.Plate]
	}
	carRecords, found := roadObservations[camera.Road]
	if !found {
		roadObservations[camera.Road] = make([]CarRecord, 0)
		carRecords, _ = roadObservations[camera.Road]
	}

	if len(carRecords) == 0 {
		carRecords = append(carRecords, CarRecord{
			Timestamp: plate.Timestamp,
			Mile:      camera.Mile,
		})
		return
	}

	insertIndex := binarySearch(carRecords, plate.Timestamp)

	// WTF is this, Go! why can't there just be a simple slice.InsertAt(elem, index).
	carRecords = append(carRecords[:insertIndex], append([]CarRecord{{
		Timestamp: plate.Timestamp,
		Mile:      camera.Mile,
	}}, carRecords[insertIndex:]...)...)

	roadObservations[camera.Road] = carRecords
}

func (database *Database) Run() {

}

func (Database *Database) StoreTicket(ticket messages.Ticket) {

}
