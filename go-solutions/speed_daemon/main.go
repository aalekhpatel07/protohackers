package main

import (
	"fmt"
	"log"
	"net"
	"speed_daemon/server"
)

func StartServer(port uint16, acceptedConnection chan<- net.Conn) {
	addr := fmt.Sprintf("0.0.0.0:%d", port)
	listener, err := net.Listen("tcp", addr)
	if err != nil {
		log.Fatal(err)
	}

	defer func(listener net.Listener) {
		err := listener.Close()
		if err != nil {
			log.Fatal(err)
		}
	}(listener)

	for {
		conn, err := listener.Accept()
		if err != nil {
			log.Fatal(err)
		}
		acceptedConnection <- conn
	}
}

func main() {
	acceptedConnections := make(chan net.Conn)
	database, newTickets := server.NewDatabase()
	daemon := server.NewSpeedDaemon(database)
	messageBus := make(chan server.MessageWithSender)
	failClient := make(chan server.FailClientMessage)
	go database.Run()
	go daemon.HandleConnections(acceptedConnections, messageBus)
	go daemon.ProcessMessages(messageBus, failClient)
	go daemon.WatchForClientsToFail(failClient)
	go daemon.SendTickets(newTickets)
	StartServer(9001, acceptedConnections)
}
