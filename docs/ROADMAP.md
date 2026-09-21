# Moka — piano di progetto

> **Stato: progettazione.** Non c'è ancora una riga di codice.
>
> Questo file è il punto di ripresa: chi riprende il lavoro, da qualunque PC, parte da qui. Va aggiornato a ogni passaggio significativo, insieme a `CHANGELOG.md`.
>
> Ultimo aggiornamento: 2026-09-21.

## In una riga

Un keep-awake per Windows nello spirito di [Amphetamine](https://apps.apple.com/us/app/amphetamine/id937984704?mt=12) su macOS. Dalla tray scegli se tenere acceso il PC, anche lo schermo, oppure spegnere solo lo schermo lasciando il PC sveglio: a tempo, fino a un'ora precisa, per sempre, o con regole automatiche. Un eseguibile piccolo, tutto locale, open source.

**Perché, visto che esiste [PowerToys Awake](https://learn.microsoft.com/en-us/windows/powertoys/awake)**: Awake copre le basi (per sempre, a tempo, fino a un'ora, schermo acceso, `--pid`), ma va installato l'intero PowerToys e non ha regole automatiche, protezione batteria, azione a fine sessione né "spegni solo lo schermo". Le regole automatiche sono il punto forte di Amphetamine, ed è lì che Moka si distingue.

---

## Come riprendere

Serve un PC Windows con la toolchain Tauri. **Il primo comando è `hostname`**, e il risultato va annotato nella tabella qui sotto: dire "qui si compila" senza dire *dove* non serve a nessuno.

1. Prerequisiti ([guida Tauri](https://tauri.app/start/prerequisites/)):
   - Rust via rustup (`stable-x86_64-pc-windows-msvc`)
   - Visual Studio Build Tools, workload "Sviluppo di applicazioni desktop con C++"
   - WebView2 (già presente su Windows 10/11 aggiornati)
   - Node LTS

   ```bash
   hostname && cargo --version && rustc --version && node --version
   ```

2. Generare lo scheletro nella root di questo repo, template **vanilla** (niente framework), con npm:

   ```bash
   npm create tauri-app@latest
   ```

   Nome `moka`, identifier `com.moka.app`. Poi allinearlo a ClipVault: `src/` servito così com'è, `frontendDist: "../src"`, `withGlobalTauri: true`, nessun bundler.

3. Primo obiettivo: la **0.1.0** (vedi la roadmap), verificata come descritto in "Cosa conta come fatto".

| Data | Macchina (`hostname`) | Rust | Cosa è stato fatto |
|---|---|---|---|
| 2026-09-21 | PC-MIKY | no | solo progettazione, nessun codice |

---

## Decisioni prese (2026-09-21)

| Tema | Decisione |
|---|---|
| Nome | **Moka** |
| Repo | `TarducciM/Moka`. **Privato** finché non c'è una versione da mostrare, poi pubblico come ClipVault e MD-Viewer. Deve essere pubblico **prima della prima release**: l'updater scarica da `releases/latest/download`, che per un repo privato non è raggiungibile. |
| Licenza | MIT |
| Stack | Tauri 2, backend Rust, frontend HTML/CSS/JS scritto a mano, niente framework né bundler |
| Identifier | `com.moka.app`. **È definitivo**: finisce nel percorso in `%APPDATA%`, nella voce di avvio automatico e nell'updater. Cambiarlo dopo la prima release rompe le installazioni esistenti. |
| Pacchetto Cargo | `moka` (lib `moka_lib`). Su crates.io esiste un crate `moka` (una cache): nessun conflitto, finché non pubblichiamo su crates.io e non lo usiamo come dipendenza. |
| Presenza (F15) | **Sì**, spenta di default, con avviso esplicito |
| Piattaforma | Solo Windows 10/11 x64 |
| Lingue | Italiano e inglese. Segue la lingua di sistema, con selettore nelle Impostazioni. |
| Sito | `moka.mtsolutions.studio`, cartella `site/` in questo repo (come ClipVault) |
| Privacy | Titolare San Marino Games S.r.l., come gli altri progetti; contatto `info@mtsolutions.studio` |
| Dati | Tutto locale: nessun account, nessun cloud, nessuna telemetria. L'unica chiamata di rete è il controllo aggiornamenti su GitHub. |

---

## Cosa fa

### Modalità

| Modalità | Effetto | Come |
|---|---|---|
| **PC acceso** | Niente sospensione; lo schermo segue il piano energetico | `PowerRequestSystemRequired` |
| **PC e schermo accesi** | Niente sospensione né spegnimento dello schermo | `PowerRequestSystemRequired` + `PowerRequestDisplayRequired` |
| **Spegni schermo ora** | Monitor spento subito, PC sveglio | `SC_MONITORPOWER` (valore 2) + `PowerRequestSystemRequired` |

Durate: 15 min · 30 min · 1 h · 2 h · 4 h · fino alle HH:MM · finché non lo spengo. La lista è personalizzabile.

Il **motivo** della richiesta (`REASON_CONTEXT`) è una frase leggibile e tradotta, per esempio "Moka: sessione fino alle 18:30" o "Moka: OBS è aperto". Chi apre `powercfg /requests` vede chi tiene sveglio il PC e perché, non un processo anonimo.

### "…e poi" — azione a fine sessione

Alla fine di una sessione a tempo, o quando una regola smette di valere, Moka può: non fare niente (default), spegnere lo schermo, bloccare il PC, sospenderlo, ibernarlo o spegnerlo.

L'azione parte **sempre dopo un conto alla rovescia di 60 secondi annullabile**, mai subito. Uso tipico: *"sveglio finché ffmpeg non finisce, poi sospendi"*.

Se il PC si è sospeso comunque durante la sessione (coperchio chiuso, tasto di accensione) e al risveglio la scadenza è già passata, la sessione finisce **senza** eseguire l'azione: niente spegnimento a sorpresa appena riapri il portatile.

### Regole automatiche ("tieni sveglio mentre…")

| Regola | Come si rileva | Note |
|---|---|---|
| Un programma è aperto | Elenco dei processi (Toolhelp32) ogni 5 s | Per nome dell'eseguibile, scelto dall'elenco dei processi in esecuzione |
| App a schermo intero | `SHQueryUserNotificationState` (`QUNS_BUSY`, `QUNS_RUNNING_D3D_FULL_SCREEN`, `QUNS_PRESENTATION_MODE`) | Video, giochi, presentazioni |
| In chiamata | Registro `CapabilityAccessManager\ConsentStore\{microphone,webcam}`: `LastUsedTimeStop = 0` significa in uso | **Da verificare** con Teams, Zoom e Meet nel browser |
| In carica | `GetSystemPowerStatus` + `RegisterPowerSettingNotification(GUID_ACDC_POWER_SOURCE)` | |
| Monitor esterno collegato | Numero di monitor (`EnumDisplayMonitors`, `WM_DISPLAYCHANGE`) | "Modalità scrivania" |
| Download in corso | Contatori delle interfacce di rete (`GetIfTable2`), soglia in KB/s | Finisce dopo N minuti sotto soglia |
| CPU occupata | `GetSystemTimes`, soglia in % | Render, compilazioni |
| Fascia oraria | Giorni della settimana + orari | Es. lun-ven 9-18 |
| Disco USB collegato | Volumi rimovibili (`WM_DEVICECHANGE`) | 0.4 |
| Rete Wi-Fi | Vedi trappola 17 | 0.4, da verificare |

**Stato effettivo** = sessione manuale + regole attive. Vale la modalità più "forte" (schermo acceso batte solo PC).

Il popover dice sempre il perché: *"Sveglio perché: OBS è aperto"*. Se si spegne a mano mentre una regola è attiva, Moka chiede se sospendere le regole per un'ora o fino al prossimo avvio: altrimenti la regola riaccenderebbe tutto cinque secondi dopo.

### Protezioni

- **Soglia batteria** (default 20%, disattivabile): sotto la soglia la sessione finisce da sola e lo dice con una notifica.
- **"Mai a batteria"**, facoltativo.

### Presenza

Ogni ~50 secondi, e solo se l'utente è inattivo da almeno 50 secondi (`GetLastInputInfo`), Moka invia un tasto **F15** con `SendInput`. È un tasto che non esiste sulle tastiere comuni e che quasi nessuna app usa.

Effetto: il contatore di inattività di Windows riparte, quindi niente salvaschermo, niente blocco per inattività, niente "Assente" su Teams o Slack. Serve perché le richieste di alimentazione **non fermano il salvaschermo** (lo dice la documentazione di `SetThreadExecutionState`).

- Spenta di default, attivabile per sessione o come impostazione.
- Testo in app, obbligatorio: *"Il blocco per inattività esiste per sicurezza: sui PC di lavoro questa opzione può violare le regole aziendali."*
- **Da verificare** che F15 non faccia niente nelle app più comuni. Alternativa: spostare il mouse di un pixel e riportarlo indietro.

### Diagnostica: "Perché il PC non dorme / si è svegliato?" (0.4)

Su richiesta, con il prompt UAC, esegue:

- `powercfg /requests`
- `powercfg /lastwake`
- `powercfg /waketimers`
- `powercfg /devicequery wake_armed`

Poi spiega il risultato in parole normali (es. "Chrome sta riproducendo audio", "il mouse può riattivare il PC"). Nessuna modifica automatica: solo la spiegazione e, dove serve, il comando da lanciare. L'output di `powercfg` potrebbe essere localizzato (trappola 18).

### Coperchio chiuso (0.4, avanzata)

È l'**unica** funzione che tocca il piano energetico. Imposta temporaneamente l'azione alla chiusura del coperchio su "Non fare nulla" (`PowerWriteACValueIndex`/`PowerWriteDCValueIndex` su `GUID_LIDCLOSE_ACTION`, poi `PowerSetActiveScheme`), dopo aver salvato su disco il valore originale.

- Il valore originale si ripristina a fine sessione, all'uscita, e al successivo avvio se l'app era caduta.
- Avviso sul surriscaldamento (portatile chiuso dentro uno zaino).
- Spenta di default.
- **Da verificare**: se serve l'amministratore, e come si comportano i PC con Modern Standby.

### Riga di comando

```text
moka                      apre il popover
moka --for 2h             sveglio per 2 ore (accetta anche 90m, 1h30m)
moka --until 18:30        fino alle 18:30
moka --screen             anche lo schermo (si combina con le altre)
moka --while-pid 1234     finché il processo 1234 è vivo
moka --while ffmpeg.exe   finché un processo con quel nome è aperto
moka --then sleep         a fine sessione: display-off | lock | sleep | hibernate | shutdown
moka --off                termina la sessione
moka --screen-off         spegne subito lo schermo
```

Gli argomenti arrivano all'istanza già aperta tramite il plugin single-instance, come fa ClipVault con `--enable-autostart`. Nella 0.2 i comandi non restituiscono output ("spara e dimentica", vedi trappola 15). Anche da riga di comando `--then shutdown` passa dal conto alla rovescia.

---

## Interfaccia

```text
┌─ Moka ─────────────────────────┐
│  ● Acceso · ancora 1 h 12 min  │
│  [ PC ] [ PC + schermo ]       │
│  15m  30m  1h  2h  4h  ∞  ⏰   │
│  Poi: [ niente ▾ ]             │
│  Regole: OBS aperto · In carica│
│  ☾ Spegni schermo ora     ⚙    │
└────────────────────────────────┘
```

- **Tray**: clic sinistro accende o spegne con l'ultima modalità usata, clic destro apre il menu completo (durate, modalità, regole, Impostazioni, Esci). Il tooltip mostra lo stato e il tempo residuo.
- **Icona**: una moka stilizzata, SVG disegnato a mano. Tre stati: spenta (contorno), PC acceso (piena), PC e schermo accesi (piena, con vapore). Serve una variante per la barra chiara e una per quella scura. **Va provata a 16 px prima di disegnare il resto.**
- **Grafica**: font di sistema, grigi neutri, un solo colore d'accento, niente emoji nell'interfaccia. Segue il tema chiaro/scuro di Windows.
  - Accento proposto: `#8B5A2B` (marrone caffè), 5,8:1 su bianco.
  - Su fondo scuro lo stesso colore scende a 2,9:1 e non basta per il testo. Serve un token separato, es. `#D9A066` (7,4:1 su `#1C1C1C`), mentre sfondi e bordi restano sull'accento.
  - I rapporti vanno rimisurati sui colori calcolati nella pagina, non su quelli scritti nel CSS.
- **Impostazioni**:
  - avvio automatico con Windows
  - lingua
  - modalità e durate predefinite
  - soglia batteria
  - Presenza
  - tasti rapidi
  - aggiornamenti
  - versione e crediti
- **Promemoria "metti una stella su GitHub"**, come in MD-Viewer: dopo 5 avvii e 3 giorni, "più tardi" rimanda di 14 giorni, "non mostrare più" lo spegne per sempre.

---

## Roadmap

Ogni passaggio: bump di patch più voce nel `CHANGELOG`. Minor alle tappe qui sotto, 1.0.0 solo alla release "vera".

### 0.1.0 — fa il suo mestiere

- [ ] Scheletro Tauri 2 vanilla, identifier `com.moka.app`
- [ ] `power.rs`: richieste di alimentazione con motivo leggibile
- [ ] Le tre modalità; durate, "fino alle", "per sempre"
- [ ] Icona nella tray con tre stati e varianti chiara/scura, tooltip con tempo residuo
- [ ] Popover (finestra dichiarata in `tauri.conf.json`; chiuderla la nasconde)
- [ ] Impostazioni: avvio automatico, lingua, durate predefinite
- [ ] Single-instance
- [ ] i18n IT/EN, con controllo automatico delle chiavi
- [ ] Sessione salvata su disco e ripresa dopo un riavvio dell'app (**non** dopo un riavvio del PC: si riconosce confrontando l'uptime)
- [ ] CI (`fmt`, `clippy -D warnings`, `test`)
- [ ] `test.md` con la verifica su un PC vero

### 0.2.0 — prima release pubblica

- [ ] "…e poi" con conto alla rovescia
- [ ] Notifica 5 minuti prima della fine con "+30 min" (vedi trappola 20)
- [ ] Tasti rapidi globali (accendi/spegni, spegni schermo ora), scelti da una lista di combinazioni sicure, **niente Ctrl+Alt** (trappola 19)
- [ ] Riga di comando
- [ ] Soglia batteria
- [ ] Installer NSIS + MSI + portable, con la pagina "Attività aggiuntive" per l'avvio automatico
- [ ] Auto-update firmato
- [ ] Promemoria stella GitHub
- [ ] `site/` con index, privacy, terms, cookie policy
- [ ] Repo pubblico, poi tag `v0.2.0`

### 0.3.0 — regole automatiche e Presenza

- [ ] Motore delle regole (una regola per file, trait comune, controllo ogni 5 s, eventi dove è semplice)
- [ ] Regole: programma aperto, schermo intero, in chiamata, in carica, monitor esterno, fascia oraria, download in corso, CPU occupata
- [ ] "Sospendi le regole per un'ora"
- [ ] Presenza

### 0.4.0 — diagnostica e casi avanzati

- [ ] Diagnostica "perché non dorme / perché si è svegliato"
- [ ] Regole: disco USB, rete Wi-Fi
- [ ] Coperchio chiuso
- [ ] (forse) Statistiche locali: quante ore sveglio, e perché

### 1.0.0

- [ ] Pubblicazione su winget
- [ ] Binari firmati
- [ ] Regole stabili, nessun bug grave aperto

---

## Architettura

```text
src-tauri/src/
  main.rs        avvio
  lib.rs         builder Tauri, plugin, tray, comandi
  power.rs       wrapper su PowerCreateRequest / PowerSetRequest / PowerClearRequest
  session.rs     macchina a stati pura, orologio iniettato (testabile)
  triggers/      una regola per file, trait comune
  presence.rs    F15 via SendInput
  actions.rs     schermo spento, blocco, sospensione, ibernazione, spegnimento
  cli.rs         parsing degli argomenti (anche quelli inoltrati dal single-instance)
  settings.rs    JSON in %APPDATA%, normalizzato in lettura
src/
  index.html     popover
  settings.html
  i18n.js
  styles.css
```

**Dipendenze previste**:

- `tauri` (feature `tray-icon`)
- plugin Tauri: `single-instance`, `autostart`, `global-shortcut`, `notification`, `updater`, `process`, `opener`
- `windows` (il crate di Microsoft), attivando **solo** le feature usate: `Win32_System_Power`, `Win32_System_Threading`, `Win32_UI_WindowsAndMessaging`, `Win32_UI_Input_KeyboardAndMouse`, `Win32_UI_Shell`, `Win32_System_Registry`, `Win32_System_Shutdown`, …
- `serde`, `serde_json`

Nessun database: le impostazioni sono un file JSON. Più leggero di ClipVault.

**Tempo**:

- Le durate usano l'orologio monotono; "fino alle HH:MM" usa l'orologio di sistema.
- Al ritorno dalla sospensione si ricalcola tutto (vedi "…e poi").
- La logica di sessione riceve l'orologio dall'esterno, così scadenze, ritorno dalla sospensione e cambi d'ora si testano senza aspettare.

**Impostazioni lette come se fossero ostili**: un campo mancante, del tipo sbagliato o fuori intervallo torna al default, e non deve mai rompere il resto.

---

## Trappole già note

### Da ClipVault (già pagate lì, da non ripagare)

1. Le finestre create da codice Rust a runtime restavano **bianche**: dichiararle tutte in `tauri.conf.json`.
2. Chiudere una finestra con la X la **distrugge**, e non si riapre più: intercettare `CloseRequested` e nasconderla.
3. `createUpdaterArtifacts: true` in `bundle` è obbligatorio: senza, `tauri-bundler` non firma niente.
4. NSIS firma direttamente il `setup.exe`, **non esiste un `.nsis.zip`**. `latest.json` va costruito a mano nel workflow dal `.sig` dell'NSIS e validato con `jq` prima dell'upload.
5. `uploadUpdaterJson` va lasciato a `true`: con `false` sparisce anche la generazione degli artefatti, non solo l'upload.
6. Avvio automatico: il nome del valore nel registro è quello del pacchetto Cargo (`moka`). L'installer **non** scrive il registro da sé: lancia `moka.exe --enable-autostart` impersonando l'utente reale, così passa dallo stesso codice del toggle nelle Impostazioni.
7. Disattivare l'avvio automatico quando non è attivo dà "os error 2" e blocca il salvataggio: toccare il registro solo se lo stato cambia davvero.
8. Le finestre restano vive in background: un errore in `load()` non deve mai sostituire il form, altrimenti ogni apertura successiva fallisce per sempre.
9. Senza bundler, i plugin si usano da `window.__TAURI__` (serve `withGlobalTauri: true`).
10. CI: `push` filtrato su `branches: [main]` più un blocco `concurrency` con `cancel-in-progress` fuori da `main` (il `ci.yml` di ClipVault non ha il `concurrency`).

### Da Windows

11. `SetThreadExecutionState` vale **per thread** e si perderebbe sul pool di thread di Tauri: usare `PowerCreateRequest`. È basato su un handle, e quando il processo termina (anche in crash) lo rilascia il sistema, quindi non può restare niente di appeso.
12. Le richieste di alimentazione non fermano il salvaschermo né il blocco per inattività: è il motivo della Presenza.
13. Non bloccano nemmeno la sospensione chiesta esplicitamente: coperchio, tasto di accensione, Start → Sospendi.
14. `SendMessage(HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER, 2)` può bloccarsi se una finestra non risponde: usare `PostMessage` o `SendMessageTimeout`.
15. L'eseguibile è GUI (`windows_subsystem = "windows"`), quindi niente stdout sul terminale. Nella 0.2 i comandi da riga di comando non danno output. Un `moka status` con risposta richiede un piccolo binario console separato oppure `AttachConsole`: da valutare.
16. Icona nella tray: la barra di Windows 10/11 può essere chiara o scura. Leggere `SystemUsesLightTheme` e cambiare variante al volo su `WM_SETTINGCHANGE`.
17. Wi-Fi: da Windows 11 24H2 leggere il nome della rete (SSID) richiederebbe il permesso di posizione (**da verificare**). Alternativa possibile: il nome del profilo di rete tramite Network List Manager.
18. `powercfg /requests` richiede l'amministratore, e l'output potrebbe essere localizzato: il parsing va provato su Windows in italiano e in inglese.
19. Sulle tastiere italiane Ctrl+Alt equivale ad AltGr: nessuna combinazione Ctrl+Alt come tasto rapido predefinito.
20. Le notifiche toast con pulsanti richiedono un AppUserModelID registrato (lo registra l'installer). Verificare cosa supporta davvero `tauri-plugin-notification` su Windows; in ogni caso il clic sulla notifica deve aprire il popover.
21. i18n: una chiave mancante non dà errori, mostra la chiave stessa a schermo. Serve un controllo in CI che italiano e inglese abbiano le stesse chiavi e che ogni `data-i18n` esista.

---

## Limiti da dichiarare all'utente (README e app)

- Moka non impedisce la sospensione **chiesta da te**: coperchio chiuso (salvo la modalità coperchio chiuso), tasto di accensione, Start → Sospendi.
- **Schermata di blocco**: comportamento da verificare e poi documentare. Secondo la documentazione di PowerToys Awake, lì le richieste non valgono.
- Sui PC aziendali i criteri di gruppo possono prevalere su tutto.
- Presenza: vedi l'avviso nella sezione dedicata.
- Tenere sveglio a lungo un portatile a batteria la consuma: per questo la soglia batteria è attiva di default.

---

## Cosa conta come "fatto"

"Compila" non vuol dire "funziona". Per considerare una modalità fatta:

- Con la sessione attiva, `powercfg /requests` (da un prompt amministratore) mostra `moka.exe` nella sezione giusta (`SYSTEM`, e anche `DISPLAY` se lo schermo è incluso), con il motivo leggibile. A sessione finita non deve comparire più.
- Chiudendo Moka a forza da Task Manager, la richiesta sparisce da `powercfg /requests`.
- Schermo: con lo spegnimento dello schermo impostato temporaneamente a 1 minuto (da ripristinare dopo), lo schermo non si spegne.
- Ogni verifica fatta a mano va in `test.md`, con data, macchina ed esito.

---

## Distribuzione

- **Release**: un tag `vX.Y.Z` fa partire `release.yml`, preso da ClipVault. `tauri-action` produce NSIS + MSI + portable; la release viene creata in bozza con la tabella dei download, e si cancella da sola se la build fallisce.
- **Installer**:
  - scelta della cartella di installazione
  - "solo per me" / "per tutti" (`installMode`, da valutare)
  - icona sul desktop
  - avvio automatico nella pagina "Attività aggiuntive"
  - template `installer.nsi` e `main.wxs` ripresi da ClipVault 0.3.9
  - in modalità silenziosa (gli aggiornamenti automatici) la pagina viene saltata, quindi un aggiornamento non cambia mai la scelta sull'avvio automatico
- **Auto-update**:
  - chiave minisign generata alla 0.2 con `tauri signer generate`
  - chiave privata come secret `TAURI_SIGNING_PRIVATE_KEY` del repo, più un backup fuori dal repo, mai dentro
  - chiave pubblica in `tauri.conf.json`
  - endpoint: `https://github.com/TarducciM/Moka/releases/latest/download/latest.json`
  - controllo all'avvio e ogni 24 ore, con notifica dalla tray
  - **Mai un riavvio per aggiornare durante una sessione attiva**: la sessione cadrebbe e il PC andrebbe in sospensione a metà di un download. Si aggiorna a sessione finita, o si ripristina la sessione dopo il riavvio.
- **Versioni**: `package.json`, `Cargo.toml` e `tauri.conf.json` sempre allineati.
- **Dopo**:
  - winget, tramite una PR a `microsoft/winget-pkgs` (identificativo tipo `TarducciM.Moka`); forse Scoop.
  - Firma del codice: senza firma Windows mostra l'avviso SmartScreen. SignPath Foundation offre firma gratuita ai progetti open source, su domanda (requisiti da verificare). Il problema è lo stesso di ClipVault, quindi risolverlo una volta vale per entrambi.

---

## Cosa NON viaggia fra i PC

- La chiave privata dell'updater, quando esisterà: sta nel secret su GitHub e in un backup esterno, mai nel repo.
- Le note di sviluppo locali sono escluse dal repo di proposito (vedi `.gitignore`): tutto ciò che serve per riprendere deve stare **in questo file**.
- Il lavoro non pushato. Si pusha a ogni passaggio, non solo a fine giornata.

## Fuori da questo repo (alla prima release)

- DNS e hosting di `moka.mtsolutions.studio`, come per gli altri progetti.
- Voce nella sezione "Progetti open source" della home di mtsolutions.studio, con l'icona.
- Anteprima locale del sito: porta 4712, dopo la 4710 e la 4711 già usate per i siti di MD-Viewer e ClipVault.

## Convenzioni del repo

- Branch `main`. Dopo la pubblicazione: branch `feature/<slug>` e PR.
- Commit in italiano, descrittivi, senza trailer di co-autore.
- `CHANGELOG.md` datato a ogni passaggio significativo, versione bumpata insieme.
- README bilingue IT/EN, come ClipVault e MD-Viewer.
