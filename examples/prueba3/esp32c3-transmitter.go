package prueba3

import (
	"machine"
	"time"
)

const (
	pinSCK  = machine.GPIO0
	pinMISO = machine.GPIO1
	pinMOSI = machine.GPIO10
	pinCS   = machine.GPIO20
	pinRST  = machine.GPIO3
)

func xfer(b byte) byte {
	var r byte
	for i := 0; i < 8; i++ {
		pinMOSI.Set(b&0x80 != 0)
		b <<= 1
		pinSCK.High()
		r <<= 1
		if pinMISO.Get() {
			r |= 1
		}
		pinSCK.Low()
	}
	return r
}

func readReg(addr byte) byte {
	pinCS.Low()
	xfer(addr & 0x7F)
	v := xfer(0)
	pinCS.High()
	return v
}

func writeReg(addr, val byte) {
	pinCS.Low()
	xfer(addr | 0x80)
	xfer(val)
	pinCS.High()
}

// Escritura en ráfaga al FIFO (una sola transacción SPI)
func writeFifo(data []byte) {
	pinCS.Low()
	xfer(0x00 | 0x80)
	for _, c := range data {
		xfer(c)
	}
	pinCS.High()
}

func initLoRa() bool {
	pinRST.Low()
	time.Sleep(10 * time.Millisecond)
	pinRST.High()
	time.Sleep(20 * time.Millisecond)

	v := readReg(0x42)
	println("RegVersion =", v, "(esperado 18)")
	if v != 0x12 {
		return false
	}

	writeReg(0x01, 0x80) // LoRa + sleep
	time.Sleep(10 * time.Millisecond)

	frf := (uint64(433_000_000) << 19) / 32_000_000
	writeReg(0x06, byte(frf>>16))
	writeReg(0x07, byte(frf>>8))
	writeReg(0x08, byte(frf))

	writeReg(0x0E, 0x00) // FIFO TX base
	writeReg(0x0F, 0x00) // FIFO RX base

	writeReg(0x1D, 0x72) // BW 125k, CR 4/5, header explícito
	writeReg(0x1E, 0x70) // SF7, CRC off (igual que el receptor)
	writeReg(0x26, 0x04) // AGC automático
	writeReg(0x31, 0xC3)
	writeReg(0x37, 0x0A)
	writeReg(0x20, 0x00)
	writeReg(0x21, 0x08) // preámbulo 8
	writeReg(0x39, 0x12) // sync word

	writeReg(0x4D, 0x84) // PA DAC normal (<=17 dBm)
	writeReg(0x09, 0x8F) // PA_BOOST, 17 dBm

	writeReg(0x01, 0x81) // standby
	return true
}

func send(msg []byte) bool {
	writeReg(0x01, 0x81) // standby
	writeReg(0x0D, 0x00) // FIFO ptr = 0
	writeFifo(msg)
	writeReg(0x22, byte(len(msg)))
	writeReg(0x12, 0xFF) // limpiar IRQ
	writeReg(0x01, 0x83) // TX

	for i := 0; i < 500; i++ { // timeout ~5 s
		if readReg(0x12)&0x08 != 0 { // TxDone
			writeReg(0x12, 0x08)
			return true
		}
		time.Sleep(10 * time.Millisecond)
	}
	return false
}

func main() {
	time.Sleep(3 * time.Second)
	println("Transmisor LoRa (ESP32-C3)")

	pinSCK.Configure(machine.PinConfig{Mode: machine.PinOutput})
	pinMOSI.Configure(machine.PinConfig{Mode: machine.PinOutput})
	pinMISO.Configure(machine.PinConfig{Mode: machine.PinInput})
	pinCS.Configure(machine.PinConfig{Mode: machine.PinOutput})
	pinRST.Configure(machine.PinConfig{Mode: machine.PinOutput})
	pinCS.High()
	pinSCK.Low()
	pinRST.High()

	if !initLoRa() {
		println("SX127x no detectado: revisa cableado y 3.3V")
		return
	}
	println("LoRa init OK")

	msg := []byte("Hello World, this is Electronic Clinic")
	for {
		if send(msg) {
			println("TX OK")
		} else {
			println("TX timeout")
		}
		time.Sleep(100 * time.Millisecond)
	}
}
