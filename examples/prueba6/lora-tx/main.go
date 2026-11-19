package main

import (
	"time"

	"tinygo.org/x/drivers/sx127x"
)

func main() {
	time.Sleep(2 * time.Second)
	println("LoRa Sender diag")

	radio := setupRadio()

	go func() {
		n := 0
		for {
			n++
			println("Tx...")
			msg := []byte("Hello World #" + itoa(n))
			err := radio.Tx(msg, 1000)
			if err != nil {
				println("Tx error:", err.Error())
			} else {
				println("Tx ok")
			}
			time.Sleep(1000 * time.Millisecond)
		}
	}()

	for {
		time.Sleep(500 * time.Millisecond)
		println("DIO0=", pinDIO0.Get(),
			" irq=", radio.ReadRegister(sx127x.SX127X_REG_IRQ_FLAGS),
			" opmode=", radio.ReadRegister(sx127x.SX127X_REG_OP_MODE))
	}
}
