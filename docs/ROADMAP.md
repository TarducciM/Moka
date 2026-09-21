# Moka — piano di progetto

> **Stato: progettazione.** Non c'è ancora una riga di codice.
>
> Questo file è il punto di ripresa: chi riprende il lavoro, da qualunque PC, parte da qui. Va aggiornato a ogni passaggio significativo, insieme a `CHANGELOG.md`.
>
> Ultimo aggiornamento: 2026-09-21.

## In una riga

Un keep-awake per Windows nello spirito di [Amphetamine](https://apps.apple.com/us/app/amphetamine/id937984704?mt=12) su macOS. Dalla tray scegli se tenere acceso il PC, anche lo schermo, oppure spegnere solo lo schermo lasciando il PC sveglio: a tempo, fino a un'ora precisa, per sempre, o con regole automatiche. **Sui portatili funziona anche a coperchio chiuso.** Un eseguibile piccolo, tutto locale, open source.

**Perché, visto che esiste [PowerToys Awake](https://learn.microsoft.com/en-us/windows/powertoys/awake)**: Awake copre le basi (per sempre, a tempo, fino a un'ora, schermo acceso, `--pid`), ma va installato l'intero PowerToys e non ha regole automatiche, protezione batteria, azione a fine sessione, "spegni solo lo schermo", né gestione del coperchio. In più, sui portatili con standby moderno, a schermo spento non tiene sveglio il PC ([issue #48965](https://github.com/microsoft/powertoys/issues/48965)).

Moka si distingue su tre fronti: le regole automatiche (il punto forte di Amphetamine), il coperchio chiuso, e il funzionamento corretto sui portatili moderni.

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

4. Per lo spike sullo standby moderno e per la 0.2 serve un **portatile**. Per sapere che tipo è:

   ```bash
   powercfg /a
   ```

   "Standby (S0 Low Power Idle)" indica standby moderno, il caso che conta di più. "Standby (S3)" indica sospensione classica. Annotare il risultato nella tabella qui sotto.

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
| Coperchio chiuso | **Funzione di punta**, non avanzata. Poche opzioni semplici, ma tutti i casi limite gestiti (vedi la sezione dedicata). Prima si verifica su un portatile vero con standby moderno, poi si promette. |
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
| Disco USB collegato | Volumi rimovibili (`WM_DEVICECHANGE`) | 0.5 |
| Rete Wi-Fi | Vedi trappola 17 | 0.5, da verificare |

**Stato effettivo** = sessione manuale + regole attive. Vale la modalità più "forte" (schermo acceso batte solo PC).

Il popover dice sempre il perché: *"Sveglio perché: OBS è aperto"*. Se si spegne a mano mentre una regola è attiva, Moka chiede se sospendere le regole per un'ora o fino al prossimo avvio: altrimenti la regola riaccenderebbe tutto cinque secondi dopo.

### Protezioni

- **Soglia batteria** (default 20%, disattivabile): sotto la soglia la sessione finisce da sola e lo dice con una notifica.
- **"Mai a batteria"**, facoltativo.
- **Protezione zaino**, per il coperchio chiuso a batteria: vedi la sezione sul coperchio.

### Presenza

Ogni ~50 secondi, e solo se l'utente è inattivo da almeno 50 secondi (`GetLastInputInfo`), Moka invia un tasto **F15** con `SendInput`. È un tasto che non esiste sulle tastiere comuni e che quasi nessuna app usa.

Effetto: il contatore di inattività di Windows riparte, quindi niente salvaschermo, niente blocco per inattività, niente "Assente" su Teams o Slack. Serve perché le richieste di alimentazione **non fermano il salvaschermo** (lo dice la documentazione di `SetThreadExecutionState`).

- Spenta di default, attivabile per sessione o come impostazione.
- Testo in app, obbligatorio: *"Il blocco per inattività esiste per sicurezza: sui PC di lavoro questa opzione può violare le regole aziendali."*
- **Da verificare** che F15 non faccia niente nelle app più comuni. Alternativa: spostare il mouse di un pixel e riportarlo indietro.

### Diagnostica: "Perché il PC non dorme / si è svegliato?" (0.5)

Su richiesta, con il prompt UAC, esegue:

- `powercfg /requests`
- `powercfg /lastwake`
- `powercfg /waketimers`
- `powercfg /devicequery wake_armed`

Poi spiega il risultato in parole normali (es. "Chrome sta riproducendo audio", "il mouse può riattivare il PC"). Nessuna modifica automatica: solo la spiegazione e, dove serve, il comando da lanciare. L'output di `powercfg` potrebbe essere localizzato (trappola 18).

### Coperchio chiuso

Funzione di punta: ha una sezione tutta sua, [più sotto](#portatili-coperchio-chiuso).

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
moka --lid / --no-lid     questa sessione resta accesa (o no) a coperchio chiuso
moka --restore-lid        rimette l'impostazione del coperchio com'era, poi esce
```

Gli argomenti arrivano all'istanza già aperta tramite il plugin single-instance, come fa ClipVault con `--enable-autostart`. I comandi non restituiscono output ("spara e dimentica", vedi trappola 15). Anche da riga di comando `--then shutdown` passa dal conto alla rovescia.

---

## Portatili: coperchio chiuso

### Il problema, in due righe

Le richieste di alimentazione **non** fermano la sospensione alla chiusura del coperchio: per Windows è un'azione esplicita dell'utente, come il tasto di accensione. Per tenere acceso un portatile chiuso l'unica strada è cambiare, per il tempo necessario, l'impostazione di Windows "Quando chiudo il coperchio" (`GUID_LIDCLOSE_ACTION`: 0 non fare nulla, 1 sospendi, 2 iberna, 3 arresta) e rimetterla com'era dopo.

È l'**unico** punto in cui Moka tocca il piano energetico. Proprio per questo va fatto in modo impeccabile: nessuna impostazione lasciata cambiata, mai, qualunque cosa succeda.

### Cosa vede l'utente

La sezione compare **solo sui portatili** (`SYSTEM_POWER_CAPABILITIES.LidPresent`). Su un fisso non esiste.

**Alla prima apertura su un portatile**, una domanda sola, perché Moka non cambia un'impostazione di sistema senza consenso:

> *Vuoi che Moka tenga acceso il portatile anche a coperchio chiuso?*
> ◉ Sì, quando è in carica *(consigliato)* · ○ Sì, anche a batteria · ○ No, lascia fare a Windows
> ☐ Modalità scrivania: con un monitor esterno collegato, chiudere il coperchio non sospende mai il PC

Finché non si risponde, Moka non tocca niente.

**Nel popover**, una riga in più, solo sui portatili:

```text
│  ☑ Anche a coperchio chiuso    │
```

Il valore iniziale viene dalle Impostazioni, e si può cambiare per la singola sessione. Il tooltip della tray lo dice: *"Acceso, anche a coperchio chiuso · ancora 1 h 12 min"*.

**Nelle Impostazioni, sezione "Coperchio"**:

```text
Durante una sessione, chiudendo il coperchio:
  ○ il PC va in sospensione, come sempre
  ◉ resta acceso, solo se è in carica
  ○ resta acceso anche a batteria
       └ a batteria e senza monitor esterno, sospendi dopo [30 min ▾]

☐ Modalità scrivania: con un monitor esterno collegato, il coperchio
  chiuso non sospende mai il PC, anche senza una sessione attiva
☑ Blocca il PC quando riapro il coperchio

Impostazione di Windows ora: Sospendi (in carica) · Sospendi (a batteria)
```

Mentre Moka la sta cambiando, l'ultima riga diventa: *"Moka l'ha messa temporaneamente su 'Non fare nulla'. Tornerà com'era alla fine."*, con il pulsante **Ripristina ora**. Se l'impostazione è gestita da un criterio aziendale, la sezione si disattiva e lo spiega (vedi più sotto).

Sono tre scelte e due caselle. Tutto il resto lo fa Moka da sola, e lo fa giusto:

### Cosa fa Moka da sola (nessuna opzione, solo comportamento corretto)

| Situazione | Cosa succede |
|---|---|
| La sessione finisce **a coperchio chiuso** | Windows applica l'azione del coperchio solo **nel momento** della chiusura: rimettere l'impostazione dopo non fa sospendere niente, e il portatile resterebbe acceso nello zaino. Quindi Moka ripristina l'impostazione e **poi fa lei ciò che Windows avrebbe fatto**: sospende, iberna o arresta, secondo il valore originale per la fonte di alimentazione del momento. Se è impostato un "…e poi", vince quello. |
| Si stacca l'alimentatore a coperchio chiuso (con "solo se è in carica") | Stessa cosa: Windows non rivaluta il coperchio al cambio di alimentazione, lo fa Moka. |
| Modalità scrivania, si scollega l'ultimo monitor esterno a coperchio chiuso | Se non c'è una sessione, Moka fa ciò che Windows avrebbe fatto. È come si comporta un Mac in clamshell. |
| **Protezione zaino**: a batteria, coperchio chiuso, nessun monitor esterno | Dopo il tempo scelto (default 30 min) Moka fa ciò che Windows avrebbe fatto. Il caso riconosciuto è proprio "portatile chiuso dentro una borsa". Windows non espone la temperatura in modo affidabile senza driver, quindi la protezione è **a tempo**, non a temperatura. |
| Soglia batteria raggiunta a coperchio chiuso | La sessione finisce, e Moka fa ciò che Windows avrebbe fatto. |
| Il coperchio si riapre | Il conto alla rovescia della protezione zaino si annulla. Con "Blocca il PC quando riapro il coperchio" parte subito `LockWorkStation()`: chi apre il portatile trova la schermata di accesso, come dopo una sospensione. |
| L'impostazione di Windows è già "Non fare nulla" | Moka non ha niente da cambiare e lo dice. A fine sessione non fa niente, perché è ciò che Windows farebbe. |

Tutte queste azioni avvengono a coperchio chiuso, dove un conto alla rovescia sullo schermo non lo vede nessuno. Quindi si eseguono senza conto alla rovescia, ma solo dopo **10 secondi di attesa**, per non litigare con un coperchio che si sta riaprendo proprio in quel momento.

### Come si cambia l'impostazione senza mai lasciarla cambiata

1. **Quando**: la modifica va fatta **prima** che il coperchio si chiuda. Si applica quando la condizione diventa vera (parte una sessione con il coperchio abilitato, oppure si collega un monitor in modalità scrivania) e si toglie appena non serve più. Più motivi possono valere insieme (una sessione e la modalità scrivania): un conteggio di riferimenti tiene la modifica finché ne resta almeno uno.
2. **Cosa si scrive**:
   - con "solo se è in carica", **solo** il valore in carica (`PowerWriteACValueIndex`); quello a batteria resta di Windows, che quindi gestisce da sé la batteria anche se Moka dovesse cadere;
   - con "anche a batteria", entrambi.

   Poi `PowerSetActiveScheme`, senza la quale la modifica non ha effetto subito.
3. **Registro delle modifiche, scritto prima di toccare niente**: `%APPDATA%\com.moka.app\lid-override.json` contiene lo schema modificato, quale valore (in carica/a batteria), il valore originale e quello scritto. Il file viene scritto e forzato su disco (`fsync`) **prima** della modifica, e cancellato solo a ripristino avvenuto.
4. **Ripristino**, con queste regole:
   - nello **schema che era stato modificato**, non in quello attivo in quel momento (l'utente potrebbe aver cambiato piano energetico nel frattempo);
   - **solo se il valore è ancora quello scritto da Moka**. Se l'utente l'ha cambiato a mano nel frattempo, si rispetta la sua scelta e si scarta il registro senza toccare niente.

   Se l'utente cambia piano energetico durante una sessione (notifica `GUID_POWERSCHEME_PERSONALITY` / cambio di schema attivo), Moka applica la modifica anche al nuovo schema e la annota nel registro.
5. **Tutte le strade per cui si ripristina**:
   - fine della condizione (sessione finita, monitor scollegato);
   - uscita normale da Moka;
   - spegnimento o disconnessione di Windows (`WM_ENDSESSION`: c'è poco tempo, ma scrivere un valore è istantaneo);
   - **avvio successivo** di Moka, se trova un registro lasciato lì;
   - **`RunOnce`**: mentre la modifica è attiva, Moka scrive in `HKCU\…\RunOnce` il comando `moka.exe --restore-lid`, e lo toglie a ripristino avvenuto. Se Moka cade, o il PC si spegne di colpo, al successivo accesso Windows lo esegue una volta sola, anche se Moka non è impostata per avviarsi con Windows;
   - **disinstallazione**: il disinstallatore esegue `moka.exe --restore-lid` prima di rimuovere i file;
   - il pulsante **Ripristina ora** nelle Impostazioni;
   - il README riporta il comando a mano, per l'ultimo caso possibile (versione portable cancellata mentre la modifica era attiva): `powercfg /setacvalueindex SCHEME_CURRENT SUB_BUTTONS LIDACTION 1`, poi lo stesso con `/setdcvalueindex`, poi `powercfg /setactive SCHEME_CURRENT`.

### Permessi, criteri aziendali, più utenti

- **Permessi**: secondo la [documentazione Microsoft](https://learn.microsoft.com/en-us/windows/win32/power/administrator-overrides), l'ACL predefinita dei piani energetici concede lettura e scrittura agli Authenticated Users. Quindi non dovrebbe servire l'amministratore: **da confermare su un account standard**. Prima di offrire la funzione si controlla con `PowerSettingAccessCheck`.
- **Criteri aziendali**: se l'impostazione del coperchio è imposta da un criterio di gruppo, `PowerSettingAccessCheck` lo segnala. La sezione si disattiva e dice *"Impostazione gestita dalla tua organizzazione"*, invece di fingere di funzionare.
- **Più utenti**: i piani energetici valgono per **tutto il PC**, non per l'utente. Durante la modifica, anche un altro utente che accede con il cambio rapido trova il coperchio su "Non fare nulla". Va scritto nella sezione Coperchio, in piccolo.

### Standby moderno: il rischio più grande del progetto, da verificare per primo

Quasi tutti i portatili recenti usano lo **standby moderno** (S0 Low Power Idle, `SYSTEM_POWER_CAPABILITIES.AoAc`) al posto della sospensione classica S3. Lì il sistema entra in standby quando l'utente "lo manda a dormire": tasto di accensione, coperchio, Start → Sospendi, oppure per **inattività** ([Microsoft](https://learn.microsoft.com/en-us/windows-hardware/design/device-experiences/modern-standby)).

Due progetti hanno già documentato che, **a schermo spento**, il metodo classico non basta:

- [ChargeKeeper #170](https://github.com/0z00z0/ChargeKeeper/issues/170): coperchio chiuso con l'azione rimandata e `ES_SYSTEM_REQUIRED` attivo, eppure standby dopo **32 secondi**;
- [PowerToys #48965](https://github.com/microsoft/powertoys/issues/48965): Awake non tiene sveglio a schermo spento, né a PC bloccato. L'autore propone `PowerSetRequest` con `SystemRequired` più `ExecutionRequired`, ma non è verificato da noi.

Il coperchio chiuso è sempre "schermo spento", quindi questo è esattamente il nostro caso. **Prima di scrivere l'interfaccia del coperchio si fa una prova tecnica** (spike) su un portatile vero con standby moderno. Ipotesi da provare, nell'ordine:

1. `PowerRequestSystemRequired` + `PowerRequestExecutionRequired` bastano, a coperchio chiuso e con "Non fare nulla".
2. Se non bastano: tenere anche `PowerRequestDisplayRequired` in modalità coperchio chiuso. Il pannello interno lo spegne comunque il coperchio, quindi non costa niente, ma per Windows lo schermo "resta acceso" e l'uscita per inattività non scatta.
3. **Blocco del PC**: lo schermo di blocco spegne il display dopo il suo timeout (di solito 60 s), e secondo la documentazione di PowerToys lì le richieste della sessione utente non valgono. Se lo spike lo conferma:
   - la Presenza (F15, che evita il blocco per inattività) diventa parte della modalità coperchio chiuso, e va anticipata;
   - "Blocca il PC quando riapro il coperchio" restituisce la sicurezza al momento giusto.
4. Come si sospende **da codice** un PC con standby moderno, per il "fai ciò che Windows avrebbe fatto": `SetSuspendState` potrebbe non essere la strada. Da provare.
5. Ultima risorsa, solo se tutto il resto fallisce: un piccolo servizio di sistema facoltativo che tiene la richiesta dalla sessione 0, installato solo con l'installer "per tutti". È pesante e va contro la semplicità: si valuta solo con dati alla mano.

**Come si misura, senza fidarsi dell'impressione**:

- uno script che ogni 10 s aggiunge l'ora corrente a un file: un buco nei tempi è uno standby;
- un `ping` continuo da un altro dispositivo;
- un download che avanza;
- a posteriori, `powercfg /sleepstudy` e gli eventi Kernel-Power nel Visualizzatore eventi.

Il risultato decide quali richieste tenere per ciascun modello di alimentazione (S3 o standby moderno). E se qualcosa non si può garantire, l'app lo dice invece di sembrare funzionare.

### Monitor esterno

Si riconosce con `QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS)`, contando i percorsi attivi la cui tecnologia di uscita **non** è interna (`INTERNAL`, `DISPLAYPORT_EMBEDDED`, `UDI_EMBEDDED`). I monitor USB (DisplayLink) contano come esterni. I cambi si ascoltano con `WM_DISPLAYCHANGE` (vedi trappola 23).

### Cosa conta come "fatto", per il coperchio

Serve un **portatile vero**: né un fisso né un emulatore possono dimostrare niente. Meglio ancora due, uno con standby moderno e uno S3. Ogni riga va segnata in `test.md` con data, macchina ed esito.

| Prova | Risultato atteso |
|---|---|
| Sessione in carica, coperchio chiuso 30 min, nessun monitor | Nessun buco nel log a 10 s, ping continuo, download avanzato |
| Stessa cosa a batteria, con "anche a batteria" | Idem, e la protezione zaino sospende dopo il tempo scelto |
| Fine sessione a coperchio chiuso | Il PC si sospende (buco nel log da lì in poi); `powercfg /q SCHEME_CURRENT SUB_BUTTONS LIDACTION` è tornato al valore originale |
| Alimentatore staccato a coperchio chiuso, con "solo se è in carica" | Il PC si sospende |
| Modalità scrivania: monitor collegato, coperchio chiuso, poi monitor scollegato | Resta acceso finché c'è il monitor, poi si sospende |
| Moka chiusa a forza da Task Manager a modifica attiva, poi riaperta | Impostazione ripristinata all'avvio |
| Moka chiusa a forza, poi riavvio del PC senza riaprirla | Impostazione ripristinata all'accesso (`RunOnce`) |
| Durante una sessione, l'utente cambia a mano l'azione del coperchio | A fine sessione Moka **non** la sovrascrive |
| Disinstallazione a modifica attiva | Impostazione ripristinata |
| Account standard (non amministratore) | Funziona, o spiega perché no |
| Coperchio riaperto con "Blocca" attivo | Schermata di accesso |
| PC bloccato (Win+L) a coperchio chiuso, in carica | Esito annotato, qualunque sia: decide l'ipotesi 3 |

---

## Interfaccia

```text
┌─ Moka ─────────────────────────┐
│  ● Acceso · ancora 1 h 12 min  │
│  [ PC ] [ PC + schermo ]       │
│  15m  30m  1h  2h  4h  ∞  ⏰   │
│  ☑ Anche a coperchio chiuso    │
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
  - coperchio (solo sui portatili, vedi la sezione dedicata)
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
- [ ] **Spike standby moderno** su un portatile vero (vedi la sezione sul coperchio): a schermo spento, quali richieste tengono davvero sveglio il PC. È il presupposto della 0.2, e il suo esito va scritto qui prima di andare avanti.

### 0.2.0 — portatili e coperchio chiuso

- [ ] `sysevents.rs`: finestra nascosta su un thread dedicato, che riceve stato del coperchio, fonte di alimentazione, batteria, cambi di monitor e di piano energetico
- [ ] `lid.rs`: lettura e scrittura dell'azione del coperchio, registro delle modifiche, ripristino con tutte le sue strade (`RunOnce` compreso), conteggio dei motivi attivi
- [ ] `actions.rs`: sospendi, iberna, arresta, blocca, spegni schermo (anche su standby moderno, secondo l'esito dello spike)
- [ ] Domanda alla prima apertura su un portatile
- [ ] Riga "Anche a coperchio chiuso" nel popover; sezione Coperchio nelle Impostazioni
- [ ] "Fai ciò che Windows avrebbe fatto" in tutti i casi della tabella
- [ ] Modalità scrivania
- [ ] Protezione zaino
- [ ] Soglia batteria (serve alla protezione zaino)
- [ ] Blocco alla riapertura del coperchio
- [ ] Criteri aziendali e account standard gestiti
- [ ] Presenza anticipata qui, **se** lo spike dimostra che il blocco del PC ferma la sessione
- [ ] Tutta la tabella "Cosa conta come fatto, per il coperchio" provata su un portatile vero

### 0.3.0 — prima release pubblica

- [ ] "…e poi" con conto alla rovescia
- [ ] Notifica 5 minuti prima della fine con "+30 min" (vedi trappola 20)
- [ ] Tasti rapidi globali (accendi/spegni, spegni schermo ora), scelti da una lista di combinazioni sicure, **niente Ctrl+Alt** (trappola 19)
- [ ] Riga di comando
- [ ] Installer NSIS + MSI + portable: pagina "Attività aggiuntive" per l'avvio automatico, e `--restore-lid` alla disinstallazione
- [ ] Auto-update firmato
- [ ] Promemoria stella GitHub
- [ ] `site/` con index, privacy, terms, cookie policy
- [ ] Repo pubblico, poi tag `v0.3.0`

### 0.4.0 — regole automatiche e Presenza

- [ ] Motore delle regole (una regola per file, trait comune, controllo ogni 5 s, eventi dove è semplice)
- [ ] Regole: programma aperto, schermo intero, in chiamata, in carica, monitor esterno, fascia oraria, download in corso, CPU occupata
- [ ] "Sospendi le regole per un'ora"
- [ ] Presenza (se non è già arrivata con la 0.2)

### 0.5.0 — diagnostica e rifiniture

- [ ] Diagnostica "perché non dorme / perché si è svegliato"
- [ ] Regole: disco USB, rete Wi-Fi
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
  lid.rs         azione del coperchio: lettura, modifica, registro, ripristino
  sysevents.rs   finestra nascosta con le notifiche di sistema (coperchio,
                 alimentazione, batteria, monitor, piano energetico)
  capabilities.rs  portatile? standby moderno? ibernazione disponibile?
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
- `windows` (il crate di Microsoft), attivando **solo** le feature usate: `Win32_System_Power`, `Win32_System_Threading`, `Win32_UI_WindowsAndMessaging`, `Win32_UI_Input_KeyboardAndMouse`, `Win32_UI_Shell`, `Win32_System_Registry`, `Win32_System_Shutdown`, `Win32_Devices_Display` (per `QueryDisplayConfig`), …
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
15. L'eseguibile è GUI (`windows_subsystem = "windows"`), quindi niente stdout sul terminale. Per ora i comandi da riga di comando non danno output. Un `moka status` con risposta richiede un piccolo binario console separato oppure `AttachConsole`: da valutare.
16. Icona nella tray: la barra di Windows 10/11 può essere chiara o scura. Leggere `SystemUsesLightTheme` e cambiare variante al volo su `WM_SETTINGCHANGE`.
17. Wi-Fi: da Windows 11 24H2 leggere il nome della rete (SSID) richiederebbe il permesso di posizione (**da verificare**). Alternativa possibile: il nome del profilo di rete tramite Network List Manager.
18. `powercfg /requests` richiede l'amministratore, e l'output potrebbe essere localizzato: il parsing va provato su Windows in italiano e in inglese.
19. Sulle tastiere italiane Ctrl+Alt equivale ad AltGr: nessuna combinazione Ctrl+Alt come tasto rapido predefinito.
20. Le notifiche toast con pulsanti richiedono un AppUserModelID registrato (lo registra l'installer). Verificare cosa supporta davvero `tauri-plugin-notification` su Windows; in ogni caso il clic sulla notifica deve aprire il popover.
21. i18n: una chiave mancante non dà errori, mostra la chiave stessa a schermo. Serve un controllo in CI che italiano e inglese abbiano le stesse chiavi e che ogni `data-i18n` esista.

### Coperchio e standby moderno

22. Windows applica l'azione del coperchio **solo nel momento della chiusura**. La modifica va fatta prima, e rimetterla dopo non fa sospendere niente: a fine sessione a coperchio chiuso la sospensione la deve fare Moka.
23. Una finestra solo-messaggi (`HWND_MESSAGE`) **non riceve i messaggi broadcast**: né `WM_DISPLAYCHANGE` né `WM_SETTINGCHANGE`. Per le notifiche di sistema serve una finestra top-level nascosta (`WS_EX_TOOLWINDOW`, mai mostrata). `RegisterPowerSettingNotification` invece funziona con entrambe.
24. La notifica `GUID_LIDSWITCH_STATE_CHANGE` arriva solo quando Windows ha trovato il coperchio e ne conosce lo stato ([Microsoft](https://learn.microsoft.com/en-us/windows/win32/power/power-setting-guids)). Su un fisso non arriva mai: non aspettarla. Per sapere se il coperchio esiste c'è `LidPresent`.
25. Dopo `PowerWriteACValueIndex`/`PowerWriteDCValueIndex` serve `PowerSetActiveScheme`, altrimenti la modifica non ha effetto subito.
26. Il ripristino va fatto nello schema **modificato**, non in quello attivo, e **solo se il valore è ancora quello scritto da Moka**: altrimenti si cancella una scelta fatta dall'utente nel frattempo.
27. I piani energetici valgono per tutto il PC, non per l'utente.
28. Criterio di gruppo sull'azione del coperchio: la scrittura non ha effetto. Controllare prima con `PowerSettingAccessCheck`, e dirlo.
29. Standby moderno: a schermo spento `ES_SYSTEM_REQUIRED` non ha impedito lo standby in due casi documentati (ChargeKeeper #170, PowerToys #48965). Si risolve con lo spike, non a sentimento.
30. Schermata di blocco: potrebbe annullare le richieste della sessione utente e spegnere il display dopo il suo timeout. Da misurare nello spike.

---

## Limiti da dichiarare all'utente (README e app)

- Moka non impedisce la sospensione **chiesta da te** con il tasto di accensione o da Start → Sospendi. È voluto: sono azioni esplicite. Il coperchio invece si gestisce, ma solo se lo scegli.
- **Schermata di blocco**: comportamento da verificare nello spike e poi documentare. Secondo la documentazione di PowerToys Awake, lì le richieste non valgono.
- **Standby moderno**: quello che lo spike non riesce a garantire va detto nell'app, non scoperto dall'utente.
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
  - il disinstallatore esegue `moka.exe --restore-lid` prima di togliere i file, così non resta mai l'impostazione del coperchio cambiata
- **Auto-update**:
  - chiave minisign generata alla prima release pubblica (0.3) con `tauri signer generate`
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
