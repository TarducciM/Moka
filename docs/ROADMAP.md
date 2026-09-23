# Moka — piano di progetto

> **Stato: 0.5.0, prima release pubblica.** Tutto il codice fino alla 0.5 (diagnostica, regole su USB e rete, memoria del pannello) è scritto e provato su LPT-MIKI; la pubblicazione della prima release aspetta lo spike e i passi di Michele in [`RELEASE.md`](RELEASE.md). In dettaglio: Il nucleo della 0.1 e tutta la parte della 0.2 che non dipende dallo spike sono scritti e verificati su LPT-MIKI (vedi "Verifiche su LPT-MIKI"). Manca lo spike sullo standby moderno, che richiede una persona davanti al portatile: procedura in [`SPIKE.md`](SPIKE.md). Lo spike decide **quali richieste** tenere a coperchio chiuso, non come si cambia e si rimette l'impostazione di Windows, che è già fatto e provato.
>
> Questo file è il punto di ripresa: chi riprende il lavoro, da qualunque PC, parte da qui. Va aggiornato a ogni passaggio significativo, insieme a `CHANGELOG.md`.
>
> Ultimo aggiornamento: 2026-09-22.

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

2. Installare e provare:

   ```bash
   npm install
   npm run dev                                   # Moka in modalità sviluppo
   cargo test --manifest-path src-tauri/Cargo.toml
   npm run check                                 # sintassi JS e traduzioni
   ```

   Lo scheletro è Tauri 2 **vanilla**, allineato a ClipVault: `src/` servito così com'è (`frontendDist: "../src"`), `withGlobalTauri: true`, nessun bundler. Le icone si rigenerano con `npm run icons` (vedi `scripts/icons.mjs`).

3. Prossimo obiettivo: chiudere la **0.1.0**. Mancano lo spike (`docs/SPIKE.md`) e le righe aperte di `test.md`.

4. Per lo spike e per la 0.2 serve un **portatile**. Per sapere che tipo è:

   ```bash
   powercfg /a
   ```

   "Standby (S0 Low Power Idle)" indica standby moderno, il caso che conta di più. "Standby (S3)" indica sospensione classica. Annotare il risultato nella tabella qui sotto.

| Data | Macchina (`hostname`) | Rust | Cosa è stato fatto |
|---|---|---|---|
| 2026-09-21 | PC-MIKY | no | solo progettazione, nessun codice |
| 2026-09-22 | LPT-MIKI (Acer Nitro ANV16S-41, portatile, standby moderno connesso, niente S3) | 1.98, MSVC, VS Build Tools 2022, Node 24, WebView2 153 | pianificazione chiusa, check della macchina, nucleo della 0.1, CI, strumento per lo spike; poi 0.0.2: coperchio (modifica e ripristino, scrivania, zaino), eventi di sistema, soglia batteria; 0.0.3: "…e poi", installer, aggiornamenti, sito; 0.0.4: regole automatiche e Presenza; 0.0.5: diagnostica, regole USB e rete, memoria del pannello |

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

### Decisioni prese scrivendo il codice (2026-09-22)

| Tema | Decisione |
|---|---|
| Identifier | `com.moka.app` resta: è coerente con `com.clipvault.app`. Il CLI di Tauri potrebbe avvisare per il suffisso `.app` (conflitto con i bundle macOS): su un'app solo Windows è innocuo. |
| Clic sinistro | **Apre il pannello** di default, perché è la cosa che si scopre da soli. Chi preferisce il comportamento alla Caffeine lo cambia in Impostazioni ("accende o spegne con l'ultima scelta"). Il clic destro apre sempre il menu completo. |
| Modalità predefinita | Niente impostazione a parte: la scelta "Solo il PC / PC e schermo" del pannello **resta** fra una sessione e l'altra, ed è quella la predefinita. Due concetti per la stessa cosa confondevano. |
| Riga di comando | Anticipata alla 0.1 (è logica pura, e passa già dal single-instance): `--for`, `--until`, `--forever`, `--screen`, `--on`, `--toggle`, `--off`, `--screen-off`, `--quit`. Restano per dopo `--while*` (0.4), `--then` (0.3), `--lid` e `--restore-lid` (0.2). Negli script `--for 2h` senza `--screen` vuol dire sempre "solo il PC", qualunque cosa sia stata cliccata l'ultima volta nel pannello. |
| "Spegni lo schermo ora" senza sessione | Parte una sessione "solo il PC" **finché non lo spegni**: chi spegne lo schermo e se ne va non sa quando tornerà. Con una sessione "PC e schermo" in corso, questa passa a "solo il PC", altrimenti lo schermo si riaccenderebbe e resterebbe acceso. |
| Ripresa della sessione | Dopo un riavvio **dell'app** sì; dopo un riavvio del PC o un nuovo accesso no. Si riconosce con due controlli insieme: lo stesso avvio di Windows (tick e orologio) **e** la stessa sessione di accesso (LUID del token). Il secondo serve per l'avvio rapido: "Arresta il sistema" non riavvia il kernel, e il tick continua a contare. "Esci" chiude la sessione per davvero. |
| Impostazioni | La finestra si crea solo quando si apre e si distrugge alla chiusura: una WebView2 costa decine di MB e Moka passa quasi tutto il tempo con le Impostazioni chiuse. Salvataggio immediato, campo per campo, senza pulsante "Salva". |
| Testi | Una sola fonte: `src/locales/{it,en}.json`, inclusi da Rust (menu, tooltip, motivo della richiesta) e letti dalle pagine. Il pannello non calcola niente: riceve da Rust i testi già tradotti e formattati. |
| Icona della tray | A 16 px (scala 100%) è disegnata pixel per pixel: il disegno vettoriale lì si impastava (vapore illeggibile, foro del manico sparito). Da 20 px in su è il vettoriale. |
| Durate | Da 1 minuto a 7 giorni. Si scrivono come `45m`, `2h`, `1h30m`, `1.5h`, `90`. |
| WebView2 senza GPU | `--disable-gpu` nelle finestre di Moka. Il pannello è statico e non ha bisogno dell'accelerazione, mentre il processo GPU di WebView2 pesava 73 MB privati a riposo; senza, 14 MB. Il pannello si disegna identico (verificato con uno screenshot della build di release). |

### Decisioni prese scrivendo la parte portatili (0.0.2, 2026-09-22)

