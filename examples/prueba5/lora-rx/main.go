package main

import (
	"machine"
	"time"
)

const (
	pinSCK  = machine.GPIO4
	pinMISO = machine.GPIO5
	pinMOSI = machine.GPIO6
	pinSS   = machine.GPIO7
	pinRST  = machine.GPIO3
)

func main() {
	time.Sleep(2 * time.Second)
	println("LoRa Receiver (ESP32-C3 / TinyGo)")

	err := machine.SPI2.Configure(machine.SPIConfig{
		Frequency: 4000000,
		SCK:       pinSCK,
		SDO:       pinMOSI,
		SDI:       pinMISO,
		Mode:      0,
	})
	if err != nil {
		for {
			println("SPI error:", err.Error())
			time.Sleep(time.Second)
		}
	}

	radio := NewSX1278(machine.SPI2, pinSS, pinRST)
	err = radio.Begin(Config{
		Freq:     433000000,
		SF:       9,
		BW:       125000,
		CR:       5,
		SyncWord: 0x12,
		TxPower:  17,
		CRC:      false,
	})
	if err != nil {
		for {
			println("Starting LoRa failed:", err.Error())
			time.Sleep(time.Second)
		}
	}
	println("LoRa init OK")

	radio.StartReceive()
	for {
		pkt, err := radio.Receive()
		if err != nil {
			println("Rx error:", err.Error())
		} else if pkt != nil {
			println("Recibido:", string(pkt),
				" (RSSI:", radio.PacketRSSI(), "dBm)")
		}
		time.Sleep(5 * time.Millisecond)
	}
}
