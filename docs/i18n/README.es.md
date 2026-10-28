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
- Tema **Liquid glass** por defecto: los pétalos y el centro son vidrio esmerilado que refracta el escritorio y se abren con una animación de burbuja. También hay un tema clásico con el color de énfasis de Windows.
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

### Descargar v1.2.1

Descarga **[Petal.exe](https://github.com/Syawi/Petal/releases/download/v1.2.1/Petal.exe)** y ejecútalo directamente en Windows 11 x64. No necesitas ZIP, carpeta de recursos, permisos de administrador ni Office.

El logotipo, el sonido y las licencias están integrados. Petal guarda sus archivos de funcionamiento en `%LOCALAPPDATA%/Petal`. En la primera conversión de audio/vídeo descarga y verifica automáticamente FFmpeg (unos 109 MB); después funciona sin conexión. Las imágenes y los documentos no necesitan esa descarga.

El inicio con Windows se activa la primera vez y puede desactivarse en **Settings**. Conserva el ejecutable en una ubicación estable. Las licencias están en **Settings → Licenses**.

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
2. Mantén **Mayús** y haz clic derecho en un archivo compatible del Explorador o del escritorio.
3. Elige un formato. Petal crea y verifica la copia y muestra **Conversion complete**.

El centro (el logotipo de Windows) abre el menú de Windows, igual que un clic derecho normal. **Mayús + clic derecho** abre Petal de forma predeterminada; el atajo es personalizable. Haz clic fuera o pulsa **Esc** para ocultar el menú; una conversión activa continúa en segundo plano.

| Atajo | Acción |
| --- | --- |
| `1`–`6` | Convertir con el pétalo correspondiente |
| `Tab`, después `Enter` / `Espacio` | Seleccionar y activar una opción |
| `M` | Abrir el menú contextual de Windows |
| `Esc` | Ocultar el menú o la animación |

Abre **Settings…** desde la bandeja para capturar una tecla o botón del ratón con Ctrl/Mayús/Alt/Win, configurar el inicio con Windows o elegir y probar un sonido WAV. Los cambios se guardan automáticamente. **Reset** restaura Mayús + clic derecho.

Un clic izquierdo en el icono de la bandeja abre los ajustes rápidos (Liquid glass, Animations, Sound y **All settings…**). En el menú del clic derecho, **Mute sound** silencia el sonido, **Settings…** abre la ventana completa y **Quit Petal** cierra la aplicación. La preferencia de sonido se guarda. No se puede salir durante una conversión.

## Apariencia y desarrollo

![Petal con los temas Liquid glass y clásico, en oscuro y claro](../media/themes.png)

**Liquid glass** (predeterminado): vidrio esmerilado e incoloro que refracta lo que hay detrás del menú; al pasar el cursor, el vidrio se ilumina con un brillo blanco y un borde más luminoso. **Classic**: superficies opacas de Windows 11; al pasar el cursor, el pétalo adopta el color de énfasis y el texto se vuelve negro. Ambos siguen el modo claro u oscuro de Windows. El tema y las animaciones se cambian en los ajustes.

```powershell
./Install-MediaEngine.ps1
cargo fmt --check
cargo test --locked
cargo build --release --locked
```

La CI de Windows comprueba el formato, ejecuta las pruebas y compila la aplicación. La comprobación del escritorio interactivo y la exportación de fotogramas del README son opcionales. Documentación técnica en inglés: [pruebas](../../TESTING.md), [contribuciones](../../CONTRIBUTING.md), [recursos visuales](../MEDIA.md).

## Licencia y créditos

[Licencia MIT](../../LICENSE), copyright © 2026 Syawi. Las herramientas y los recursos de terceros conservan sus propias licencias: [avisos](../../THIRD_PARTY_NOTICES.md), [fuentes de recursos](../../assets/SOURCES.md), [motor multimedia](../../ffmpeg/SOURCES.md).