| Tema | Decisione |
|---|---|
| Consenso | Finché la domanda sul coperchio non ha risposta, Moka non tocca niente, nemmeno con la modalità scrivania (che si sceglie nella stessa domanda). Scegliere un'opzione nelle Impostazioni vale come risposta. |
| "Anche a coperchio chiuso" | Come la modalità, **resta** da una sessione all'altra (predefinito: sì). La riga compare nel pannello solo se l'utente ha scelto "in carica" o "anche a batteria". Anche il menu della tray ha la voce. |
| "Ripristina ora" | Rimette subito l'impostazione di Windows, toglie il coperchio alla sessione in corso (che continua) e sospende ogni modifica, modalità scrivania compresa, **fino alla prossima sessione**: altrimenti Moka la riapplicherebbe un attimo dopo. |
| Soglia batteria | Scatta **scendendo** sotto il valore (predefinito 20%, scelte da 10 a 50, o mai). Una sessione avviata con la batteria già sotto la soglia non si interrompe: l'ha chiesta l'utente, sapendolo. Si riarma quando si torna in carica o sopra la soglia. Lo dice con una notifica. |
| Protezione zaino | Scelte: 10, 15, 30 (predefinito), 60, 120 minuti, o mai. Vale solo con "anche a batteria", a coperchio chiuso e senza monitor esterni. |
| `moka --restore-lid` | Gestito in `main.rs` **prima** di avviare Tauri: niente finestre e niente single-instance. Al prossimo accesso `RunOnce` e l'avvio automatico partono insieme, e con il single-instance uno dei due inoltrerebbe l'altro e l'app potrebbe non partire. |
| All'avvio | Se c'è un registro lasciato lì, prima si rimette tutto com'era, **poi** (se la sessione riprende) si riapplica. Invertendo l'ordine, Moka leggerebbe "non fare nulla" come valore originale e a fine sessione lo "rimetterebbe", lasciando l'impostazione cambiata per sempre. Verificato: dopo crash e riapertura il registro dice ancora "sospendi". |
| Richieste a coperchio chiuso | Con la modifica attiva su un PC con standby moderno Moka tiene anche `ExecutionRequired` (ipotesi 1 dello spike). Non costa niente; lo spike dirà se basta o se è inutile. |
| Sospendere da codice | `SetSuspendState`; se su standby moderno viene rifiutata, il ripiego è spegnere lo schermo senza richieste attive (ipotesi 4 dello spike). |

### Decisioni prese scrivendo le regole (0.0.4, 2026-09-22)

