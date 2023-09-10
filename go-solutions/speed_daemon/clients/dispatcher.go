package clients

type Dispatcher struct {
	WantHeartbeatActive bool
	Roads               []uint16
}

func (dispatcher *Dispatcher) SubscribeHeartbeat(interval uint32) {

	dispatcher.WantHeartbeatActive = true
}
