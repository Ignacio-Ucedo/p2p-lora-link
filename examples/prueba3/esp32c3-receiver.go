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

func readFifo(buf []byte) {
	pinCS.Low()
	xfer(0x00)
	for i := range buf {
		buf[i] = xfer(0)
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

	writeReg(0x0E, 0x00)
	writeReg(0x0F, 0x00)

	writeReg(0x0C, 0x23) // LNA ganancia máxima
	writeReg(0x1D, 0x72) // BW 125k, CR 4/5, header explícito
	writeReg(0x1E, 0x70) // SF7, CRC off (igual que el emisor)
	writeReg(0x26, 0x04) // AGC automático
	writeReg(0x31, 0xC3)
	writeReg(0x37, 0x0A)
	writeReg(0x20, 0x00)
	writeReg(0x21, 0x08) // preámbulo 8
	writeReg(0x39, 0x12) // sync word

	writeReg(0x12, 0xFF) // limpiar IRQ
	writeReg(0x01, 0x85) // RX continuo
	return true
}

func main() {
	time.Sleep(3 * time.Second)
	println("Receptor LoRa (ESP32-C3)")

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
	println("LoRa init OK, esperando paquetes...")

	for {
		irq := readReg(0x12)

		if irq&0x40 != 0 { // RxDone
			writeReg(0x12, 0xFF)

			if irq&0x20 != 0 { // PayloadCrcError
				println("Paquete con CRC erróneo")
				continue
			}

			n := int(readReg(0x13))
			writeReg(0x0D, readReg(0x10)) // FIFO ptr = FifoRxCurrentAddr
			buf := make([]byte, n)
			readFifo(buf)

			rssi := int(readReg(0x1A)) - 164
			snr := int(int8(readReg(0x19))) / 4

			println("Recibido:", string(buf), "| RSSI:", rssi, "dBm | SNR:", snr, "dB")
		}

		time.Sleep(5 * time.Millisecond)
	}
}
