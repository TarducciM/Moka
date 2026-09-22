# Verifiche a mano

"Compila" non vuol dire "funziona". Ogni riga ha data, macchina (`hostname`) ed esito. Chi riprende il lavoro spunta le righe aperte e aggiunge quelle nuove.

## Già verificato

2026-09-22, **LPT-MIKI**, Moka 0.0.1 in `tauri dev`. Lo stato di esecuzione del sistema (`SYSTEM`, `DISPLAY`) è letto con `spike info`, che lo legge **senza amministratore** (vedi la roadmap, "Verifiche su LPT-MIKI").

- [x] `moka --for 1m --screen` inoltrato all'istanza già aperta: sessione "PC e schermo", richiesta `SYSTEM+DISPLAY` visibile nel sistema, stato salvato in `state.json`
- [x] Alla scadenza la sessione si chiude da sola: richiesta rilasciata (`0x0`), sessione tolta da `state.json`
- [x] Moka chiusa a forza (`taskkill /F`) con una sessione attiva: la richiesta sparisce subito (`0x1` → `0x0`)
- [x] Riaperta subito dopo: la sessione riprende con il tempo giusto ("ancora 30 min") e la richiesta torna attiva
- [x] `moka --quit`: l'app si chiude, nessuna richiesta resta, la sessione non verrà ripresa
- [x] Impostazioni: si aprono solo quando servono (la finestra non esiste finché non si apre), titolo tradotto
- [x] Durate rapide: "15m, boh" → errore tradotto accanto al campo; "45m, 1h30m ,10" → salvato come "10m, 45m, 1h30m" e subito nel pannello
- [x] Lingua su English: Impostazioni, titolo della finestra e pannello passano all'inglese; di nuovo su "Come Windows" → italiano
- [x] Contrasto misurato sui colori calcolati, pannello e Impostazioni, tema chiaro e scuro: nessun testo sotto soglia (minimo 5,45:1 in chiaro, 5,84:1 in scuro)
- [x] Icona della tray a 16, 20, 24, 32 px, barra chiara e scura: le tre forme si distinguono (a 16 px è disegnata pixel per pixel)

2026-09-22, **LPT-MIKI**, Moka 0.0.2. L'impostazione del coperchio è quella **vera** di Windows, letta ogni volta con `spike info`; `RunOnce` con `reg query`; il registro è `%APPDATA%\com.moka.app\lid-override.json`. Prima delle prove: "Sospendi" in carica e a batteria, nessuna traccia di Moka. **Dopo tutte le prove: di nuovo "Sospendi" e "Sospendi", nessuna traccia.**

- [x] Primo avvio su portatile: la domanda compare; "Conferma" con la scelta consigliata la chiude e fa comparire la riga "Anche a coperchio chiuso · Solo quando è in carica"
- [x] Sessione con il coperchio, "solo in carica": Windows passa a "non fare nulla" **solo in carica**, a batteria resta "sospendi"; registro con l'originale scritto prima; `RunOnce` presente con `"…\moka.exe" --restore-lid`
- [x] Fine della sessione: di nuovo "sospendi", registro e `RunOnce` spariti
- [x] Moka chiusa a forza con la modifica attiva: l'impostazione resta cambiata (il processo è morto) ma `RunOnce` c'è; `moka --restore-lid` (ciò che Windows esegue al prossimo accesso) la rimette, cancella registro e `RunOnce`, ed esce senza avviare l'app
- [x] Moka chiusa a forza e riaperta: all'avvio rimette tutto, poi la sessione ripresa riapplica la modifica, e nel registro l'originale è ancora "sospendi" (non "non fare nulla")
- [x] Durante la sessione l'utente mette a mano "iberna" in carica: a fine sessione Moka **non** la sovrascrive, e pulisce registro e `RunOnce`
- [x] "Ripristina ora" nelle Impostazioni: Windows torna com'era subito, la sessione continua (PC ancora sveglio) senza il coperchio
- [x] Da "solo in carica" ad "anche a batteria" durante una sessione: entrambi su "non fare nulla"; di nuovo "solo in carica": torna "sospendi" solo quello a batteria
- [x] Modalità scrivania con due monitor esterni, senza sessione: entrambi i valori tenuti; tolta: tutto com'era
- [x] Uscita (`--quit`) con una sessione e la modifica attive: tutto com'era, nessuna traccia
- [x] Contrasti delle parti nuove (domanda, riga, sezioni Coperchio e Batteria), chiaro e scuro: minimo 5,45:1 e 5,84:1

