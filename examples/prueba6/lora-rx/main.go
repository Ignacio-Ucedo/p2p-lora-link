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

	// Entrar en RX Continuous. Las interrupciones de pin no funcionan en
	// ESP32-C3 con TinyGo, así que usamos polling del registro IRQ_FLAGS.
	startContinuousRx(radio)
	opmode := radio.ReadRegister(sx127x.SX127X_REG_OP_MODE)
	println("opmode RX Continuous =", opmode)

	lastPrint := time.Now()
	count := 0
	for {
		count++
		// Polling: leer IRQ flags directamente del registro
		irq := radio.ReadRegister(sx127x.SX127X_REG_IRQ_FLAGS)

		if irq&0x40 != 0 { // Bit 6: RxDone
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

		// Imprimir diagnóstico cada 100 iteraciones (~1 segundo)
		if count%100 == 0 {
			opmode := radio.ReadRegister(sx127x.SX127X_REG_OP_MODE)
			irqMask := radio.ReadRegister(sx127x.SX127X_REG_IRQ_FLAGS_MASK)
			rxNbBytes := radio.ReadRegister(sx127x.SX127X_REG_RX_NB_BYTES)
			fifoCurrent := radio.ReadRegister(sx127x.SX127X_REG_FIFO_RX_CURRENT_ADDR)
			modemCfg2 := radio.ReadRegister(sx127x.SX127X_REG_MODEM_CONFIG_2)
			println("DIAG irq=", irq, " mask=", irqMask, " opmode=", opmode, " rxNb=", rxNbBytes, " fifoCur=", fifoCurrent, " modCfg2=", modemCfg2)
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

// startContinuousRx configura el chip igual que lo hace Rx() del driver
// y entra en RX Continuous permanente.
func startContinuousRx(radio *sx127x.Device) {
	// El chip debe estar en SLEEP antes de cambiar configuraciones
	radio.SetOpMode(sx127x.SX127X_OPMODE_SLEEP)
	time.Sleep(10 * time.Millisecond)

	// Configurar LNA a máximo gain
	radio.WriteRegister(sx127x.SX127X_REG_LNA, 0x20)

	// Mapear DIO0 = RxDone, DIO1 = RxTimeout (igual que Rx() del driver)
	radio.WriteRegister(sx127x.SX127X_REG_DIO_MAPPING_1, 0x00)

	// Limpiar todos los IRQ flags
	radio.WriteRegister(sx127x.SX127X_REG_IRQ_FLAGS, 0xFF)

	// Desmaskar solo RxDone y RxTimeout (igual que Rx() del driver)
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
