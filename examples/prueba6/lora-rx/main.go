package main

import (
	"time"

	"tinygo.org/x/drivers/lora"
	"tinygo.org/x/drivers/sx127x"
)

func main() {
	time.Sleep(2 * time.Second)
	println("LoRa Receiver (ESP32-C3 / TinyGo + sx127x) - canal de eventos")

	radio := setupRadio()
	println("LoRa init OK")

	// DIO0 = RxDone, limpiar flags y pasar a recepcion continua
	radio.WriteRegister(sx127x.SX127X_REG_DIO_MAPPING_1, sx127x.SX127X_MAP_DIO0_LORA_RXDONE)
	radio.WriteRegister(sx127x.SX127X_REG_IRQ_FLAGS, 0xFF)
	radio.SetOpMode(sx127x.SX127X_OPMODE_RX)
	println("opmode en RX =", radio.ReadRegister(sx127x.SX127X_REG_OP_MODE))

	// Estado del chip cada 2 s, para diagnostico
	go func() {
		for {
			time.Sleep(2 * time.Second)
			println("  [diag] opmode=", radio.ReadRegister(sx127x.SX127X_REG_OP_MODE),
				" irq=", radio.ReadRegister(sx127x.SX127X_REG_IRQ_FLAGS),
				" DIO0=", pinDIO0.Get())
		}
	}()

	ch := radio.GetRadioEventChan()
	for ev := range ch {
		switch ev.EventType {
		case lora.RadioEventRxDone:
			println("Recibido:", string(ev.EventData),
				" (RSSI:", int8(radio.LastPacketRSSI()), "dBm)")
		default:
			println("evento tipo", ev.EventType, " irq=", ev.IRQStatus)
		}
	}
}