2026-09-22, **LPT-MIKI**, Moka 0.0.3.

- [x] "…e poi: spegni lo schermo" su una sessione di 1 minuto: alla scadenza compare la finestrella "Schermo spento tra 58 s", **senza** rubare il focus; durante l'attesa il PC resta sveglio (`SYSTEM` attiva)
- [x] "Annulla": la finestrella si chiude, la richiesta si rilascia, il "…e poi" torna "niente"
- [x] "+30 min" dal conto alla rovescia: riaccende per 30 minuti mantenendo il "…e poi"
- [x] Avviso dei 5 minuti su una sessione di 11: compare puntuale dopo 6 minuti, in basso a destra (e "+30 min" lì funziona: la sessione è passata a 35 minuti)
- [x] Tasto rapido Ctrl+Maiusc+F9 scelto nelle Impostazioni (etichette "Maiusc" in italiano), registrato senza errori; premuto (simulato con `SendKeys`) spegne, premuto di nuovo riaccende con l'ultima scelta
- [x] Controllo aggiornamenti con il repo ancora privato: "Controllo non riuscito", nessun pulsante di installazione, niente di rotto (il `latest.json` risponde 404, come atteso)
- [x] Sito (`site/`, servito in locale): italiano e inglese, pagine legali e link interni, nessuno scorrimento orizzontale a 375 px, contrasti sopra soglia in chiaro e scuro (minimo 5,84:1)
- [x] Build di release firmata in locale, con la chiave dell'updater al posto del secret: NSIS 2,0 MB e MSI 2,8 MB, ognuno con la sua `.sig`
- [x] Installer NSIS silenzioso (`/S`): installa in `%LOCALAPPDATA%\Moka`, voce in "App e funzionalità" con la versione giusta
- [x] **Reinstallazione sopra una Moka aperta con la modifica del coperchio attiva**: l'installer la chiude in modo pulito, l'impostazione torna "sospendi", niente `RunOnce`, niente registro, nessun processo rimasto
- [x] **Disinstallazione con la modifica attiva**: stessa cosa, e in più via la cartella, la voce di disinstallazione, l'avvio automatico e i collegamenti

