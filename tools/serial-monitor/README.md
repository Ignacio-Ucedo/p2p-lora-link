# Monitor serial

Monitor de los nodos por **Web Serial**, en un solo `index.html` autocontenido
(vanilla, sin dependencias ni CDN). Lee los logs que el firmware ya saca por
serial y permite mandar líneas al puerto.

## Uso

- Abrí `index.html` directamente (`file://`) en **Chrome** o **Edge**. Funciona
  offline — pensado para pruebas de banco y de distancia sin WiFi.
- Pulsá **Conectar**, elegí el puerto del nodo (baud por defecto 115200, la
  consola del ESP).
- Para repartirlo con internet: GitHub Pages; los colegas hacen Ctrl+S y lo usan
  offline. Es el mismo archivo.

## Límites

- Solo navegadores Chromium (no Firefox/Safari).
- El puerto es **exclusivo**: cerrá la pestaña antes de flashear con `espflash`.
- **Fase 1 = read-first.** El envío de líneas ya funciona a nivel transporte,
  pero el firmware todavía no parsea comandos por serial (el control está en BLE).
  El parser serial es la fase 2.
