package main

import (
	"errors"
	"machine"
	"time"
)

// Registros del SX1276/77/78/79 (modo LoRa)
const (
	regFifo               = 0x00
	regOpMode             = 0x01
	regFrfMsb             = 0x06
	regFrfMid             = 0x07
	regFrfLsb             = 0x08
	regPaConfig           = 0x09
	regLna                = 0x0C
	regFifoAddrPtr        = 0x0D
	regFifoTxBase         = 0x0E
	regFifoRxBase         = 0x0F
	regFifoRxCurrent      = 0x10
	regIrqFlags           = 0x12
	regRxNbBytes          = 0x13
	regPktSnr             = 0x19
	regPktRssi            = 0x1A
	regModemConfig1       = 0x1D
	regModemConfig2       = 0x1E
	regPreambleMsb        = 0x20
	regPreambleLsb        = 0x21
	regPayloadLength      = 0x22
	regModemConfig3       = 0x26
	regDetectOptimize     = 0x31
	regDetectionThreshold = 0x37
	regSyncWord           = 0x39
	regDioMapping1        = 0x40
	regVersion            = 0x42
	regPaDac              = 0x4D

	modeLongRange    = 0x80
	modeStandby      = 0x01
	modeTx           = 0x03
	modeRxContinuous = 0x05

	irqTxDone   = 0x08
	irqCrcError = 0x20
	irqRxDone   = 0x40
)

var (
	ErrNoDevice  = errors.New("sx1278: no responde (RegVersion != 0x12)")
	ErrBadConfig = errors.New("sx1278: configuracion invalida")
	ErrTxTimeout = errors.New("sx1278: timeout de transmision")
	ErrCRC       = errors.New("sx1278: error de CRC")
	ErrTooLong   = errors.New("sx1278: payload mayor a 255 bytes")
)

type Config struct {
	Freq     uint32 // Hz
	SF       uint8  // 7..12
	BW       uint32 // Hz: 7800 ... 500000
	CR       uint8  // 5..8 (4/5 .. 4/8)
	SyncWord uint8
	TxPower  int8 // 2..17 dBm (PA_BOOST)
	CRC      bool
}

type SX1278 struct {
	spi  *machine.SPI
	cs   machine.Pin
	rst  machine.Pin
	freq uint32
}

func NewSX1278(spi *machine.SPI, cs, rst machine.Pin) *SX1278 {
	return &SX1278{spi: spi, cs: cs, rst: rst}
}

// --- Acceso SPI ---

func (d *SX1278) readReg(reg uint8) uint8 {
	w := [2]byte{reg & 0x7F, 0}
	var r [2]byte
	d.cs.Low()
	d.spi.Tx(w[:], r[:])
	d.cs.High()
	return r[1]
}

func (d *SX1278) writeReg(reg, val uint8) {
	w := [2]byte{reg | 0x80, val}
	d.cs.Low()
	d.spi.Tx(w[:], nil)
	d.cs.High()
}

func (d *SX1278) writeFifo(data []byte) {
	w := make([]byte, len(data)+1)
	w[0] = regFifo | 0x80
	copy(w[1:], data)
	d.cs.Low()
	d.spi.Tx(w, nil)
	d.cs.High()
}

func (d *SX1278) readFifo(n int) []byte {
	if n == 0 {
		return nil
	}
	w := make([]byte, n+1)
	r := make([]byte, n+1)
	w[0] = regFifo
	d.cs.Low()
	d.spi.Tx(w, r)
	d.cs.High()
	return r[1:]
}

func (d *SX1278) setMode(m uint8) {
	d.writeReg(regOpMode, modeLongRange|m)
}

func bwCode(bw uint32) (uint8, bool) {
	switch bw {
	case 7800:
		return 0, true
	case 10400:
		return 1, true
	case 15600:
		return 2, true
	case 20800:
		return 3, true
	case 31250:
		return 4, true
	case 41700:
		return 5, true
	case 62500:
		return 6, true
	case 125000:
		return 7, true
	case 250000:
		return 8, true
	case 500000:
		return 9, true
	}
	return 0, false
}

// --- API publica ---

