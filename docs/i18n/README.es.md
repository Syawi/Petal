<p align="center"><a href="../../README.md">English</a> · <a href="README.zh-CN.md">简体中文</a> · <strong>Español</strong> · <a href="README.ar.md">العربية</a> · <a href="README.hi.md">हिन्दी</a> · <a href="README.fr.md">Français</a></p>

<p align="center"><img src="../media/hero.png" alt="Petal" width="100%"></p>

# Petal

**Haz clic derecho en un archivo. Elige un formato. Listo.**

Petal es una utilidad nativa para Windows 11 que reúne la conversión de imágenes, audio, vídeo y documentos de texto en un menú radial compacto. Funciona en el Explorador y en el escritorio, sigue el tema y el color de énfasis de Windows y permite abrir el menú contextual original desde el centro.

Esta página está en español. **La interfaz de la aplicación está en inglés.**

## Demostración

![Menú radial, animación al pasar el cursor, carga y confirmación de Petal](../media/demo.gif)

Vista previa generada con el renderizador nativo y un nombre de archivo de ejemplo. [Cómo se crean los recursos visuales](../MEDIA.md) (en inglés).

## Funciones

- Hasta seis formatos adecuados para el archivo; el formato actual se sustituye por una alternativa.
- Conversión local de imágenes, audio, vídeo y documentos de texto, sin subir archivos.
- Temas claro y oscuro, color de énfasis de Windows, pétalos animados y compatibilidad con movimiento reducido.
- Indicador de carga, marca de confirmación, notificación de Windows con el logotipo y sonido opcional.
- Rust con Win32/GDI+, sin entorno de navegador.

> **El archivo original permanece intacto.** Petal verifica el resultado y guarda una copia en la misma carpeta con la nueva extensión: `photo.jpg` → `photo.webp`. Nunca sobrescribe un destino existente. Los metadatos y la composición de la copia pueden cambiar.

## Formatos

| Archivo seleccionado | Opciones predeterminadas | Alternativa al formato actual |
| --- | --- | --- |
| Imagen | JPG · PNG · WebP · GIF · BMP · PDF | TIFF |
| Audio | MP3 · WAV · FLAC · M4A · OGG · MP4 | OPUS |
| Vídeo | MP4 · MKV · WebM · MOV · MP3 · WAV | AVI |
| Documento de texto | TXT · DOCX · MD · HTML · RTF · PDF | ODT |

Audio → MP4 genera un vídeo con fondo negro y el sonido del archivo. Vídeo → MP3 extrae la pista de audio. La conversión de documentos conserva el texto y los párrafos, no la composición original. Los PDF escaneados necesitan OCR y se rechazan. Consulta los [formatos y límites](../../FORMATS.md) (en inglés).

## Primeros pasos

**Requisitos: Windows 11 x64.**

### Descargar v1.0

Descarga **Petal-v1.0-Windows-x64.zip** desde la [versión v1.0](https://github.com/Syawi/Petal/releases/tag/v1.0). Extrae toda la carpeta y ejecuta `Petal.exe`. Incluye la aplicación, el código fuente y la documentación. Las herramientas de audio y vídeo se instalan por separado con `Install-MediaEngine.ps1`.

### Descargar una compilación

Abre una ejecución correcta del [workflow de Windows](https://github.com/Syawi/Petal/actions/workflows/windows.yml) y descarga **Petal-Windows-x64** desde sus artifacts. Es necesario iniciar sesión en GitHub. Extrae toda la carpeta y ejecuta `Petal.exe`.

La compilación básica admite imágenes y documentos. Para activar audio y vídeo, ejecuta una vez el script incluido desde PowerShell en esa carpeta:

```powershell
./Install-MediaEngine.ps1
```

Mantén `assets/` y, tras instalarlo, `ffmpeg/` junto a `Petal.exe`. No se necesitan permisos de administrador ni Office. La aplicación registra un acceso directo del menú Inicio y una identidad de notificación para el usuario actual; no se añade al inicio automático.

### Compilar el código fuente

Instala [Rust](https://rustup.rs/), las herramientas de compilación de Visual Studio C++ y el SDK de Windows para MSVC. Después:

```powershell
git clone https://github.com/Syawi/Petal.git
cd Petal
./Build.ps1
./Petal.exe
```

El script prepara FFmpeg/FFprobe, comprueba el SHA-256 de la descarga, ejecuta las pruebas y crea el ejecutable. Para GNU, consulta [CONTRIBUTING.md](../../CONTRIBUTING.md) (en inglés).

## Usar el menú radial

1. Inicia `Petal.exe`. Su icono azul aparece en la bandeja del sistema, a veces entre los iconos ocultos.
2. Haz clic derecho en un archivo compatible del Explorador o del escritorio.
3. Elige un formato. Petal crea y verifica la copia y muestra **Conversion complete**.

El centro abre **Windows menu**. **Mayús + clic derecho** permite abrir directamente el menú de Windows. Haz clic fuera o pulsa **Esc** para ocultar el menú; una conversión activa continúa en segundo plano.

| Atajo | Acción |
| --- | --- |
| `1`–`6` | Convertir con el pétalo correspondiente |
| `Tab`, después `Enter` / `Espacio` | Seleccionar y activar una opción |
| `M` | Abrir el menú contextual de Windows |
| `Esc` | Ocultar el menú o la animación |

En el menú del icono de la bandeja, **Mute sound** silencia el sonido y **Quit Petal** cierra la aplicación. La preferencia de sonido se guarda. No se puede salir durante una conversión.

## Apariencia y desarrollo

![Petal con los temas claro y oscuro de Windows](../media/themes.png)

Al pasar el cursor, el pétalo o el centro adopta el color de énfasis y el texto se vuelve negro. El movimiento reducido desactiva los desplazamientos y utiliza una señal estática.

```powershell
./Install-MediaEngine.ps1
cargo fmt --check
cargo test --locked
cargo build --release --locked
```

La CI de Windows comprueba el formato, ejecuta las pruebas y compila la aplicación. La comprobación del escritorio interactivo y la exportación de fotogramas del README son opcionales. Documentación técnica en inglés: [pruebas](../../TESTING.md), [contribuciones](../../CONTRIBUTING.md), [recursos visuales](../MEDIA.md).

## Licencia y créditos

[Licencia MIT](../../LICENSE), copyright © 2026 Syawi. Las herramientas y los recursos de terceros conservan sus propias licencias: [avisos](../../THIRD_PARTY_NOTICES.md), [fuentes de recursos](../../assets/SOURCES.md), [motor multimedia](../../ffmpeg/SOURCES.md).
