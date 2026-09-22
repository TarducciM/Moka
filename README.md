# Moka

[🇮🇹 Italiano](#italiano) · [🇬🇧 English](#english)

---

## Italiano

Tieni sveglio il tuo PC Windows, nello spirito di [Amphetamine](https://apps.apple.com/us/app/amphetamine/id937984704?mt=12) su macOS: dalla tray scegli se tenere acceso il PC, anche lo schermo, oppure spegnere solo lo schermo lasciando il PC sveglio — a tempo, fino a un'ora precisa, per sempre, o con regole automatiche.

**Stato: in sviluppo (0.0.3).** Il nucleo funziona; non c'è ancora una release. Il piano completo è in [docs/ROADMAP.md](docs/ROADMAP.md).

### Cosa fa già

- Due modalità: solo il PC, oppure PC e schermo. In più "spegni lo schermo ora" lasciando il PC sveglio (sui portatili con standby moderno è ancora in prova: vedi [docs/SPIKE.md](docs/SPIKE.md))
- Durate rapide (15 min … 4 h, personalizzabili), "fino alle HH:MM", "finché non lo spengo"
- Icona nella tray che mostra lo stato, pannello accanto all'icona, menu del clic destro
- Riga di comando per script e automazioni
- La sessione sopravvive a un crash dell'app, ma non a un riavvio del PC
- Portatili: resta acceso anche a coperchio chiuso, solo in carica o anche a batteria, con modalità scrivania, protezione zaino e blocco alla riapertura. L'impostazione di Windows torna sempre com'era, anche dopo un crash. Sui portatili con standby moderno è ancora in prova (vedi [docs/SPIKE.md](docs/SPIKE.md))
- Soglia batteria: sotto una certa carica la sessione finisce da sola
- "…e poi": a fine sessione spegni lo schermo, blocca, sospendi, iberna o spegni, sempre dopo un conto alla rovescia annullabile; e 5 minuti prima un avviso con "+30 min"
- Tasti rapidi globali per accendere, spegnere e spegnere lo schermo
- Regole automatiche: sveglio mentre un programma è aperto, sei in chiamata, c'è un'app a schermo intero, il PC è in carica, è collegato un monitor esterno, c'è un download in corso, il processore è occupato, è collegato un disco USB, sei su una certa rete, o in una fascia oraria. Ogni regola ha la sua modalità e il suo "…e poi", il pannello dice perché è acceso, e le regole si sospendono per un'ora con un clic
- Presenza (facoltativa, spenta di default): dopo un minuto senza toccare niente preme F15, così niente salvaschermo, blocco per inattività o stato "Assente". Il blocco per inattività esiste per sicurezza: sui PC di lavoro può violare le regole aziendali
- "Perché non dorme? Perché si è svegliato?": Moka legge ciò che Windows sa già (registro eventi, dispositivi che possono svegliarlo, impostazioni di sospensione) e lo spiega a parole; chi tiene sveglio il PC lo chiede a `powercfg /requests`, solo se lo chiedi tu e con il permesso dell'amministratore. Dal menu della tray: "Perché non dorme?…"
- Aggiornamenti automatici firmati, mai durante una sessione
- Chi tiene sveglio il PC e perché si vede in `powercfg /requests` ("Moka: sveglio per 2 h, fino alle 16:12")
- Italiano e inglese
- Completamente locale: nessun account, nessun cloud, nessuna telemetria

### Cosa farà

- Pubblicazione su winget

### Riga di comando

```text
moka                  apre il pannello
moka --for 2h         sveglio per 2 ore (anche 90m, 1h30m)
moka --until 18:30    fino alle 18:30
moka --forever        finché non lo spengo
moka --screen         anche lo schermo (si combina con le altre)
moka --on / --off     accende con l'ultima scelta / spegne
moka --toggle         accende o spegne
moka --screen-off     spegne subito lo schermo, il PC resta sveglio
moka --quit           chiude Moka
moka --then sleep     a fine sessione: display-off, lock, sleep, hibernate, shutdown
moka --lid / --no-lid questa sessione resta accesa (o no) a coperchio chiuso
moka --restore-lid    rimette l'impostazione del coperchio com'era
moka --while ffmpeg.exe    sveglio finché gira ffmpeg (con --screen e --then)
moka --while-pid 1234      sveglio finché vive il processo 1234
moka --pause-rules         sospende le regole per un'ora (--pause-rules=2h)
moka --resume-rules        le riattiva
```

Se Moka è già aperta, il comando arriva a lei.

Se l'impostazione del coperchio fosse rimasta cambiata (per esempio con la versione portable cancellata mentre la modifica era attiva), si rimette a mano da un prompt dei comandi:

```bash
powercfg /setacvalueindex SCHEME_CURRENT SUB_BUTTONS LIDACTION 1
```

```bash
powercfg /setdcvalueindex SCHEME_CURRENT SUB_BUTTONS LIDACTION 1
```

```bash
powercfg /setactive SCHEME_CURRENT
```

(1 = sospendi; su alcuni portatili l'impostazione è nascosta e la si legge con `powercfg /qh SCHEME_CURRENT SUB_BUTTONS LIDACTION`.)

### Sviluppo

Serve la [toolchain di Tauri](https://tauri.app/start/prerequisites/) (Rust con MSVC, Visual Studio Build Tools, WebView2, Node).

```bash
npm install
npm run dev                                          # Moka in modalità sviluppo
cargo test --manifest-path src-tauri/Cargo.toml      # test Rust
npm run check                                        # sintassi JS e traduzioni
npm run build                                        # installer NSIS e MSI
```

### Licenza

[MIT](LICENSE)

---

## English

Keep your Windows PC awake, in the spirit of [Amphetamine](https://apps.apple.com/us/app/amphetamine/id937984704?mt=12) on macOS: from the tray, choose whether to keep the system awake, the screen too, or turn off just the screen while the PC stays awake — for a set time, until a given time, indefinitely, or through automatic rules.

**Status: in development (0.0.3).** The core works; there is no release yet. The full plan is in [docs/ROADMAP.md](docs/ROADMAP.md) (in Italian).

### What it does today

- Two modes: PC only, or PC and screen. Plus "turn off the screen now" while the PC stays awake (still being tested on laptops with modern standby)
- Quick durations (15 min … 4 h, customizable), "until HH:MM", "until I turn it off"
- Tray icon that shows the state, a panel next to the icon, a right-click menu
- Command line for scripts and automation
- A session survives an app crash, but not a PC restart
- Laptops: stay awake with the lid closed, plugged in only or on battery too, with desk mode, bag protection and lock on reopen. The Windows setting always goes back as it was, even after a crash. Still being tested on laptops with modern standby
- Battery cutoff: the session ends on its own below a set charge
- "…and then": at the end of a session turn off the screen, lock, sleep, hibernate or shut down, always after a cancellable countdown; and 5 minutes before, a warning with "+30 min"
- Global keyboard shortcuts to turn it on, off, and turn off the screen
- Automatic rules: stay awake while a program is open, you're in a call, a full-screen app is showing, the PC is charging, an external monitor is connected, a download is running, the processor is busy, a USB drive is plugged in, you're on a given network, or during a time window. Each rule has its own mode and "…and then", the panel says why it's on, and rules pause for an hour with one click
- Presence (optional, off by default): after a minute without input it presses F15, so no screen saver, idle lock or "Away" status. The idle lock exists for security: on work PCs this may break company rules
- "Why won't it sleep? Why did it wake up?": Moka reads what Windows already knows (event log, devices that can wake it, sleep settings) and explains it in plain words; who keeps the PC awake comes from `powercfg /requests`, only when you ask and with administrator permission. From the tray menu: "Why won't it sleep?…"
- Signed automatic updates, never during a session
- `powercfg /requests` shows who is keeping the PC awake and why ("Moka: awake for 2 h, until 16:12")
- Italian and English UI
- Fully local: no account, no cloud, no telemetry

### Planned

- Publishing on winget

### Command line

```text
moka                  opens the panel
moka --for 2h         awake for 2 hours (also 90m, 1h30m)
moka --until 18:30    until 18:30
moka --forever        until I turn it off
moka --screen         screen too (combines with the others)
moka --on / --off     turn on with the last choice / turn off
moka --toggle         turn on or off
moka --screen-off     turn off the screen now, the PC stays awake
moka --quit           quit Moka
moka --then sleep     at the end: display-off, lock, sleep, hibernate, shutdown
moka --lid / --no-lid this session stays on (or not) with the lid closed
moka --restore-lid    put the lid setting back as it was
moka --while ffmpeg.exe    awake while ffmpeg runs (with --screen and --then)
moka --while-pid 1234      awake while process 1234 is alive
moka --pause-rules         pause rules for an hour (--pause-rules=2h)
moka --resume-rules        resume them
```

If Moka is already running, the command goes to it.

### Development

You need the [Tauri toolchain](https://tauri.app/start/prerequisites/) (Rust with MSVC, Visual Studio Build Tools, WebView2, Node).

```bash
npm install
npm run dev                                          # run Moka in development
cargo test --manifest-path src-tauri/Cargo.toml      # Rust tests
npm run check                                        # JS syntax and translations
npm run build                                        # NSIS and MSI installers
```

### License

[MIT](LICENSE)
