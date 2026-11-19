package main

import (
	"time"

	"tinygo.org/x/drivers/sx127x"
)

func main() {
	time.Sleep(2 * time.Second)
	println("LoRa Receiver OK")

	radio := setupRadio()

	// Entrar en RX Continuous una sola vez. Las interrupciones DIO no funcionan
	// en ESP32-C3 con TinyGo, así que usamos polling de RX_NB_BYTES.
	startContinuousRx(radio)

	lastPrint := time.Now()
	lastFifoAddr := uint8(0) // Marca de agua: FIFO_RX_CURRENT_ADDR cambia con cada paquete nuevo
	for {
		rxNb := radio.ReadRegister(sx127x.SX127X_REG_RX_NB_BYTES)
		fifoAddr := radio.ReadRegister(sx127x.SX127X_REG_FIFO_RX_CURRENT_ADDR)

		// En RX Continuous, cada paquete nuevo actualiza FIFO_RX_CURRENT_ADDR
		// para señalar el inicio del payload en el FIFO. Si el valor cambió
		// y RX_NB_BYTES > 0, tenemos un paquete nuevo.
		if rxNb > 0 && fifoAddr != lastFifoAddr {
			t := time.Now()
			pkt := readPacketFIFO(radio)
			ms := time.Since(t).Milliseconds()
			lastFifoAddr = fifoAddr // Guardar la marca de agua

			if len(pkt) > 0 {
				println("Recibido:", string(pkt), " (RSSI:", int8(radio.LastPacketRSSI()), "dBm)")
			}
			lastPrint = time.Now()
		}

		// Timeout cada 5 segundos sin actividad
		if time.Since(lastPrint) > 5000*time.Millisecond {
			println("Sin actividad (5s)")
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
