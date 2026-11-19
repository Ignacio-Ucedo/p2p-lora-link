#include <SPI.h>
#include <LoRa.h>

// --- Pines para ESP32-C3 (los mismos que el emisor) ---
#define LORA_SCK    4
#define LORA_MISO   5
#define LORA_MOSI   6
#define LORA_SS     7
#define LORA_RST    3
#define LORA_DIO0   10

String inString = "";    // acumula los caracteres entrantes
String MyMessage = "";   // mensaje completo

void setup() {
    Serial.begin(9600);
    delay(1000); // Pausa para que el SX1278 se estabilice

    Serial.println("LoRa Receiver (ESP32-C3)");

    // Inicializar SPI con los pines personalizados
    SPI.begin(LORA_SCK, LORA_MISO, LORA_MOSI, LORA_SS);

    // Configurar los pines del módulo LoRa (con reset por hardware)
    LoRa.setPins(LORA_SS, LORA_RST, LORA_DIO0);
    LoRa.setSPIFrequency(4000000); // 4 MHz

    if (!LoRa.begin(433E6)) { // 433 MHz
        Serial.println("Starting LoRa failed!");
        while (1);
    }

    // Deben ir DESPUÉS de begin() e ser idénticos al emisor
    LoRa.setSpreadingFactor(9);
    LoRa.setSignalBandwidth(125E3);
    LoRa.setCodingRate4(5);
    LoRa.setSyncWord(0x12);

    Serial.println("LoRa init OK");
}

void loop() {
    int packetSize = LoRa.parsePacket();
    if (packetSize) {
        while (LoRa.available()) {
            inString += (char)LoRa.read();
        }
        MyMessage = inString;
        inString = "";

        Serial.print("Recibido: ");
        Serial.print(MyMessage);
        Serial.print("  (RSSI: ");
        Serial.print(LoRa.packetRssi());
        Serial.println(" dBm)");
    }
}