| Tema | Decisione |
|---|---|
| Motore | Un solo tipo `Rule` con un `RuleKind` per ogni regola (`rules.rs`, logica pura e testata), invece di un trait con un file per regola: le regole sono dati che vanno salvati, confrontati e mostrati, e un enum lo fa senza codice in più. Le sonde stanno in `probes.rs` e girano **fuori dal lock**, su un thread che controlla ogni 5 s. |
| Sonde a richiesta | Gira solo ciò che serve alle regole attive: senza una regola sul download, niente contatori di rete; senza una regola sul processore, niente `GetSystemTimes`. |
| Isteresi | Download e processore restano veri per 2 minuti dopo l'ultima volta sopra soglia (un download ha pause, una compilazione ha fasi leggere); la chiamata per 30 s (il microfono si chiude fra una riunione e la successiva); le altre finiscono subito. |
| Doppioni | Due regole identiche non si possono creare ("cambia quella che c'è"); al massimo 20. Programma, soglie e giorni vengono validati in Rust: la pagina non può scrivere una regola che il codice non avrebbe scritto. |
| Coperchio | Le regole seguono la stessa scelta "Anche a coperchio chiuso" delle sessioni (l'ultima fatta nel pannello), con le stesse protezioni (solo in carica, zaino, scrivania). |
| "…e poi" di una regola | Parte solo quando la regola finisce **da sola** e niente altro tiene acceso il PC. Non parte se la regola viene disattivata, eliminata, sospesa o fermata dalla soglia batteria. Se durante il conto alla rovescia una regola torna vera (il download riparte), il conto si annulla. La finestrella dice quale regola è finita. |
| Soglia batteria | Ferma anche le regole: sono automatiche, nessuno le ha chieste adesso. Una sola notifica, anche se si fermano insieme sessione e regole. |
| Sospendere | "Per un'ora" o "fino al riavvio di Moka" (non si salva su disco). Sospendere ferma anche i `--while` in corso: chi spegne vuole il PC libero. Un `--while` chiesto durante la pausa vale, perché è una richiesta esplicita. |
| Spegnere con una regola attiva | Dal pannello Moka chiede prima (sospendi per un'ora, fino al riavvio, annulla). Dal clic sull'icona, dal menu e dal tasto rapido non si può chiedere: lì "spegni" sospende le regole per un'ora. Il menu ha anche la voce "Sospendi le regole per un'ora" / "Riprendi le regole", solo se ci sono regole. |
| `--while` / `--while-pid` | Regole temporanee, mai salvate, che vivono finché vive il processo. Se il processo non compare entro 10 s la regola si scarta con una notifica ("non è in esecuzione"). |
| Suggerimenti nel modulo | I programmi con una finestra visibile, meno quelli che ospitano la shell (`applicationframehost.exe`, `textinputhost.exe`…): il campo resta libero, l'elenco è solo un aiuto. |
| Presenza | Controllo ogni 10 s: F15 solo se Moka sta tenendo sveglio il PC e l'utente è fermo da almeno 50 s. Spenta di default, con l'avviso obbligatorio accanto all'interruttore. |

### Decisioni prese scrivendo la diagnostica (0.0.5, 2026-09-22)

| Tema | Decisione |
|---|---|
| Da dove si legge | Quasi tutto **senza amministratore**: il registro eventi di sistema (`Kernel-Power` 506/507 per lo standby moderno, `Power-Troubleshooter` 1 per sospensione e ibernazione) letto come XML con `EvtQuery`; i dispositivi con `DevicePowerEnumDevices` (la fonte di `powercfg /devicequery wake_armed`); le impostazioni di sospensione dello schema attivo. Niente processi da lanciare, niente testo di Windows da interpretare. |
| La lingua di Windows | I motivi di entrata e uscita dallo standby sono **codici** (`POWER_MONITOR_REQUEST_REASON`): Moka li traduce da sé. Solo quelli di cui è sicura; gli altri restano "motivo n. X di Windows", invece di un'ipotesi. |
| Chi tiene sveglio adesso | Solo `powercfg /requests` lo sa, e vuole l'amministratore. Si esegue **solo** quando l'utente preme "Mostra chi lo tiene sveglio", con il prompt di Windows, insieme a `/waketimers` (un prompt per entrambi). L'uscita va su file temporanei, cancellati subito dopo. Categorie (`SYSTEM:`) e tipi (`[PROCESS]`) non sono tradotti da Windows: il parser guarda solo quelli e ignora le righe "nessuno" di ogni lingua. Senza `SYSTEM:` nell'uscita non risponde "nessuno": dice che Windows non ha risposto. |
| Cosa dice senza amministratore | Lo stato di esecuzione del sistema (senza nomi) basta per "un altro programma chiede di tenerlo sveglio", quando Moka è spenta. |
| Le cause più comuni, in cima | "Sospensione dopo: mai" e l'audio aperto negli standby (dal campo `AudioPlaying` e dal tempo a basso consumo dell'evento 507) si segnalano come probabili risposte. Su LPT-MIKI sono uscite entrambe, vere. |
| Dove sta | Una scheda delle Impostazioni (al posto di "Verifica") e la voce "Perché non dorme?…" nel menu della tray, che apre le Impostazioni già lì e fa il controllo. |
| Rumore | Gli standby sotto il minuto non si elencano; si mostrano gli ultimi 8. |

### Decisioni prese per le regole USB e rete (0.0.5)

| Tema | Decisione |
|---|---|
| Disco USB | Conta il **bus** del volume (`IOCTL_STORAGE_QUERY_PROPERTY`, `BusTypeUsb`), non il tipo di unità: un disco esterno USB per Windows è "fisso". Il volume si apre con accesso 0 (niente amministratore, niente disco svegliato). Un lettore di schede vuoto non ha un volume da interrogare, quindi non conta. Un solo tipo di regola, "un disco USB qualsiasi": la chiavetta precisa si aggiungerà se qualcuno la chiede. |
| Rete | Il nome **come lo mostra Windows** (Network List Manager), Wi-Fi o cavo, confrontato senza maiuscole. Non l'SSID: da Windows 11 24H2 leggerlo vuole il permesso di posizione (trappola 17), il nome della rete no. Resta vera 30 s dopo la disconnessione, per i Wi-Fi che cadono e si riconnettono. |

### Memoria del pannello (0.0.5)

Quando il pannello è nascosto Moka chiede a WebView2 `MemoryUsageTargetLevel = Low` (da WebView2 114; prima non fa niente) e lo rimette normale prima di mostrarlo. È la prima delle due strade annotate sotto per la 0.0.1. Misurato su LPT-MIKI, processi WebView2 di Moka a pannello nascosto:

- 0.0.5: **95 MB** di working set (88 subito dopo averlo nascosto; 254 a pannello aperto);
- 0.0.3 nascosta da ore, senza la chiamata: 138 MB;
- memoria privata invariata, ~103 MB per entrambe: WebView2 restituisce la RAM fisica, non quella allocata.

**La seconda strada — creare il pannello solo quando si apre — è stata provata il 2026-09-22 e scartata.** Funzionava (finestra creata su un thread a parte, mostrata quando la pagina si era disegnata e misurata, così compariva già della misura giusta), e con il pannello mai aperto Moka stava a **5,3 MB in tutto, zero processi WebView2**. Il problema è il prezzo della prima apertura, misurato su LPT-MIKI:

| | tempo |
|---|---|
| prima apertura del pannello, con il runtime WebView2 spento | **3,2 s** |
| finestra Impostazioni, con il runtime già vivo | 0,2 s |
| la seconda istanza che porta il comando (`moka --off`) | 0,1 s |

Quei tre secondi non sono il pannello: sono l'**avvio a freddo del runtime WebView2**, che tocca al primo webview del processo. Per una finestra che deve comparire sotto il dito appena si clicca sull'icona non è accettabile, e scaldare il runtime all'avvio vorrebbe dire creare comunque un webview, cioè rinunciare al risparmio.

C'è anche un effetto secondario che conta: tenendo il pannello creato, il runtime resta caldo anche per la **finestrella degli avvisi** ("si spegne tra 5 minuti", il conto alla rovescia di "…e poi"), che è pigra e deve comparire puntuale.

Resta quindi il pannello creato all'avvio, con la memoria bassa mentre è nascosto. Se un giorno si volesse riprovare, il modo giusto non è la creazione pigra ma un modo per scaldare il runtime senza pagarne la memoria: oggi non esiste.

**Misure della build di release 0.0.1** (LPT-MIKI, 2026-09-22):

- eseguibile 3,5 MB; installer NSIS 1,3 MB; MSI 1,8 MB;
- memoria privata a riposo: `moka.exe` 5 MB più WebView2 103 MB in 6 processi (erano 159 con la GPU).

WebView2 resta la voce più pesante. Due strade da valutare, in quest'ordine:

1. abbassare la memoria del pannello quando è nascosto (`MemoryUsageTargetLevel` di WebView2);
2. creare il pannello solo quando si apre, come già le Impostazioni, al prezzo di qualche decimo di secondo alla prima apertura.

---

## Verifiche su LPT-MIKI (2026-09-22)

Il "check veloce" prima di scrivere codice, fatto sulla macchina e non a memoria. Ogni riga dice come ripeterla.

| Cosa | Risultato | Come si ripete |
|---|---|---|
| Tipo di PC | Portatile, standby moderno **connesso alla rete**, S3 non disponibile (anche Device Guard lo disattiva), ibernazione sì | `powercfg /a`, oppure `spike info` |
| Azione del coperchio | "Sospendi" in carica e a batteria. ⚠️ L'impostazione è **nascosta** (`ATTRIB_HIDE`): `powercfg /q` non la mostra affatto, serve `powercfg /qh` (vedi trappola 31) | `powercfg /qh SCHEME_CURRENT SUB_BUTTONS LIDACTION` |
| Criteri aziendali | Nessuno sul coperchio: `PowerSettingAccessCheck` dà via libera | `spike info` |
| Scrivere l'azione del coperchio senza amministratore | **Funziona** su questo account (amministratore con token non elevato). Su un account standard vero resta da provare | `spike lid-write-check` (riscrive il valore che c'è già: non cambia niente) |
| Vedere le richieste senza amministratore | **Si può**: `CallNtPowerInformation(SystemExecutionState)` riflette le richieste attive (0x0 → 0x3 con SYSTEM+DISPLAY → 0x0 al rilascio). Quindi "Moka tiene davvero sveglio il PC" si verifica anche senza `powercfg /requests` | `spike info` |
| Moka 0.0.1 in funzione | CLI inoltrata all'istanza aperta, scadenza, rilascio dopo chiusura forzata, ripresa, `--quit`, Impostazioni, lingua, contrasti: tutto in `test.md` | `test.md` |
| Monitor esterni | **2** (due 2560×1440; il pannello interno non risulta attivo) | `spike info` |
| Moka 0.0.2, coperchio | Modifica "solo in carica" e "anche a batteria", ritorno com'era a fine sessione, all'uscita e con "Ripristina ora"; crash con `RunOnce` e con la riapertura; scelta dell'utente rispettata; modalità scrivania senza sessione. Tutto provato sull'impostazione **vera**, letta ogni volta con `spike info`: dettagli in `test.md` | `test.md` |
| Diagnostica (0.0.5) | 200 eventi di standby e sospensione letti in 45 ms senza amministratore; dispositivi che possono svegliarlo identici a `powercfg /devicequery wake_armed`; sospensione "mai" in carica e 2 h a batteria, come `powercfg /qh`. E una scoperta: in **tutti** gli ultimi standby il PC è rimasto attivo (0% a basso consumo) con un audio aperto | `spike diagnose` |
| Sonde delle regole (0.0.4) | Processi (~195), programmi con finestra, velocità di download e carico del processore letti ogni 5 s; schermo intero e chiamata "no" a riposo. ⚠️ Con un desktop remoto aperto (RustDesk, TeamViewer) entrano ~1 MB/s **continui**: la regola sul download a 1 MB/s risulta vera (trappola 47) | `spike probes --seconds 60` |

