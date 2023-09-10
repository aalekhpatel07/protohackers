package server

import (
	"errors"
	"fmt"
	"log"
	"net"
	"speed_daemon/clients"
	"speed_daemon/messages"
	"sync"
)

type SpeedDaemon struct {
	Cameras     map[net.Addr]clients.Camera
	Dispatchers map[net.Addr]clients.Dispatcher
	mu          sync.Mutex
	connections map[net.Addr]net.Conn
	database    *Database
}

func NewSpeedDaemon(database *Database) *SpeedDaemon {
	daemon := SpeedDaemon{
		connections: make(map[net.Addr]net.Conn),
		Dispatchers: make(map[net.Addr]clients.Dispatcher),
		Cameras:     make(map[net.Addr]clients.Camera),
		database:    database,
	}
	return &daemon
}

type MessageWithSender struct {
	message messages.MessageImpl
	peer    net.Addr
}
type MessageWithRecipient struct {
	message   messages.MessageImpl
	recipient net.Addr
}
type FailClientMessage struct {
	msg       string
	recipient net.Addr
}

func (daemon *SpeedDaemon) HandleConnections(
	acceptedConnections <-chan net.Conn,
	messageBus chan<- MessageWithSender,
) {
	for {
		connection := <-acceptedConnections
		go func() {
			for {
				message, err := messages.Deserialize(connection)
				if err != nil {
					daemon.failConnectionWithError(fmt.Sprintf("could not deserialize message: %s", err), connection)
					return
				}
				if !message.Payload.IsClientOrigin() {
					daemon.failConnectionWithError("not expecting clients to send this message", connection)
					return
				}

				_, found := daemon.connections[connection.RemoteAddr()]
				if !found {
					daemon.mu.Lock()
					daemon.connections[connection.RemoteAddr()] = connection
					daemon.mu.Unlock()
				}

				messageBus <- MessageWithSender{
					message: message,
					peer:    connection.RemoteAddr(),
				}
			}
		}()
	}
}

func (daemon *SpeedDaemon) failConnectionWithError(msg string, connection net.Conn) {
	// Try to close connection at the end.
	defer func(connection net.Conn) {
		err := connection.Close()
		// Remove this connection from map.
		daemon.mu.Lock()
		defer daemon.mu.Unlock()
		delete(daemon.connections, connection.RemoteAddr())
		if err != nil {
			return
		}

	}(connection)

	// Send an error message.
	message := messages.Error{
		Msg: msg,
	}
	_ = message.Serialize(connection)
}

func (daemon *SpeedDaemon) WatchForClientsToFail(
	failClient <-chan FailClientMessage) {
	for {
		message := <-failClient
		log.Printf("Will fail client: %s", message)

		daemon.mu.Lock()
		connection, found := daemon.connections[message.recipient]
		daemon.mu.Unlock()
		// we don't know about this connection. just ignore the disconnection request.
		if !found {
			continue
		}
		daemon.failConnectionWithError(message.msg, connection)
	}
}

func (daemon *SpeedDaemon) FindDispatcherForRoad(road Road) (net.Addr, bool) {
	for peer, dispatcher := range daemon.Dispatchers {
		for roadIdx := range dispatcher.Roads {
			storedRoad := dispatcher.Roads[roadIdx]
			if storedRoad == road {
				// found a dispatcher we can send this ticket to.
				return peer, true
			}
		}
	}
	return &net.TCPAddr{}, false
}

func (daemon *SpeedDaemon) SendTickets(tickets <-chan messages.Ticket) {
	for {
		ticket := <-tickets

		// Try to send a ticket to some dispatcher. If none available, ask the db to store it.
		// It'll eventually be picked up for retransmission when possible.
		peer, found := daemon.FindDispatcherForRoad(ticket.Road)
		if !found {
			daemon.database.StoreTicket(ticket)
			continue
		}
		// we don't care about any errors when sending ticket messages.
		// if the dispatcher drops connection, the ticket is lost.
		_ = daemon.TrySendMessage(peer, messages.MessageImpl{Payload: ticket})
	}
}

func (daemon *SpeedDaemon) TrySendMessage(peer net.Addr, message messages.MessageImpl) error {
	conn, found := daemon.connections[peer]
	if !found {
		return errors.New("no connection found for given peer addr")
	}
	return message.Serialize(conn)
}

func (daemon *SpeedDaemon) ProcessMessages(
	messageBus <-chan MessageWithSender,
	failClient chan<- FailClientMessage,
) {
	for {
		box := <-messageBus
		peer := box.peer
		message := box.message

		switch payload := message.Payload.(type) {
		case messages.IAmCamera:
			{
				_, foundCamera := daemon.Cameras[peer]
				_, foundDispatcher := daemon.Dispatchers[peer]
				if foundCamera || foundDispatcher {
					// already identified. fail client.
					failClient <- FailClientMessage{
						msg:       "You've already identified yourself wtf.",
						recipient: peer,
					}
					continue
				}
				daemon.mu.Lock()
				daemon.Cameras[peer] = clients.Camera{
					Road:  payload.Road,
					Limit: payload.Limit,
					Mile:  payload.Mile,
				}
				daemon.mu.Unlock()
			}
		case messages.IAmDispatcher:
			{
				_, foundCamera := daemon.Cameras[peer]
				_, foundDispatcher := daemon.Dispatchers[peer]
				if foundCamera || foundDispatcher {
					// already identified. fail client.
					failClient <- FailClientMessage{
						msg:       "You've already identified yourself wtf.",
						recipient: peer,
					}
					continue
				}
				daemon.mu.Lock()
				daemon.Dispatchers[peer] = clients.Dispatcher{
					Roads:               payload.Roads,
					WantHeartbeatActive: false,
				}
				daemon.mu.Unlock()
				log.Printf("Acknowledged an IAmDispatcher message: %+v", payload)
			}
		case messages.Plate:
			{
				// if we got a plate message, we must know about this camera.
				// in case we do not, fail this client.
				camera, found := daemon.Cameras[peer]
				if !found {
					failClient <- FailClientMessage{
						msg:       "You're not a known camera. Please identify yourselves before sending plates.",
						recipient: peer,
					}
					continue
				}

				// Now we know we got this from a camera.
				daemon.database.AcknowledgePlateByCamera(&payload, &camera)
			}
		case messages.WantHeartbeat:
			{
				// if we've already received a WantHeartbeat from this client,
				// tell them its an error.
				cameraFound := false

				camera, found := daemon.Cameras[peer]
				if found {
					cameraFound = true
					if camera.WantHeartbeatActive {
						failClient <- FailClientMessage{
							msg:       "You've already requested heartbeats so please shut up.",
							recipient: peer,
						}
						continue
					}
				}
				// otherwise check if this client is a dispatcher and has already received a WantHeartbeat.
				dispatcher, found := daemon.Dispatchers[peer]
				dispatcherFound := false

				if found {
					dispatcherFound = true
					if dispatcher.WantHeartbeatActive {
						failClient <- FailClientMessage{
							msg:       "You've already requested heartbeats so please shut up.",
							recipient: peer,
						}
						continue
					}
				}

				if !dispatcherFound && !cameraFound {
					// unknown client.
					failClient <- FailClientMessage{
						msg:       "You're not a known client. Please identify yourselves before requesting heartbeats.",
						recipient: peer,
					}
					continue
				}
				if dispatcherFound {
					dispatcher.SubscribeHeartbeat(payload.Interval)
				} else if cameraFound {
					camera.SubscribeHeartbeat(payload.Interval)
				}
			}
		}
	}
}
