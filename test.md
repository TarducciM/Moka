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