---

## Cosa fa

### Modalità

| Modalità | Effetto | Come |
|---|---|---|
| **PC acceso** | Niente sospensione; lo schermo segue il piano energetico | `PowerRequestSystemRequired` |
| **PC e schermo accesi** | Niente sospensione né spegnimento dello schermo | `PowerRequestSystemRequired` + `PowerRequestDisplayRequired` |
| **Spegni schermo ora** | Monitor spento subito, PC sveglio | `SC_MONITORPOWER` (valore 2) + `PowerRequestSystemRequired` |

⚠️ "Spegni schermo ora" è proprio "schermo spento, PC sveglio": sui portatili con standby moderno è lo stesso rischio del coperchio chiuso. Nella 0.0.1 usa `SystemRequired` come le altre; **quali richieste servano davvero lo decide lo spike** (prove A–C in `docs/SPIKE.md`), e finché non c'è l'esito non va promesso nel README.

Durate: 15 min · 30 min · 1 h · 2 h · 4 h · fino alle HH:MM · finché non lo spengo. La lista è personalizzabile.

Il **motivo** della richiesta (`REASON_CONTEXT`) è una frase leggibile e tradotta, per esempio "Moka: sessione fino alle 18:30" o "Moka: OBS è aperto". Chi apre `powercfg /requests` vede chi tiene sveglio il PC e perché, non un processo anonimo.

### "…e poi" — azione a fine sessione

Alla fine di una sessione a tempo, o quando una regola smette di valere, Moka può: non fare niente (default), spegnere lo schermo, bloccare il PC, sospenderlo, ibernarlo o spegnerlo.

L'azione parte **sempre dopo un conto alla rovescia di 60 secondi annullabile**, mai subito. Uso tipico: *"sveglio finché ffmpeg non finisce, poi sospendi"*.

Se il PC si è sospeso comunque durante la sessione (coperchio chiuso, tasto di accensione) e al risveglio la scadenza è già passata, la sessione finisce **senza** eseguire l'azione: niente spegnimento a sorpresa appena riapri il portatile.

### Regole automatiche ("tieni sveglio mentre…")

| Regola | Come si rileva | Note |
|---|---|---|
| Un programma è aperto | Elenco dei processi (Toolhelp32) ogni 5 s | Per nome dell'eseguibile; il modulo suggerisce i programmi con una finestra aperta |
| App a schermo intero | `SHQueryUserNotificationState` (`QUNS_BUSY`, `QUNS_RUNNING_D3D_FULL_SCREEN`, `QUNS_PRESENTATION_MODE`) | Video, giochi, presentazioni |
| In chiamata | Registro `CapabilityAccessManager\ConsentStore\{microphone,webcam}` (anche `NonPackaged`): `LastUsedTimeStop = 0` significa in uso | **Da verificare** con Teams, Zoom e Meet nel browser |
| In carica | `GetSystemPowerStatus` + `RegisterPowerSettingNotification(GUID_ACDC_POWER_SOURCE)` | |
| Monitor esterno collegato | Numero di monitor (`EnumDisplayMonitors`, `WM_DISPLAYCHANGE`) | "Modalità scrivania" |
| Download in corso | Contatori delle interfacce di rete (`GetIfTable2`), soglia 100 KB/s, 500 KB/s, 1 MB/s o 5 MB/s | Vale l'interfaccia più veloce, non la somma (trappola 46); finisce dopo 2 minuti sotto soglia |
| CPU occupata | `GetSystemTimes`, soglia 25, 50 o 75% | Render, compilazioni; finisce dopo 2 minuti sotto soglia |
| Fascia oraria | Giorni della settimana + orari | Es. lun-ven 9-18 |
| Disco USB collegato | Bus dei volumi (`IOCTL_STORAGE_QUERY_PROPERTY`) | 0.5; non ancora provato con un disco vero |
| Rete connessa | Nome della rete dal Network List Manager, Wi-Fi o cavo | 0.5; vedi trappola 17 |

**Stato effettivo** = sessione manuale + regole attive. Vale la modalità più "forte" (schermo acceso batte solo PC).

Il popover dice sempre il perché: *"Sveglio perché: OBS è aperto"*. Se si spegne a mano mentre una regola è attiva, Moka chiede se sospendere le regole per un'ora o fino al prossimo avvio: altrimenti la regola riaccenderebbe tutto cinque secondi dopo.

### Protezioni

- **Soglia batteria** (default 20%, disattivabile): sotto la soglia la sessione finisce da sola e lo dice con una notifica.
- **"Mai a batteria"**, facoltativo.
- **Protezione zaino**, per il coperchio chiuso a batteria: vedi la sezione sul coperchio.

### Presenza

Ogni ~50 secondi, e solo se l'utente è inattivo da almeno 50 secondi (`GetLastInputInfo`), Moka invia un tasto **F15** con `SendInput`. È un tasto che non esiste sulle tastiere comuni e che quasi nessuna app usa.

Effetto: il contatore di inattività di Windows riparte, quindi niente salvaschermo, niente blocco per inattività, niente "Assente" su Teams o Slack. Serve perché le richieste di alimentazione **non fermano il salvaschermo** (lo dice la documentazione di `SetThreadExecutionState`).

- Spenta di default, attivabile nelle Impostazioni (sezione Presenza).
- Testo in app, obbligatorio: *"Il blocco per inattività esiste per sicurezza: sui PC di lavoro questa opzione può violare le regole aziendali."*
- Verificato su LPT-MIKI che F15 azzera davvero il contatore di inattività di Windows (`spike probes`: sale fino a 50 s, al controllo dopo torna a 0). **Da verificare** che non faccia niente nelle app più comuni e che tenga "presente" Teams. Alternativa, se servisse: spostare il mouse di un pixel e riportarlo indietro.

