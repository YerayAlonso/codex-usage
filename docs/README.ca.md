# Codex Usage

[English](../README.md) · **Català**

Monitor lleuger de quota de Codex per a la safata del sistema, amb **Tauri 2 + Rust**.
Sense Electron, Chromium inclòs, Node resident ni framework de frontend.

## Ús

- **Anell exterior:** quota restant de 5 hores. **Interior:** quota setmanal restant.
- La tooltip i les barres i percentatges del panell mostren la **quota utilitzada**,
  amb el temps fins al restabliment. El panell no mostra cercles de quota:
  només els anells de la safata representen la quota restant.
- Clic a la icona: panell. Clic dret: menú natiu.
- Tres pestanyes: **Consum, Configuració i Activitat**.
- Finestra compacta de 460 × 600 punts, ajustada a l’àrea útil del monitor i al DPI.
  Només hi ha la creu nativa del sistema. Escape també tanca el panell.
- Sense scroll: historial paginat i detalls llargs dividits en pàgines completes.
- Consultes de quota cada 5 minuts, també amb el panell tancat.
- Català, castellà, anglès, francès, alemany i italià. Detecció de l’idioma del sistema
  amb anglès com a alternativa. L’idioma es pot seleccionar i desar a Configuració;
  també s’aplica al menú de safata, les dates, els errors i els esdeveniments nous.

La sessió és la de **Codex CLI**, que ha d’estar instal·lat i connectat a ChatGPT.
L’app no llegeix ni copia credencials. Les respostes externes de Codex i del sistema,
i els esdeveniments històrics importats, poden conservar el seu idioma original.

### Configuració

- Idioma.
- Enviar un prompt mínim cada 5 hores.
- Iniciar amb la sessió (només en versions compilades).
- **Opcions avançades:** model i ruta absoluta opcional de l’executable de Codex.
  El camp buit activa la detecció automàtica. A Windows cal `codex.exe`, no `.cmd` o `.bat`.

El model inicial és `gpt-6-luna`, heretat de la configuració original; canvia’l si
Codex no l’admet. El cicle i l’inici amb sessió estan desactivats per defecte.
El cicle funciona directament a l’app i no necessita cap tasca del Programador de Windows.

**Enviar `Responde solo: OK` pot consumir quota, però no restableix límits ni gasta
crèdits de restabliment.** El restabliment és una operació independent i sempre
requereix confirmació. No es gasten crèdits automàticament.

## Consum i fiabilitat

El procés resident només manté el nucli Rust, dos treballadors asíncrons,
temporitzadors i l’estat. El webview es crea en obrir el panell i es **destrueix**
en tancar-lo. No hi ha animacions ni bucles de consulta contínua.
Codex només s’executa durant les operacions i després es tanca.
«Surt», al menú de safata, tanca tota l’app.

Les peticions es programen cada 5 hores des de l’inici de l’últim intent, fins i tot
si falla. La programació es desa abans de la petició. Després d’una suspensió o
reinici es fa com a màxim un intent pendent, no tots els perduts. No hi ha
reintents ràpids que puguin gastar quota inesperadament.

- Temps màxim de protocol: 25 s; prompt: 120 s.
- Operacions serialitzades i una sola instància de l’app.
- Si falla una lectura, es conserven les dades anteriors marcades com a antigues.
- El prompt s’executa en un directori propi, en mode només lectura, sense
  configuració d’usuari, eines de shell, MCP, plugins o hooks.
- Disponibilitat de crèdits i compte verificats de nou abans de consumir-ne un.
- Clau d’idempotència desada **abans** de restablir la quota; es conserva en errors
  incerts, també després de reiniciar l’app. No eliminis `state.json` per reintentar.
- Només els últims 80 esdeveniments en memòria. Logs d’aproximadament 1 MiB amb dos
  fitxers anteriors. Els fitxers són llegibles en anglès i inclouen claus de
  traducció perquè el panell pugui mostrar-los en l’idioma seleccionat.

Revisa els logs abans de compartir-los. Els diagnòstics són limitats i s’oculten
formats de credencials comuns, però no es pot garantir que qualsevol sortida externa
no contingui informació personal.

## Dades locals i migració

«Obre la carpeta de logs» porta al directori exacte:

| Sistema | Directori |
|---|---|
| Windows | `%APPDATA%\dev.codex.usage` |
| macOS | `~/Library/Application Support/dev.codex.usage` |
| Linux | `$XDG_DATA_HOME/dev.codex.usage`, habitualment `~/.local/share/dev.codex.usage` |

- `state.json`: configuració, idioma, programació i possible clau de restabliment pendent.
- `codex-usage.log`, `codex-usage.1.log`, `codex-usage.2.log`: esdeveniments JSONL.
- `workspace/`: directori de treball propi.

En el primer inici es migra automàticament l’antic directori `dev.ciclo.codex`
si el nou encara no existeix, mitjançant un canvi de nom atòmic. Es conserven
configuració, programació, claus pendents i logs; els noms dels logs també s’actualitzen.
Si els dos directoris ja existeixen, no es fusionen ni se sobreescriuen estats.
Un `state.json` invàlid no se sobreescriu silenciosament.
Tanca qualsevol versió anterior abans d’obrir la nova, per evitar dos nuclis actius.

## Desenvolupament i proves

Node.js 22.12+, npm, Rust estable 1.90+ i els
[requisits natius de Tauri](https://v2.tauri.app/start/prerequisites/).
A Windows: eines C++ de Visual Studio i WebView2. A Linux: GTK 3, WebKitGTK 4.1
i suport AppIndicator.

```sh
npm ci
npm run dev
# Obrir el panell inicialment:
npm run dev -- -- --show

npm run check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

`npm run verify` agrupa les comprovacions de metadades, frontend, format Rust,
Clippy i proves natives. La llicència del projecte és [MIT](../LICENSE).

Les proves habituals són offline i no gasten quota ni crèdits. Els sis diccionaris
de `locales/` són compartits entre Rust i TypeScript; les proves comproven les claus
i els paràmetres de totes les traduccions.

Prova d’integració opcional, **només de lectura**:

```sh
cargo test --manifest-path src-tauri/Cargo.toml live_read_only_usage -- --ignored --nocapture
```

En desenvolupament, el frontend es recompila automàticament. Tanca i reobre el
panell per carregar els canvis.

Per comprovar la distribució de les targetes als sis idiomes i en mides compactes,
obre la vista de desenvolupament (`http://127.0.0.1:1420`) i executa a la consola:

```js
await (await import("/layout.browser.mjs")).checkLayout()
```

Aquesta comprovació fa servir dades simulades i verifica que els textos queden
dins del marge interior de cada targeta, sense solapaments ni scroll. El mòdul
de prova només es publica al servidor de desenvolupament, no a l’app compilada.

## Compilar

Executa al sistema de destí:

```sh
# Windows
npm run build -- --bundles nsis
# macOS
npm run build -- --bundles app,dmg
# Linux
npm run build -- --bundles deb,appimage
```

Executable Windows: `src-tauri/target/release/codex-usage.exe`.
Instal·ladors: `src-tauri/target/release/bundle/`.
L’usuari final no necessita Node ni Rust. Els paquets no estan signats.

Windows és la primera plataforma validada. macOS i Linux requereixen compilació
i comprovació als sistemes corresponents. A Linux, «Obre Codex Usage» al menú natiu
substitueix els esdeveniments de clic que alguns escriptoris no suporten.
