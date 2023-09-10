package clients

type Camera struct {
	// The road that this Camera is located at.
	Road uint16
	// The position of the Camera on the road.
	Mile uint16
	// In miles per hour.
	Limit               uint16
	WantHeartbeatActive bool
}

func NewCamera(
	road uint16,
	mile uint16,
	limit uint16,
) Camera {
	return Camera{
		Road:  road,
		Mile:  mile,
		Limit: limit,
	}
}

func (camera *Camera) Run() {

}

func (camera *Camera) SubscribeHeartbeat(interval uint32) {
	camera.WantHeartbeatActive = true
}