### Diagnostica: "Perché il PC non dorme / si è svegliato?" (0.5, fatta: vedi le decisioni della 0.0.5)

Su richiesta, con il prompt UAC, esegue:

- `powercfg /requests`
- `powercfg /lastwake`
- `powercfg /waketimers`
- `powercfg /devicequery wake_armed`

Poi spiega il risultato in parole normali (es. "Chrome sta riproducendo audio", "il mouse può riattivare il PC"). Nessuna modifica automatica: solo la spiegazione e, dove serve, il comando da lanciare. L'output di `powercfg` potrebbe essere localizzato (trappola 18).

### Coperchio chiuso

Funzione di punta: ha una sezione tutta sua, [più sotto](#portatili-coperchio-chiuso).

### Riga di comando

Già nella 0.0.1 (`src-tauri/src/cli.rs`):

```text
moka                      apre il pannello
moka --for 2h             sveglio per 2 ore (accetta anche 90m, 1h30m, 1.5h)
moka --until 18:30        fino alle 18:30 (anche 18.30)
moka --forever            finché non lo spengo
moka --screen             anche lo schermo (si combina con le altre)
moka --on                 accende con l'ultima scelta fatta nel pannello
moka --toggle             accende o spegne
moka --off                termina la sessione
moka --screen-off         spegne subito lo schermo
moka --quit               chiude Moka
```

Arrivati dopo:

```text
moka --lid / --no-lid     questa sessione resta accesa (o no) a coperchio chiuso            (0.2)
moka --restore-lid        rimette l'impostazione del coperchio com'era, poi esce            (0.2)
moka --then sleep         a fine sessione: display-off | lock | sleep | hibernate | shutdown   (0.3)
moka --while ffmpeg.exe   finché un processo con quel nome è aperto (con --screen, --then)  (0.4)
moka --while-pid 1234     finché il processo 1234 è vivo                                    (0.4)
moka --pause-rules        sospende le regole per un'ora (--pause-rules=2h per un altro tempo) (0.4)
moka --resume-rules       le riattiva                                                       (0.4)
```

Gli argomenti arrivano all'istanza già aperta tramite il plugin single-instance, come fa ClipVault con `--enable-autostart` (che Moka accetta già, insieme a `--disable-autostart`, per l'installer della 0.3). I comandi non restituiscono output ("spara e dimentica", vedi trappola 15). Anche da riga di comando `--then shutdown` passa dal conto alla rovescia.

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
| Fine sessione a coperchio chiuso | Il PC si sospende (buco nel log da lì in poi); `powercfg /qh SCHEME_CURRENT SUB_BUTTONS LIDACTION` è tornato al valore originale (`/qh`, non `/q`: vedi trappola 31) |
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

