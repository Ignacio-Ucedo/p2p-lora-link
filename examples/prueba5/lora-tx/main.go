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
	// DIO0 (GPIO10) no hace falta: se consulta el registro de IRQ por SPI
)

func main() {
	time.Sleep(2 * time.Second)
	println("LoRa Sender (ESP32-C3 / TinyGo)")

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

	msg := []byte("Hello World")
	for {
		if err := radio.Send(msg); err != nil {
			println("Send error:", err.Error())
		}
		time.Sleep(100 * time.Millisecond)
	}
}