2026-09-22, **LPT-MIKI**, Moka 0.0.4 in `tauri dev` con identifier `com.moka.dev` (un'altra Moka stava tenendo sveglio il PC: trappola 49). Stato letto con `invoke('get_state')` via CDP; sonde con `spike probes`.

- [x] Regola "Programma aperto: notepad" (salvata come `notepad.exe`): aperto Blocco note, entro 5 s "Acceso · notepad.exe è aperto"; chiuso, entro 5 s "Spento"
- [x] Regola duplicata, fascia oraria senza giorni, soglia del processore fuori elenco: rifiutate con messaggi tradotti, niente salvato
- [x] "…e poi: spegni lo schermo" su una regola: alla chiusura del programma parte la finestrella "Schermo spento tra 59 s — La regola «Programma aperto · notepad.exe» non tiene più acceso il PC."; Annulla la chiude
- [x] Spegnere dal pannello con una regola attiva: compare la domanda (focus su "Sospendi per un'ora"); sospese: "Regole sospese fino alle 14:54", e a Blocco note ancora aperto il PC resta spento; "Riprendi": di nuovo acceso subito, senza aspettare i 5 s
- [x] `moka --while-pid <pid> --screen` inoltrato all'istanza aperta: "Acceso · il processo … è attivo", anche lo schermo; chiuso il processo: "Spento"
- [x] `moka --pause-rules=2h` → "Regole sospese fino alle 15:55"; `moka --resume-rules` → tolte
- [x] Fascia oraria che comprende adesso: accesa subito ("è fra le 13:25 e le 14:25"); "In carica" con l'alimentatore: accesa; processore sotto carico (16 processi al 100% per 15 s): compare "il processore è occupato"
- [x] Regola "PC e schermo" attiva: il vapore sulla moka nel pannello
- [x] Impostazioni: elenco con "Adesso" sulle regole che valgono, interruttore, "Tieni acceso", "Quando finisce", elimina; modulo con suggerimenti (14 programmi aperti), giorni feriali di default, errori tolti appena si corregge, Esc chiude prima il modulo
- [x] Presenza attiva: l'inattività di Windows sale fino a 50 s e al controllo dopo torna a 0 da sola (F15)
- [x] Contrasti delle parti nuove, pannello e Impostazioni, chiaro e scuro: nessun testo sotto soglia (minimo 4,95:1, il badge "Adesso")

2026-09-22, **LPT-MIKI**, Moka 0.0.5 in `tauri dev` con identifier `com.moka.dev` (trappola 49). Dati letti via CDP e con `spike diagnose`/`spike probes`.

- [x] Diagnostica dalla scheda delle Impostazioni (e con l'evento che usa il menu della tray): in ~2 s "Adesso", ultimi standby e dispositivi. Sulla macchina vera ha detto "in carica non lo sospende mai" (vero: `powercfg` dà 0) e "negli ultimi standby è rimasto attivo per un audio aperto" (vero: 0% a basso consumo e `AudioPlaying` in tutti gli ultimi 507)
- [x] Ultimi standby: "Oggi alle 13:08, dopo 42 min di standby · si è svegliato: hai mosso il mouse · era entrato in standby per inattività", coerente con il registro di Windows
- [x] "Cosa può svegliarlo": gli stessi 4 dispositivi di `powercfg /devicequery wake_armed`; timer di risveglio disattivati, come nello schema
- [x] Aperta dal menu, la scheda va in cima alla finestra dopo i risultati e il focus resta sul pulsante
- [x] Regola "Rete connessa": il modulo suggerisce "WiFi-ABCOM 4" (la rete su cavo), la regola accende subito ("Acceso · sei connesso a «WiFi-ABCOM 4»")
- [x] Regola "Disco USB collegato" senza dischi USB: resta spenta; il bus dei volumi si legge (`C:` NVMe, 17)
- [x] Memoria: a pannello nascosto 95 MB di working set per i processi WebView2 (254 aperto), contro 138 MB della 0.0.3 nascosta da ore; memoria privata uguale (~103 MB)
- [x] Contrasti della scheda Diagnostica (anche con una riga d'esempio di "chi lo tiene sveglio" e l'errore), chiaro e scuro: nessun testo sotto soglia; il bordo "probabile causa" ≥ 5,8:1

2026-09-22, **LPT-MIKI**, Moka 0.0.6.

- [x] Pannello con benvenuto e domanda sul coperchio (prima avvio): alto 928 px, tutto visibile fino a "Spegni lo schermo ora" (con la 0.0.5 si fermava a 720 e il fondo era tagliato)
- [x] Contenuto più alto dello schermo: la finestra si ferma all'area di lavoro (1368 px su 1392) e la pagina scorre

2026-09-22, **LPT-MIKI**, Moka 0.0.7: prove con dati ostili e scritture impedite.

- [x] `settings.json` e `state.json` ostili (lingua `42`, durate `"15"`/`-3`/`999999999`, `lidMode: "esplodi"`, regole con nomi da 84 caratteri, giorni `255`, modalità `teletrasporto`, una stringa al posto di una regola, sessione da 9.999.999 minuti con `logonId` testuale): Moka parte, tiene solo il valido (6 durate, 1 regola su 6), scarta la sessione impossibile
- [x] Cartella dei dati non scrivibile: "Non è stato possibile salvare: Accesso negato. (os error 5)" tradotto; soglia batteria e regole **invariate** (prima restavano cambiate in memoria); la sessione in corso continua
- [x] `settings.json` e `state.json` cancellati mentre Moka gira: al primo salvataggio li ricrea
- [x] Tre `start_session` e quattro `toggle` simultanei: una sola sessione, poi spento; due `add_rule` identiche insieme: una salvata, una "Questa regola c'è già"
- [x] 22 regole: si ferma a 20 con "Hai raggiunto il numero massimo di regole"; con 20 nomi lunghissimi l'elenco resta leggibile e non compare scorrimento orizzontale

## Da verificare

Servono le mani sul PC (clic sull'icona, menu nativo, prompt amministratore) oppure un altro PC.

- [ ] L'icona è nelle icone nascoste (^) al primo avvio: trascinarla sulla barra. Clic sinistro → il pannello si apre **sopra l'icona**, dentro lo schermo; clic di nuovo sull'icona → si chiude (non si riapre subito)
- [ ] Clic fuori dal pannello o Esc → si nasconde
- [ ] Clic destro → menu completo: riga di stato, Accendi/Spegni, "Accendi per" con le durate, "Anche lo schermo" con la spunta giusta, Spegni lo schermo ora, Apri Moka, Impostazioni…, Esci
- [ ] Tooltip dell'icona: "Moka · Solo il PC · ancora 1 h 12 min", e il minuto scende da solo
- [ ] Icona che cambia da contorno (spenta) a piena (acceso) a piena con vapore (anche lo schermo)
- [ ] Barra delle applicazioni da scura a chiara (Impostazioni di Windows → Personalizzazione → Colori): l'icona cambia variante **senza riavviare Moka**
- [ ] Impostazioni → "Clic sull'icona: accende o spegne": il clic sinistro accende/spegne con l'ultima scelta
- [ ] Impostazioni → avvio automatico: dopo un riavvio del PC Moka è nella tray, senza pannello aperto, e la sessione di prima **non** è ripresa
- [ ] `powercfg /requests` (prompt amministratore) durante una sessione: `moka.exe` in `SYSTEM` (e in `DISPLAY` con lo schermo), con il motivo leggibile ("Moka: sveglio per 2 h, fino alle 16:12"). A sessione finita non compare più
- [ ] "PC e schermo" con lo spegnimento dello schermo impostato a 1 minuto (da ripristinare dopo): lo schermo non si spegne
- [ ] "Spegni lo schermo ora" su un PC **senza** standby moderno: lo schermo si spegne, il PC resta sveglio. Sui portatili con standby moderno vale lo spike (`docs/SPIKE.md`)
- [ ] Riavvio del PC con una sessione attiva: al nuovo accesso la sessione **non** riparte
- [ ] Scala 100%, 125%, 150%: icona nitida, pannello posizionato bene
- [ ] Installer NSIS e MSI (workflow `build`): installazione, avvio, disinstallazione
- [ ] Installer NSIS interattivo: la pagina "Attività aggiuntive" in italiano, la casella dell'avvio automatico funziona
- [ ] MSI: disinstallazione con la modifica del coperchio attiva (serve l'amministratore)
- [ ] "…e poi" eseguito davvero: blocca, sospendi, iberna, arresta (non provati: avrebbero fermato il PC di lavoro)
- [ ] Aggiornamento vero da una versione pubblicata alla successiva (serve il repo pubblico): mai durante una sessione, l'impostazione del coperchio rimessa prima
- [ ] Promemoria stella dopo 5 avvii e 3 giorni

### Pannello (0.0.6)

- [ ] Su uno schermo basso (1366×768, o scala al 150% su un 1080p): con benvenuto e domanda sul coperchio il pannello scorre con la rotella e con il touchpad, e il piede si raggiunge

### Diagnostica, USB e rete (0.0.5)

- [ ] "Mostra chi lo tiene sveglio": compare il prompt dell'amministratore; con "Sì", l'elenco (e `moka.exe` segnato "Moka (questa app)" se è accesa); con "No", il messaggio "senza il permesso…". Su Windows in italiano e in inglese (trappola 18)
- [ ] Voce "Perché non dorme?…" nel menu della tray: apre le Impostazioni già sulla diagnostica e fa il controllo; con le Impostazioni già aperte, ci va lo stesso
- [ ] Regola "Disco USB collegato" con una chiavetta e con un disco esterno: accesa entro 5 s dall'inserimento, spenta alla rimozione
- [ ] Regola "Rete connessa" su Wi-Fi: accende sulla rete giusta, resta accesa 30 s se il Wi-Fi cade, si spegne cambiando rete
- [ ] Diagnostica su un PC con sospensione classica (S3): i risvegli da `Power-Troubleshooter` 1 con il dispositivo o il timer

### Regole e Presenza (0.0.4)

- [ ] Menu della tray con regole salvate: c'è "Sospendi le regole per un'ora"; dopo il clic diventa "Riprendi le regole"
- [ ] Clic sull'icona (con "accende o spegne") o tasto rapido mentre vale solo una regola: le regole si sospendono per un'ora
- [ ] Schermo intero: un video a schermo intero nel browser, una presentazione di PowerPoint, un gioco
- [ ] In chiamata: Teams, Zoom, Meet nel browser (microfono e webcam, entrambi); la chiamata finita libera il PC dopo ~30 s
- [ ] Monitor esterno: collegato → acceso, scollegato → spento
- [ ] Download vero (un file grande) con "…e poi: sospendi": a download finito, dopo 2 minuti, parte il conto alla rovescia
- [ ] `moka --while programma-che-non-esiste.exe`: dopo ~10 s la notifica "Niente da seguire"
- [ ] Soglia batteria con una regola attiva: sotto soglia la regola smette di tenere acceso, una sola notifica
- [ ] Regola attiva e coperchio chiuso in carica (con "Anche a coperchio chiuso"): resta sveglio come una sessione
- [ ] Presenza: F15 non fa niente di visibile in Word, Excel, Chrome, Teams, giochi; Teams resta "Disponibile" oltre il suo tempo di inattività
- [ ] `powercfg /requests` (amministratore) con una regola attiva: `moka.exe` con il motivo "Moka: sveglio perché notepad.exe è aperto"

### Coperchio e batteria (0.0.2) — servono le mani sul portatile

Tutto ciò che chiede di **chiudere il coperchio** o di **staccare l'alimentatore**. Da fare dopo lo spike, che dice se a coperchio chiuso il PC resta davvero sveglio.

- [ ] Sessione "solo in carica", coperchio chiuso 5 minuti: resta sveglio (log dello spike o download che avanza); riaperto: schermata di blocco
- [ ] Stessa sessione, la si spegne dal telefono o con `moka --off` da remoto a coperchio chiuso: dopo ~10 s il PC si sospende
- [ ] Sessione "solo in carica", coperchio chiuso, si stacca l'alimentatore: dopo ~10 s il PC si sospende
- [ ] Coperchio riaperto entro i 10 s: niente sospensione, schermata di blocco
- [ ] "Anche a batteria", a batteria, coperchio chiuso, nessun monitor: dopo il tempo della protezione zaino (provarla a 10 min) il PC si sospende
- [ ] Modalità scrivania, coperchio chiuso con i monitor: resta acceso; si scollega l'ultimo monitor: si sospende
- [ ] Soglia batteria (provarla al 50%), a batteria con una sessione: sotto soglia la sessione finisce e compare la notifica
- [ ] Una sessione avviata con la batteria già sotto soglia non si interrompe
- [ ] Disconnessione o arresto di Windows con la modifica attiva: al nuovo accesso l'impostazione è com'era (`WM_ENDSESSION`, e in subordine `RunOnce`)
- [ ] Account standard (non amministratore): la modifica riesce, o la sezione spiega perché no
- [ ] Sospendi / iberna / arresta "come Windows" su un PC con standby moderno: `SetSuspendState` funziona o scatta il ripiego (ipotesi 4 dello spike)