- **Tray**: clic sinistro apre il pannello (in Impostazioni si può fargli accendere e spegnere con l'ultima scelta), clic destro apre il menu completo (durate, modalità, regole, Impostazioni, Esci). Il tooltip mostra lo stato e il tempo residuo.
- **Primo avvio**: Windows 11 mette le icone nuove fra quelle nascoste (^). Il pannello si apre da solo con un benvenuto che lo dice e spiega come portare l'icona sulla barra; resta finché non si preme "Ho capito".
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

- [x] Scheletro Tauri 2 vanilla, identifier `com.moka.app`
- [x] `power.rs`: richieste di alimentazione con motivo leggibile
- [x] Le tre modalità; durate, "fino alle", "per sempre" (la terza, "spegni lo schermo ora", aspetta l'esito dello spike per i portatili con standby moderno)
- [x] Icona nella tray con tre stati e varianti chiara/scura (cambio al volo, senza polling), sei dimensioni scelte secondo i DPI, tooltip con tempo residuo
- [x] Pannello (finestra dichiarata in `tauri.conf.json`; chiuderla la nasconde; altezza adattata al contenuto; accanto all'icona, dentro l'area di lavoro)
- [x] Menu del clic destro
- [x] Impostazioni: avvio automatico, lingua, clic sull'icona, durate rapide
- [x] Single-instance, con la riga di comando (anticipata dalla 0.3)
- [x] i18n IT/EN, con controllo automatico delle chiavi (`tests/i18n.test.mjs`)
- [x] Sessione salvata su disco e ripresa dopo un riavvio dell'app, **non** dopo un riavvio del PC o un nuovo accesso
- [x] CI (`fmt`, `clippy -D warnings`, `test` su Windows; sintassi JS, traduzioni, versioni allineate e audit su Linux)
- [x] `test.md` con la verifica su un PC vero (parziale: le righe aperte vogliono le mani sul PC)
- [ ] **Spike standby moderno** su un portatile vero: strumento pronto (`src-tauri/examples/spike.rs`), procedura in `docs/SPIKE.md`. Serve una persona davanti al portatile. È il presupposto della 0.2, e il suo esito va scritto qui prima di andare avanti.
- [ ] Le righe aperte di `test.md`

### 0.2.0 — portatili e coperchio chiuso

- [x] `sysevents.rs`: finestra nascosta su un thread dedicato, che riceve stato del coperchio, fonte di alimentazione, batteria, cambi di monitor e di piano energetico, sospensione/ripresa e fine della sessione di Windows
- [x] `lidoverride.rs` (+ primitive in `lid.rs`): registro delle modifiche, ripristino con tutte le sue strade (`RunOnce` compreso), più schemi energetici, scelta dell'utente rispettata. Provato con un Windows finto nei test e con quello vero su LPT-MIKI
- [x] `lidplan.rs`: i motivi attivi (sessione, scrivania) e "fai ciò che Windows avrebbe fatto", logica pura con un test per ogni riga della tabella
- [x] `actions.rs`: sospendi, iberna, arresta, blocca (spegni schermo era già in `sys.rs`). **Non provate** su macchina: spegnerebbero il PC che esegue i test
- [x] Domanda alla prima apertura su un portatile
- [x] Riga "Anche a coperchio chiuso" nel pannello e nel menu; sezione Coperchio nelle Impostazioni, con l'impostazione di Windows e "Ripristina ora"
- [x] "Fai ciò che Windows avrebbe fatto" in tutti i casi della tabella (logica; l'esecuzione vera aspetta il coperchio chiuso)
- [x] Modalità scrivania (provata con due monitor esterni)
- [x] Protezione zaino (logica)
- [x] Soglia batteria, con notifica
- [x] Blocco alla riapertura del coperchio (logica)
- [x] Criteri aziendali: la sezione si disattiva e lo dice. Account standard: da provare
- [ ] Presenza anticipata qui, **se** lo spike dimostra che il blocco del PC ferma la sessione
- [ ] Tutta la tabella "Cosa conta come fatto, per il coperchio" provata su un portatile vero: le righe che non chiedono di chiudere il coperchio sono fatte (vedi `test.md`), le altre aspettano lo spike

### 0.3.0 — prima release pubblica

- [x] "…e poi" con conto alla rovescia di 60 s (annulla, +30 min, adesso), in una finestrella che non ruba il focus; a coperchio chiuso solo i 10 s di garanzia, senza finestra; vince su ciò che Windows avrebbe fatto
- [x] Avviso 5 minuti prima della fine con "+30 min" (nella stessa finestrella, non una notifica di Windows: vedi trappola 20)
- [x] Tasti rapidi globali (accendi/spegni, spegni schermo ora), da una lista sicura: Ctrl+Maiusc+F7…F11 e Ctrl+Maiusc+Pausa, **niente Ctrl+Alt** (trappola 19)
- [x] Riga di comando: `--then` (da solo cambia il "…e poi" della sessione in corso)
- [x] Installer NSIS (pagina "Attività aggiuntive", italiano e inglese) + MSI + portable; alla disinstallazione e agli aggiornamenti Moka si chiude in modo pulito e l'impostazione del coperchio torna com'era
- [x] Auto-update firmato: chiave generata, controllo all'avvio e ogni 24 ore, **mai durante una sessione**
- [x] Promemoria stella GitHub
- [x] `site/` con index, privacy, terms, cookie policy (bozze legali da far rivedere)
- [x] `release.yml`: NSIS + MSI + portable, `latest.json` costruito a mano, bozza, controllo tag/versione, pulizia se fallisce
- [x] Secret su GitHub, repo pubblico, tag e pubblicazione: fatti il 2026-09-23 con il numero **0.5.0** (nel frattempo erano arrivate anche le tappe 0.4 e 0.5). Release: [v0.5.0](https://github.com/TarducciM/Moka/releases/tag/v0.5.0), updater verificato dall'esterno
- [ ] **Michele**: copia di sicurezza della chiave privata dell'updater fuori da questo PC, DNS del sito, voce nella home MTSolutions, revisione legale delle pagine

### 0.4.0 — regole automatiche e Presenza

- [x] Motore delle regole (`rules.rs`: un enum invece di un file per regola, vedi le decisioni della 0.0.4; controllo ogni 5 s su un thread, sonde fuori dal lock e solo quelle che servono)
- [x] Regole: programma aperto, schermo intero, in chiamata, in carica, monitor esterno, fascia oraria, download in corso, CPU occupata; ognuna con modalità e "…e poi" propri
- [x] Il pannello dice perché è acceso ("Acceso · notepad.exe è aperto", "Anche: …")
- [x] "Sospendi le regole per un'ora" (o fino al riavvio), dal pannello, dal menu, dalle Impostazioni e da riga di comando
- [x] `--while`, `--while-pid`, `--pause-rules`, `--resume-rules`
- [x] Presenza (F15), spenta di default, con l'avviso
- [ ] Le righe 0.0.4 aperte di `test.md`: schermo intero, chiamata con Teams/Zoom/Meet, monitor esterno, voce del menu, F15 nelle app comuni

### 0.5.0 — diagnostica e rifiniture

- [x] Diagnostica "perché non dorme / perché si è svegliato": registro eventi, dispositivi, impostazioni di sospensione senza amministratore; `powercfg /requests` e `/waketimers` con il prompt, solo su richiesta; scheda nelle Impostazioni e voce nel menu della tray
- [x] Regole: disco USB, rete connessa (Wi-Fi o cavo)
- [x] Memoria del pannello nascosto (`MemoryUsageTargetLevel`)
- [ ] (forse, rimandate) Statistiche locali: quante ore sveglio, e perché. Non servono a nessuna decisione dell'utente oggi; la diagnostica copre già il "perché"
- [ ] Le righe 0.0.5 aperte di `test.md`: "Mostra chi" con il prompt vero, voce del menu, disco USB vero, rete Wi-Fi

### 1.0.0

- [ ] Pubblicazione su winget
- [ ] Binari firmati
- [ ] Regole stabili, nessun bug grave aperto

---

## Architettura

Com'è oggi (0.0.1); fra parentesi ciò che arriva dopo.

```text
src-tauri/src/
  main.rs          avvio
  lib.rs           builder Tauri, plugin, tray, menu, timer
  state.rs         stato dell'app dietro un Mutex; ogni modifica passa da apply()
                   (richiesta di alimentazione + state.json, mai uno senza l'altro)
  control.rs       le azioni, condivise da pannello, menu, clic e riga di comando;
                   tray e menu si ridisegnano solo sul thread principale
  commands.rs      i comandi invoke delle pagine
  popover.rs       posizione accanto all'icona, apertura/chiusura, altezza
  tray.rs          icone (3 stati × 2 barre × 6 dimensioni) e menu
  power.rs         PowerCreateRequest / PowerSetRequest / PowerClearRequest
  session.rs       sessione e durate: logica pura, orologio iniettato (testabile)
  settings.rs      settings.json e state.json, letti come ostili, scritti in modo atomico
  i18n.rs          testi da src/locales/*.json, inclusi in compilazione
  cli.rs           parsing degli argomenti (anche quelli inoltrati dal single-instance)
  sys.rs           orologi, sessione di accesso, DPI, tema della barra, schermo spento
  capabilities.rs  portatile? standby moderno? ibernazione? stato di esecuzione
  lid.rs           azione del coperchio: leggere, scrivere, permessi
  lidoverride.rs   la modifica che non resta mai cambiata: registro, RunOnce,
                   ripristino (Windows dietro un trait: nei test è finto)
  lidplan.rs       la logica del coperchio, pura: cosa forzare, cosa avrebbe
                   fatto Windows, scrivania, zaino, blocco alla riapertura
  sysevents.rs     finestra nascosta con le notifiche di sistema
  actions.rs       blocco, sospensione, ibernazione, spegnimento
  (triggers/       una regola per file, trait comune — 0.4)
  (presence.rs     F15 via SendInput — 0.4, o 0.2 se lo spike lo chiede)
src-tauri/examples/
  spike.rs         prova tecnica sullo standby moderno (docs/SPIKE.md)
src/
  index.html, popover.js     il pannello
  settings.html, settings.js le Impostazioni
  i18n.js, locales/          traduzioni (una sola fonte con Rust)
  styles.css                 token chiaro/scuro, due token per l'accento
scripts/
  icons.mjs        genera tutte le icone dagli SVG scritti a mano
  check-js.mjs     node --check su ogni file JS
  check-versions.mjs  stessa versione in package.json, Cargo.toml, tauri.conf.json, Cargo.lock
tests/
  i18n.test.mjs    chiavi uguali nelle lingue, ogni chiave usata esiste ed è usata
```

**Dipendenze previste**:

- `tauri` (feature `tray-icon`, `image-png`)
- plugin Tauri: oggi `single-instance`, `autostart`, `opener`; poi `global-shortcut`, `notification`, `updater`, `process`
- `windows` 0.61 (la stessa versione che usa Tauri, così non si compila due volte), attivando **solo** le feature usate. Poi arriveranno `Win32_UI_Input_KeyboardAndMouse`, `Win32_UI_Shell`, `Win32_System_Shutdown`, `Win32_Devices_Display` (per `QueryDisplayConfig`), …
- `serde`, `serde_json`, `chrono` (solo `clock`, per "fino alle HH:MM" nel fuso locale e l'ora legale)

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
17. Wi-Fi: da Windows 11 24H2 leggere il nome della rete (SSID) richiede il permesso di posizione. **Risolta** (0.0.5): il Network List Manager dà il nome della rete come lo mostra Windows senza nessun permesso, e vale anche per il cavo. Verificato su LPT-MIKI (Windows 11 26200) sulla rete via cavo; col Wi-Fi da riprovare.
18. `powercfg /requests` richiede l'amministratore, e l'output potrebbe essere localizzato. Le categorie (`SYSTEM:`) e i tipi (`[PROCESS]`) non lo sono: il parser (0.0.5) guarda solo quelli. Da provare con il prompt vero su Windows in italiano. Il resto della diagnostica evita `powercfg` e legge le API.
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

### Trovate scrivendo la 0.0.1 (2026-09-22)

31. Su alcuni portatili l'azione del coperchio è **nascosta** (`ATTRIB_HIDE`, è il caso di LPT-MIKI): `powercfg /q` non la mostra affatto, e sembra che l'impostazione non esista. Serve `powercfg /qh`. Le API la leggono e la scrivono normalmente.
32. Nel crate `windows` 0.61 `PowerReadDCValueIndex`/`PowerWriteDCValueIndex` restituiscono un `u32` grezzo, mentre le versioni AC restituiscono `WIN32_ERROR`: si avvolgono in `WIN32_ERROR(...)`.
33. Windows 11 mette le icone nuove fra quelle **nascoste** (^). Un'app che vive solo nella tray, al primo avvio, per l'utente semplicemente non c'è: da qui il benvenuto.
34. Avvio rapido: "Arresta il sistema" non riavvia il kernel, quindi `GetTickCount64` continua a contare e un confronto sull'uptime scambia l'accensione successiva per lo stesso avvio. Per "riavvio dell'app sì, riavvio del PC no" serve anche la sessione di accesso (LUID dal token).
35. A 16 px il disegno vettoriale dell'icona non regge: va disegnata pixel per pixel. Da 20 px in su il vettoriale va bene.
36. Con `crate-type` `staticlib`/`cdylib` (quelli del template di Tauri, pensati per mobile) il linker MSVC stampa "Creazione della libreria …" e Rust recente lo segnala come avviso (`linker_messages`). Moka è solo per Windows: basta `rlib`.
37. Dopo un `taskkill /F` il processo sparisce e con lui le richieste (verificato), ma sparisce anche il `tauri dev` che lo aveva lanciato: per riprovare la ripresa della sessione va rilanciato.

### Trovate scrivendo la 0.0.3 (2026-09-22)

41. Il template NSIS di Tauri ha degli **agganci** (`installerHooks`: prima e dopo installazione e disinstallazione) che permettono di aggiungere logica senza copiare il template. Una pagina nuova ("Attività aggiuntive") invece richiede il template: Moka usa entrambi, e il template differisce dall'originale solo in tre punti marcati "Moka:".
42. Alla disinstallazione l'ordine conta: se l'installer chiude Moka a forza con la modifica del coperchio attiva, `RunOnce` punterebbe a un eseguibile appena cancellato e l'impostazione resterebbe cambiata per sempre. Prima `moka --quit` (uscita pulita), poi `--restore-lid`, e solo dopo i file.
43. `createUpdaterArtifacts: true` nella configurazione normale fa fallire ogni `tauri build` senza la chiave privata (anche in locale e nel workflow `build`). Sta in `tauri.release.conf.json`, usato solo dalla release.
44. La classe `.toast` esisteva già (il messaggio "Salvato" delle Impostazioni, fisso e trasparente): la finestrella degli avvisi la ereditava e restava invisibile. Lezione generale: un foglio di stile condiviso fra più pagine vuole nomi di classe che non si pestino.
45. `focusable: false` nella configurazione della finestra la mostra senza rubare il focus a chi sta scrivendo: giusto per un avviso che compare da solo.

### Trovate scrivendo la 0.0.5 (2026-09-22)

53. In Git Bash `powercfg /lastwake` risponde "Parametri non validi": MSYS riscrive `/lastwake` come un percorso. Serve `MSYS_NO_PATHCONV=1` (come per `adb` nel mega-repo).
54. Sui PC con standby moderno `powercfg /lastwake` è **vuoto** ("Conteggio cronologia riattivazioni - 0"): i risvegli stanno solo negli eventi `Kernel-Power` 507, con il motivo come codice.
55. `Kernel-Power` 506 e 507 non si accoppiano con `ScenarioInstanceId` (sull'uscita è un altro numero) ma con `ScenarioInstanceIdV2`, uguale sui due eventi.
56. `wevtutil` e `EvtRender` scrivono gli attributi XML fra apici singoli, gli esempi della documentazione fra doppi: il parser accetta entrambi.
57. Un disco esterno USB per `GetDriveType` è "fisso" come quello interno: per riconoscerlo serve il bus del volume.
58. Con `A && B; C` in Bash, se A fallisce B non parte e C sì: un `moka --quit` saltato così sembra un `--quit` che non funziona.
59. Uno script Python dentro un heredoc Bash trasforma `\\0` in un carattere NUL vero dentro il sorgente Rust. Per le stringhe con barre rovesciate: `r"..."` in Rust e il file scritto direttamente, non passato da una shell.
60. Un'altezza massima fissa per una finestra che si adatta al contenuto (erano 720 px) più `overflow: hidden` sulla pagina è un fondo che sparisce: con due schede aperte insieme il pannello arrivava a 928 px. Il tetto giusto è l'area di lavoro del monitor, e oltre la pagina deve scorrere. Si vedeva già in una schermata della 0.0.4 (il pannello finiva a "Cosa tenere acceso"), ma nessuno ha guardato il fondo: guardare **tutta** la schermata, non solo la parte nuova.
61. Cambiare i dati in memoria e **poi** salvarli: se il salvataggio non riesce (cartella non scrivibile, disco pieno) l'app mostra una cosa e il disco ne contiene un'altra, e al riavvio l'utente ritrova qualcosa che aveva "cambiato". Si cambia una copia e la si mette solo a salvataggio riuscito (`Core::set_settings`). La sessione invece deve continuare anche se non si può scrivere: quella sta in memoria per scelta.
62. Un file reso di sola lettura non impedisce a un salvataggio atomico di sostituirlo: su Windows il permesso di cancellazione può arrivare dalla **cartella**. Per provare davvero un salvataggio che fallisce va negata la scrittura sulla cartella, non sul file.
63. Il **primo** webview di un processo paga l'avvio a freddo del runtime WebView2 (3,2 s su LPT-MIKI); i successivi costano 0,2 s. Quindi una finestra che deve comparire all'istante non può essere la prima: il pannello di Moka si crea all'avvio anche per questo, e tiene il runtime caldo per la finestrella degli avvisi.
64. Una chiave di registro condivisa non ha padrone: il valore `RunOnce` che rimette l'azione del coperchio si chiamava `MokaRestoreLid` per **tutte** le Moka, e `persist()` lo cancella ogni volta che l'elenco delle modifiche si svuota — cioè a ogni chiusura, anche di una Moka che non l'aveva scritto. Una build di sviluppo accanto a quella vera toglieva così la rete di sicurezza alla sessione vera, in silenzio. Il nome ora contiene un'impronta dell'eseguibile. Regola: se due copie dello stesso programma possono girare insieme (e in sviluppo succede sempre), ogni cosa che scrivono **fuori** dalla loro cartella dati vuole un nome che dica di chi è.

### Trovate scrivendo la 0.0.4 (2026-09-22)

46. `GetIfTable2` elenca la **stessa** scheda di rete più volte (le interfacce dei filtri: QoS, WFP, …), ognuna con i suoi contatori. Sommarle conta lo stesso download due o tre volte: per la velocità vale la più veloce.
47. Un **desktop remoto** aperto (RustDesk, TeamViewer) fa entrare circa 1 MB/s in continuazione: una regola "download sopra 1 MB/s" resta vera per tutta la sessione remota. Non si distingue da un download vero; va detto a chi sceglie la soglia.
48. Per lo stesso motivo il bit `DISPLAY` dello stato di esecuzione del sistema può essere già acceso da altri (il desktop remoto tiene lo schermo): con una sessione remota aperta non prova che la richiesta è di Moka.
49. Per provare una Moka nuova mentre un'altra sta già tenendo sveglio il PC (magari proprio quello da cui si lavora), serve un identifier diverso: `npx tauri dev --config '{"identifier":"com.moka.dev"}'`. Single-instance, dati e WebView2 restano separati; dopo, via `%APPDATA%\com.moka.dev` e `%LOCALAPPDATA%\com.moka.dev`. Con il coperchio mai chiesto nell'istanza nuova, l'impostazione di Windows non viene toccata.
50. Nel crate `windows` 0.61 `BOOL` sta in `windows::core`, non più in `Win32::Foundation`.
51. `Add-Type` di PowerShell fallisce se la variabile `LIB` contiene una cartella che non esiste (su LPT-MIKI la lascia un SDK Quixant): `env -u LIB powershell …`.
52. Un'etichetta visibile per ogni tendina, anche quando lo spazio è poco: "Niente" da solo, accanto a "Solo il PC", non dice che è il "…e poi".

### Trovate scrivendo la 0.0.2 (2026-09-22)

38. Il binario di debug lanciato da solo (`target/debug/moka.exe`) cerca le pagine sul server di sviluppo (`127.0.0.1:1430`): se `tauri dev` non gira, le finestre restano vuote. Va bene per provare la parte Rust (sessioni, coperchio, riga di comando), non l'interfaccia.
39. Durante `tauri dev`, modificare **qualunque** file sotto `src-tauri/` (anche un esempio) ricompila e riavvia l'app: le finestre aperte spariscono. Chi prova l'interfaccia non tocca il codice nel frattempo.
40. Con due monitor esterni e il pannello interno spento, un portatile sembra un fisso a chi guarda solo gli schermi. Il coperchio va letto dalle notifiche (`GUID_LIDSWITCH_STATE_CHANGE`), non dedotto dai monitor. E durante le prove va ricordato che una sessione spenta **a coperchio chiuso** fa sospendere davvero il PC dopo 10 secondi.

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
- Senza amministratore, `spike info` legge lo stato di esecuzione del sistema: con una sessione attiva la prima colonna ("prima") vale `0x1` (SYSTEM) o `0x3` (SYSTEM+DISPLAY), a sessione finita `0x0`. Dice **che** qualcuno tiene sveglio il PC, non **chi**: per il nome del processo e il motivo serve `powercfg /requests`.
- Chiudendo Moka a forza da Task Manager, la richiesta sparisce da `powercfg /requests` (e lo stato di esecuzione torna a `0x0`).
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

- La chiave privata dell'updater: generata il 2026-09-22 su LPT-MIKI in `%USERPROFILE%\.tauri\moka-updater.key` (senza password). Va nel secret su GitHub e in un backup esterno, **mai nel repo**. Su un altro PC non c'è: le build locali con firma si fanno solo da LPT-MIKI (le build normali non la chiedono).
- Le note di sviluppo locali sono escluse dal repo di proposito (vedi `.gitignore`): tutto ciò che serve per riprendere deve stare **in questo file**.
- Il lavoro non pushato. Si pusha a ogni passaggio, non solo a fine giornata.

## Fuori da questo repo (alla prima release)

- DNS e hosting di `moka.mtsolutions.studio`, come per gli altri progetti.
- Voce nella sezione "Progetti open source" della home di mtsolutions.studio, con l'icona.
- Anteprima locale del sito: la porta si prende dal registro del mega-repo (`MTSolutions/docs/ports.json`, range dei siti 4400–4499) quando il sito esiste. Il piano iniziale diceva 4712, dopo la 4710 e la 4711 dei siti di MD-Viewer e ClipVault, ma quelle stanno fuori dal registro.

## Convenzioni del repo

- Branch `main`. Dopo la pubblicazione: branch `feature/<slug>` e PR.
- Dal 2026-09-22 Moka è un **submodule del mega-repo MTSolutions** (`MTSolutions/Moka`), pur restando un repo personale (`TarducciM/Moka`, niente prefisso `app-`, niente organizzazione). Il mega-repo tiene un commit preciso, non un ramo: dopo un push qui, il puntatore lassù si aggiorna a parte, con una PR sul mega-repo.
- Commit in italiano, descrittivi, senza trailer di co-autore.
- `CHANGELOG.md` datato a ogni passaggio significativo, versione bumpata insieme.
- README bilingue IT/EN, come ClipVault e MD-Viewer.
