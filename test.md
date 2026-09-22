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
