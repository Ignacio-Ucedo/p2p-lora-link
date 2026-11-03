package examples

import (
	"machine"
	"time"
)

const (
	pinSCK  = machine.GPIO12
	pinMISO = machine.GPIO13
	pinMOSI = machine.GPIO11
	pinCS   = machine.GPIO10
	pinRST  = machine.GPIO14
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

func initLoRa() bool {
	pinRST.Low()
	time.Sleep(10 * time.Millisecond)
	pinRST.High()
	time.Sleep(20 * time.Millisecond)

	v := readReg(0x42)
	println("RegVersion = ", v, "(esperado 18 = 0x12)")
	if v != 0x12 {
		return false
	}

	writeReg(0x01, 0x80) // LoRa + sleep
	time.Sleep(10 * time.Millisecond)

	// Frecuencia 433 MHz
	frf := (uint64(433_000_000) << 19) / 32_000_000
	writeReg(0x06, byte(frf>>16))
	writeReg(0x07, byte(frf>>8))
	writeReg(0x08, byte(frf))

	writeReg(0x0E, 0x00) // FIFO TX base
	writeReg(0x0F, 0x00) // FIFO RX base

	writeReg(0x1D, 0x72) // BW 125k, CR 4/5, header explícito
	//writeReg(0x1E, 0x90) // SF9, CRC off
	writeReg(0x1E, 0x70) // SF7, CRC off (antes 0x90 = SF9)
	writeReg(0x26, 0x04) // AGC automático
	writeReg(0x31, 0xC3) // detection optimize (SF7-12)
	writeReg(0x37, 0x0A) // detection threshold (SF7-12)
	writeReg(0x20, 0x00) // preámbulo MSB
	writeReg(0x21, 0x08) // preámbulo LSB = 8
	writeReg(0x39, 0x12) // sync word

	writeReg(0x4D, 0x84) // PA DAC normal (<=17 dBm)
	writeReg(0x09, 0x8F) // PA_BOOST, 17 dBm

	writeReg(0x01, 0x81) // standby
	return true
}

func send(msg []byte) bool {
	writeReg(0x01, 0x81) // standby
	writeReg(0x0D, 0x00) // FIFO ptr = 0
	for _, c := range msg {
		writeReg(0x00, c)
	}
	writeReg(0x22, byte(len(msg)))
	writeReg(0x12, 0xFF) // limpiar IRQ
	writeReg(0x01, 0x83) // TX

	for i := 0; i < 500; i++ { // timeout ~5 s
		if readReg(0x12)&0x08 != 0 {
			writeReg(0x12, 0x08)
			return true
		}
		time.Sleep(10 * time.Millisecond)
	}
	return false
}

func main() {
	time.Sleep(2 * time.Second)
	println("Iniciando")

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
