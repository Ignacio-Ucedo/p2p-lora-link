package main

import (
	"machine"
	"time"

	"tinygo.org/x/drivers/lora"
	"tinygo.org/x/drivers/sx127x"
)

const (
	pinSCK  = machine.GPIO4
	pinMISO = machine.GPIO5
	pinMOSI = machine.GPIO6
	pinSS   = machine.GPIO7
	pinRST  = machine.GPIO3
	pinDIO0 = machine.GPIO10
	pinDIO1 = machine.GPIO1
)

func fatal(msg string, err error) {
	for {
		if err != nil {
			println(msg, err.Error())
		} else {
			println(msg)
		}
		time.Sleep(time.Second)
	}
}

func setupRadio() *sx127x.Device {
	err := machine.SPI2.Configure(machine.SPIConfig{
		Frequency: 4000000,
		SCK:       pinSCK,
		SDO:       pinMOSI,
		SDI:       pinMISO,
		Mode:      0,
	})
	if err != nil {
		fatal("SPI error:", err)
	}

	pinRST.Configure(machine.PinConfig{Mode: machine.PinOutput})

	radio := sx127x.New(machine.SPI2, pinRST)

	// El RadioControl maneja NSS y las interrupciones de DIO0/DIO1
	err = radio.SetRadioController(sx127x.NewRadioControl(pinSS, pinDIO0, pinDIO1))
	if err != nil {
		fatal("RadioController error:", err)
	}

	radio.Reset()
	println("opmode tras Reset =", radio.ReadRegister(sx127x.SX127X_REG_OP_MODE))

	if !radio.DetectDevice() {
		fatal("Starting LoRa failed (RegVersion != 0x12)", nil)
	}
	println("opmode tras Detect =", radio.ReadRegister(sx127x.SX127X_REG_OP_MODE))

	// El bit LoRa solo se puede cambiar desde sleep
	radio.SetOpMode(sx127x.SX127X_OPMODE_SLEEP)
	time.Sleep(10 * time.Millisecond)
	println("opmode tras sleep =", radio.ReadRegister(sx127x.SX127X_REG_OP_MODE))

	radio.SetOpModeLora()
	time.Sleep(10 * time.Millisecond)
	println("opmode tras LoRa =", radio.ReadRegister(sx127x.SX127X_REG_OP_MODE))

	radio.LoraConfig(lora.Config{
		Freq:           433000000,
		Bw:             lora.Bandwidth_125_0,
		Sf:             lora.SpreadingFactor9,
		Cr:             lora.CodingRate4_5,
		HeaderType:     lora.HeaderExplicit,
		Preamble:       8,
		Ldr:            lora.LowDataRateOptimizeOff,
		Crc:            lora.CRCOff,
		Iq:             lora.IQStandard,
		SyncWord:       0x12,
		LoraTxPowerDBm: 17,
	})
	radio.SetSyncWord(0x12)

	radio.SetOpMode(sx127x.SX127X_OPMODE_STANDBY)
	println("opmode final =", radio.ReadRegister(sx127x.SX127X_REG_OP_MODE))

	println("RegVersion =", radio.GetVersion(),
		" SyncWord =", radio.ReadRegister(sx127x.SX127X_REG_SYNC_WORD))
	return radio
}

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	var b [10]byte
	i := len(b)
	for n > 0 {
		i--
		b[i] = byte('0' + n%10)
		n /= 10
	}
	return string(b[i:])
}
