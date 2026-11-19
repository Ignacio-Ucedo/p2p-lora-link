package main

import (
	"time"

	"tinygo.org/x/drivers/sx127x"
)

func main() {
	time.Sleep(2 * time.Second)
	println("LoRa Receiver (ESP32-C3 / TinyGo + sx127x)")

	radio := setupRadio()
	println("LoRa init OK")

	// Entrar en RX Continuous. IRQ RxDone no se dispara en este modo,
	// así que usamos polling del registro RX_NB_BYTES para detectar paquetes.
	startContinuousRx(radio)
	opmode := radio.ReadRegister(sx127x.SX127X_REG_OP_MODE)
	println("opmode RX Continuous =", opmode)

	lastPrint := time.Now()
	for {
		// Polling: en RX Continuous, IRQ RxDone no se dispara.
		// Usamos RX_NB_BYTES para detectar paquetes listos en el FIFO.
		rxNb := radio.ReadRegister(sx127x.SX127X_REG_RX_NB_BYTES)

		if rxNb > 0 {
			radio.WriteRegister(sx127x.SX127X_REG_IRQ_FLAGS, 0xFF) // Limpiar IRQ
			t := time.Now()
			pkt := readPacketFIFO(radio)
			ms := time.Since(t).Milliseconds()

			if len(pkt) > 0 {
				println("Recibido:", string(pkt), " (RSSI:", int8(radio.LastPacketRSSI()), "dBm) tras", ms, "ms")
			} else {
				println("vacio tras", ms, "ms")
			}
			lastPrint = time.Now()
		}

		// Imprimir timeout cada 5 segundos sin actividad
		if time.Since(lastPrint) > 5000*time.Millisecond {
			ms := time.Since(lastPrint).Milliseconds()
			println("vacio tras", ms, "ms")
			lastPrint = time.Now()
		}

		time.Sleep(10 * time.Millisecond) // Poll cada 10ms
	}
}

// startContinuousRx configura y entra en RX Continuous permanente.
// No usa SLEEP para evitar que la configuración del modem (SF, BW, etc.)
// se pierda y vuelva a los valores por defecto.
func startContinuousRx(radio *sx127x.Device) {
	// Configurar LNA a máximo gain
	radio.WriteRegister(sx127x.SX127X_REG_LNA, 0x20)

	// Mapear DIO0 = RxDone, DIO1 = RxTimeout
	radio.WriteRegister(sx127x.SX127X_REG_DIO_MAPPING_1, 0x00)

	// Limpiar todos los IRQ flags
	radio.WriteRegister(sx127x.SX127X_REG_IRQ_FLAGS, 0xFF)

	// Desmaskar solo RxDone y RxTimeout
	// 0x3F = 00111111: bits 6 (RxDone) y 7 (RxTimeout) en 0 → desenmascaradas
	radio.WriteRegister(sx127x.SX127X_REG_IRQ_FLAGS_MASK, 0x3F)

	// FIFO RX base address = 0
	radio.WriteRegister(sx127x.SX127X_REG_FIFO_RX_BASE_ADDR, 0)

	// Entrar en RX Continuous (SX127X_OPMODE_RX = 0x05, con LoRa = 0x80 → 0x85)
	radio.SetOpMode(sx127x.SX127X_OPMODE_LORA | sx127x.SX127X_OPMODE_RX)
	time.Sleep(10 * time.Millisecond)
}

// readPacketFIFO lee el payload del FIFO, igual que lo hace el driver en Rx().
func readPacketFIFO(radio *sx127x.Device) []byte {
	radio.WriteRegister(sx127x.SX127X_REG_FIFO_RX_BASE_ADDR, 0)
	radio.WriteRegister(sx127x.SX127X_REG_FIFO_ADDR_PTR, 0)
	pLen := radio.ReadRegister(sx127x.SX127X_REG_RX_NB_BYTES)
	radio.WriteRegister(sx127x.SX127X_REG_FIFO_ADDR_PTR, radio.ReadRegister(sx127x.SX127X_REG_FIFO_RX_CURRENT_ADDR))
	rxData := make([]byte, pLen)
	for i := uint8(0); i < pLen; i++ {
		rxData[i] = radio.ReadRegister(sx127x.SX127X_REG_FIFO)
	}
	return rxData
}
