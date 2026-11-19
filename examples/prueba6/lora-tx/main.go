package main

import (
	"time"

	"tinygo.org/x/drivers/sx127x"
)

func main() {
	time.Sleep(2 * time.Second)
	println("LoRa Sender OK")

	radio := setupRadio()

	go func() {
		n := 0
		for {
			n++
			// Limpiar IRQ flags antes de cada transmisión
			radio.WriteRegister(sx127x.SX127X_REG_IRQ_FLAGS, 0xFF)

 			msg := []byte("Hello World #" + itoa(n))
			err := radio.Tx(msg, 1000)
			if err != nil {
				println("Tx error:", err.Error())
			} else {
				println("Enviado:", string(msg))
				time.Sleep(50 * time.Millisecond) // Esperar a que el chip vuelva a STANDBY
			}
			time.Sleep(1000 * time.Millisecond)
		}
	}()

	for {
		time.Sleep(1 * time.Second)
	}
}