// Begin hace reset por hardware, verifica el chip y aplica la configuracion.
// Deja el modulo en standby.
func (d *SX1278) Begin(c Config) error {
	bw, ok := bwCode(c.BW)
	if !ok || c.SF < 7 || c.SF > 12 || c.CR < 5 || c.CR > 8 || c.TxPower < 2 || c.TxPower > 17 {
		return ErrBadConfig
	}

	d.cs.Configure(machine.PinConfig{Mode: machine.PinOutput})
	d.cs.High()
	d.rst.Configure(machine.PinConfig{Mode: machine.PinOutput})
	d.rst.Low()
	time.Sleep(10 * time.Millisecond)
	d.rst.High()
	time.Sleep(10 * time.Millisecond)

	if d.readReg(regVersion) != 0x12 {
		return ErrNoDevice
	}

	d.setMode(0x00) // sleep (necesario para activar LoRa)
	d.freq = c.Freq
	frf := (uint64(c.Freq) << 19) / 32000000
	d.writeReg(regFrfMsb, uint8(frf>>16))
	d.writeReg(regFrfMid, uint8(frf>>8))
	d.writeReg(regFrfLsb, uint8(frf))

	d.writeReg(regFifoTxBase, 0)
	d.writeReg(regFifoRxBase, 0)
	d.writeReg(regLna, d.readReg(regLna)|0x03) // LNA boost

	// Modem: ancho de banda + coding rate, header explicito
	d.writeReg(regModemConfig1, bw<<4|(c.CR-4)<<1)

	// Spreading factor (SF7..SF12)
	d.writeReg(regDetectOptimize, (d.readReg(regDetectOptimize)&0xF8)|0x03)
	d.writeReg(regDetectionThreshold, 0x0A)
	cfg2 := (d.readReg(regModemConfig2) & 0x0B) | c.SF<<4
	if c.CRC {
		cfg2 |= 0x04
	} else {
		cfg2 &^= 0x04
	}
	d.writeReg(regModemConfig2, cfg2)

	// AGC automatico + Low Data Rate Optimize si el simbolo dura > 16 ms
	cfg3 := uint8(0x04)
	if (uint64(1)<<c.SF)*1000000/uint64(c.BW) > 16000 {
		cfg3 |= 0x08
	}
	d.writeReg(regModemConfig3, cfg3)

	d.writeReg(regPreambleMsb, 0)
	d.writeReg(regPreambleLsb, 8)
	d.writeReg(regSyncWord, c.SyncWord)

	// Potencia en PA_BOOST (modulos Ra-01/Ra-02/RFM9x)
	d.writeReg(regPaDac, 0x84)
	d.writeReg(regPaConfig, 0x80|uint8(c.TxPower-2))

	d.setMode(modeStandby)
	return nil
}

// Send transmite un paquete y espera a que termine (bloqueante).
func (d *SX1278) Send(payload []byte) error {
	if len(payload) > 255 {
		return ErrTooLong
	}
	d.setMode(modeStandby)
	d.writeReg(regFifoAddrPtr, 0)
	d.writeFifo(payload)
	d.writeReg(regPayloadLength, uint8(len(payload)))
	d.writeReg(regIrqFlags, 0xFF)
	d.setMode(modeTx)

	deadline := time.Now().Add(5 * time.Second)
	for d.readReg(regIrqFlags)&irqTxDone == 0 {
		if time.Now().After(deadline) {
			d.setMode(modeStandby)
			return ErrTxTimeout
		}
		time.Sleep(time.Millisecond)
	}
	d.writeReg(regIrqFlags, irqTxDone)
	d.setMode(modeStandby)
	return nil
}

// StartReceive pone el modulo en recepcion continua.
func (d *SX1278) StartReceive() {
	d.writeReg(regDioMapping1, 0x00) // DIO0 = RxDone
	d.writeReg(regIrqFlags, 0xFF)
	d.setMode(modeRxContinuous)
}

// Receive devuelve (nil, nil) si no hay paquete nuevo.
func (d *SX1278) Receive() ([]byte, error) {
	irq := d.readReg(regIrqFlags)
	if irq&irqRxDone == 0 {
		return nil, nil
	}
	d.writeReg(regIrqFlags, irq) // limpiar flags
	if irq&irqCrcError != 0 {
		return nil, ErrCRC
	}
	n := int(d.readReg(regRxNbBytes))
	d.writeReg(regFifoAddrPtr, d.readReg(regFifoRxCurrent))
	return d.readFifo(n), nil
}

// PacketRSSI del ultimo paquete recibido, en dBm.
func (d *SX1278) PacketRSSI() int {
	off := 164
	if d.freq >= 525000000 {
		off = 157
	}
	return int(d.readReg(regPktRssi)) - off
}

// PacketSNR del ultimo paquete recibido, en dB.
func (d *SX1278) PacketSNR() float32 {
	return float32(int8(d.readReg(regPktSnr))) * 0.25
}
