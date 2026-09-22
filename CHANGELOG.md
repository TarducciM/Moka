# Changelog

## 2026-09-22 — 0.0.7: quello che si vede e quello che c'è su disco non divergono più

Primo giro di prove con **dati ostili e scritture impedite**, mai fatto su Moka (è la stessa passata che nelle app MTSolutions ha trovato bug veri).

- **Correzione**: se il salvataggio non riesce (cartella non scrivibile per un antivirus o un criterio, disco pieno) le impostazioni e le regole restavano cambiate **in memoria** pur non essendo su disco: l'app mostrava una cosa e al riavvio ne ricompariva un'altra. Ora si cambia una copia e la si mette solo a salvataggio riuscito; l'errore resta quello tradotto di prima.
- Verificato su LPT-MIKI, con la cartella dei dati resa non scrivibile: messaggio tradotto, soglia batteria e regole **invariate**, e la sessione in corso continua (il PC resta sveglio anche se il disco non si può scrivere).
- Verificato anche: file `settings.json` e `state.json` pieni di valori ostili (tipi sbagliati, durate impossibili, regole non valide, una sessione da 9.999.999 minuti) → Moka parte, scarta ciò che non può esistere e tiene il resto; file cancellati mentre gira → li ricrea; tre accensioni e quattro interruttori simultanei → una sola sessione, nessun doppione; due regole identiche insieme → una salvata e una rifiutata; 22 regole → si ferma a 20 con il messaggio giusto; 20 regole dai nomi lunghissimi → elenco leggibile, nessuno scorrimento orizzontale.
- Il registro delle modifiche al coperchio (l'unico dato su disco che Moka riscrive **dentro Windows**) era già letto come ostile: GUID valido, valore originale fra 0 e 3, e solo "non fare nulla" come valore scritto.

## 2026-09-22 — 0.0.6: il pannello si vede tutto, e scorre

- **Correzione** (segnalata da Michele: "a me non scorre"): il pannello aveva un tetto fisso di 720 px e la pagina `overflow: hidden`. Con il benvenuto e la domanda sul coperchio aperti insieme il contenuto arrivava a 928 px: durate, "…e poi" e il piede restavano tagliati, e non c'era modo di vederli.
- Ora il pannello è alto quanto il contenuto fino all'area di lavoro dello schermo, e oltre scorre (barra sottile, senza trascinare la pagina sotto).
- Verificato su LPT-MIKI: 928 px, piede visibile; con contenuto più alto dello schermo la finestra si ferma a 1368 px su 1392 e la pagina scorre. Il sito scorreva già: controllato con la rotella nel browser.

## 2026-09-22 — 0.0.5: diagnostica, regole su USB e rete, meno memoria

- **"Perché non dorme? Perché si è svegliato?"**: una scheda nelle Impostazioni e la voce "Perché non dorme?…" nel menu della tray.
  - Senza amministratore legge il registro eventi (standby moderno, sospensione, ibernazione), i dispositivi che possono svegliare il PC e le impostazioni di sospensione, e li spiega a parole. I motivi sono codici di Windows tradotti da Moka, quindi non dipendono dalla lingua del sistema.
  - Mette in cima le cause più comuni: "sospensione: mai" e l'audio aperto che tiene attivo lo standby.
  - "Mostra chi lo tiene sveglio" esegue `powercfg /requests` e `/waketimers` con il prompt dell'amministratore, solo se lo chiedi.
- **Regole nuove**: disco USB collegato (dal bus del volume, anche per i dischi esterni che Windows chiama "fissi") e rete connessa (il nome come lo mostra Windows, Wi-Fi o cavo, senza il permesso di posizione).
- **Meno memoria**: a pannello nascosto WebView2 tiene meno RAM (95 MB di working set invece di 138 su LPT-MIKI).
- La scheda "Verifica" è sostituita dalla diagnostica. Sito e privacy aggiornati.
- `spike diagnose`: ciò che la diagnostica legge, per confrontarlo con `powercfg`.
- Verificato su LPT-MIKI: diagnostica sui dati veri della macchina, regola di rete, bus dei volumi, memoria, contrasti (dettagli in `test.md`). Da provare con le mani: il prompt dell'amministratore, la voce del menu, un disco USB vero, il Wi-Fi.

## 2026-09-22 — sito: header e footer allineati alla struttura reale di ClipVault/MD-Viewer

Solo `site/`, nessun cambio all'app. Il giro precedente aveva rifatto lo stile (font, colori, componenti) ma non tutta la struttura: header e footer restavano nel vecchio impianto (`.site-header`/`.site-footer`, niente `.wrap`/`.footer-row`), non nel sistema a due blocchi flex di ClipVault/MD-Viewer.

- **Footer di `index.html` ricostruito su `.footer-row`**: prima erano due `<p>` impilate (link, poi copyright), ora è la stessa riga `display:flex; justify-content:space-between` di ClipVault/MD-Viewer — testo di licenza/copyright a sinistra, gruppo `.footer-links` a destra (Sorgente, Segnala un problema, Licenza, Privacy, Termini, Cookie Policy), stesso ordine e stessa etichetta "Sorgente/Source" al posto di "GitHub". Misurato: stesso padding (`32px 0 44px`), stessa altezza della riga, identici a pixel a quelli di ClipVault e MD-Viewer.
- **Header di `index.html` riorganizzato in due gruppi flex** (marchio+attribuzione a sinistra, pillola versione+GitHub+selettore lingua a destra) invece di quattro elementi allineati con un `margin-right:auto` di ripiego — stesso `justify-content:space-between` a due blocchi del riferimento, con l'attribuzione "di/by MTSolutions" e il selettore lingua (che ClipVault/MD-Viewer non hanno, sono mono-lingua) integrati nella stessa struttura invece che accostati.
- **Pagine legali**: header portato sullo stesso `header.site` (marchio con `.mark`+`.word`, non più testo diretto sull'`<a>`), footer ridotto a un'unica riga compatta ("Moka — Licenza MIT. © 2026 San Marino Games S.r.l. · MTSolutions. Home · [le altre due pagine legali]"), stesso schema minimale che usano `privacy.html`/`terms.html`/`cookie-policy.html` di ClipVault e MD-Viewer (niente split a due colonne lì, solo su `index.html`).
- Contenitore delle sezioni rinominato concettualmente a wrapper condiviso (`container`, riusato anche per header/footer invece di introdurne uno nuovo) e padding-top di `.hero` riportato a `56px` come nel riferimento (era 40px).
- Verificato con screenshot automatizzati (Playwright headless, tre siti serviti in locale fianco a fianco) confrontando header/footer pixel per pixel: stesso `padding`, stesso `display:flex`/`justify-content:space-between`, stessa altezza della riga footer. Provate entrambe le lingue (IT/EN) su tutte e quattro le pagine e nessun errore in console a parte il 404 atteso di `releases/latest` (nessuna release pubblicata ancora).

## 2026-09-22 — sito: nuovo design, stesso linguaggio visivo di ClipVault/MD-Viewer

Solo `site/`, nessun cambio all'app (versione invariata, 0.0.4).

- `style.css` riscritto da zero sullo stesso sistema di ClipVault/MD-Viewer: font Sora/Inter/IBM Plex Mono, tema scuro con eyebrow in monospace, pillola versione, griglia di feature, sezione download che legge le release GitHub via API e mostra installer/MSI/portable quando esistono (oggi "nessuna build installabile ancora", perché non c'è ancora una release pubblicata). Palette propria del progetto (ambra/caffè, non il viola di ClipVault né il blu di MD-Viewer), coerente con l'icona della moka.
- `index.html` riscritto sulla stessa struttura (header con marchio+pillola versione+GitHub, hero con CTA, sezione con gli screenshot veri del pannello al posto di un mockup finto, griglia di funzioni, riga di comando, download dinamici, footer con i link legali) mantenendo tutto il contenuto bilingue IT/EN esistente e il meccanismo `data-lang`/`lang.js` invariato.
- `privacy.html`/`terms.html`/`cookie-policy.html` non toccate: condividono già `style.css`, quindi ereditano il nuovo stile automaticamente.
- Carica i font da Google Fonts (`fonts.googleapis.com`), come fanno ClipVault e MD-Viewer — diverso dalla nota "niente CDN" della release 0.0.3: è lo scambio esplicito richiesto per lo stesso linguaggio visivo. Se si preferisce restare senza CDN, i tre font vanno auto-ospitati in `site/fonts/` e la sezione `@font-face` sostituita al `<link>`.
- Verificato in locale (server statico): entrambe le lingue, stato "nessuna build" (nessuna release esiste ancora su GitHub), pagine legali, nessun errore in console a parte il 404 atteso della chiamata `releases/latest` quando non c'è nessuna release. Non verificato: build reale/emulatore (non pertinente, è solo il sito), un vero cambio di release che popoli la sezione download.

## 2026-09-22 — 0.0.4: regole automatiche e Presenza

- **Regole automatiche**: Moka si accende da sola quando serve e si spegne quando non serve più, controllando ogni 5 secondi:
  - programma aperto, app a schermo intero, in chiamata (microfono o webcam), in carica, monitor esterno, fascia oraria, download in corso, processore occupato;
  - ogni regola ha la sua modalità (solo il PC o anche lo schermo) e il suo "…e poi" quando finisce;
  - download e processore restano veri 2 minuti dopo l'ultima volta sopra soglia, la chiamata 30 secondi;
  - le sonde girano solo se una regola le usa, e mai sotto il lock dello stato.
- **Il pannello dice perché**: "Acceso · notepad.exe è aperto", e "Anche: …" quando i motivi sono più d'uno.
- **Sospendere le regole** per un'ora o fino al riavvio: spegnendo dal pannello Moka lo propone invece di spegnere e basta (la regola riaccenderebbe tutto); anche dal menu, dalle Impostazioni e da riga di comando.
- **Riga di comando**: `--while NOME`, `--while-pid N` (regole che vivono finché vive il processo, con notifica se il processo non c'è), `--pause-rules[=durata]`, `--resume-rules`.
- **Presenza**: F15 dopo 50 secondi di inattività, solo mentre Moka tiene acceso il PC; spenta di default, con l'avviso sulle regole aziendali.
- Sezioni **Regole automatiche** e **Presenza** nelle Impostazioni, con il modulo per aggiungere una regola (suggerisce i programmi aperti).
- `spike probes`: stampa ciò che vedono le sonde, per provarle a mano.
- Verificato su LPT-MIKI: programma aperto e chiuso, "…e poi" di una regola, sospensione e ripresa, `--while-pid`, `--pause-rules`, fascia oraria, in carica, processore, F15, contrasti (dettagli in `test.md`).

## 2026-09-22 — 0.0.3: tutto il necessario per la prima release

- **"…e poi"**:
  - a fine sessione a tempo spegne lo schermo, blocca, sospende, iberna o arresta il PC, dopo un conto alla rovescia di 60 s in una finestrella che non ruba il focus (Annulla, +30 min, Adesso);
  - durante l'attesa il PC resta sveglio;
  - se il PC ha dormito oltre la scadenza non fa niente;
  - a coperchio chiuso aspetta solo 10 s, e vince su ciò che Windows avrebbe fatto;
  - `--then` da riga di comando.
- **Avviso 5 minuti prima della fine**, con "+30 min".
- **Tasti rapidi globali** per accendere/spegnere e per spegnere lo schermo, da una lista sicura (niente Ctrl+Alt).
- **Aggiornamenti automatici firmati**: controllo all'avvio e ogni 24 ore, installazione solo su richiesta e mai durante una sessione; prima di installare l'impostazione del coperchio torna com'era.
- **Installer**:
  - NSIS con la pagina "Attività aggiuntive" in italiano e in inglese;
  - MSI;
  - agli aggiornamenti e alla disinstallazione Moka si chiude in modo pulito e rimette l'impostazione del coperchio (agganci NSIS e un frammento WiX).
- **Workflow `release.yml`**:
  - installer, portable e `latest.json` costruito a mano;
  - release in bozza, pulizia se fallisce;
  - controllo che il tag coincida con la versione.
- **Promemoria stella su GitHub** (dopo 5 avvii e 3 giorni).
- **Sito** (`site/`): landing, privacy, termini, cookie; italiano e inglese, stile MTSolutions, niente cookie né CDN.
- Chiave dell'updater generata fuori dal repo. Passi per pubblicare in [docs/RELEASE.md](docs/RELEASE.md).
- Verificato su LPT-MIKI:
  - conto alla rovescia, annulla e +30 min;
  - avviso dei 5 minuti;
  - tasto rapido;
  - controllo aggiornamenti con il repo privato;
  - sito in chiaro, scuro e a larghezza telefono;
  - build firmata e installer (dettagli in `test.md`).

## 2026-09-22 — 0.0.2: portatili e coperchio chiuso

- **Coperchio chiuso**. Moka cambia per il tempo necessario l'impostazione di Windows "Quando chiudo il coperchio" e la rimette sempre com'era:
  - registro su disco scritto prima di toccare Windows;
  - ripristino nello schema modificato e solo se il valore è ancora quello scritto da Moka;
  - `RunOnce` finché la modifica è attiva;
  - ripristino all'avvio, all'uscita, alla fine della sessione di Windows, con `moka --restore-lid` e con "Ripristina ora".
- **Cosa fa Moka da sola** (logica pura, con un test per ogni caso della roadmap):
  - a fine sessione a coperchio chiuso, allo stacco dell'alimentatore o allo scollegamento del monitor fa ciò che Windows avrebbe fatto, dopo 10 s;
  - protezione zaino;
  - blocco alla riapertura del coperchio.
- **Modalità scrivania**: con un monitor esterno collegato il coperchio chiuso non sospende mai il PC, anche senza una sessione.
- **Soglia batteria**: la sessione finisce da sola scendendo sotto la soglia, e lo dice con una notifica.
- **Eventi di sistema** su una finestra nascosta: coperchio, alimentazione, batteria, piano energetico, monitor, sospensione, fine della sessione di Windows.
- **Interfaccia**:
  - domanda al primo avvio sui portatili (Moka non tocca niente senza consenso);
  - riga "Anche a coperchio chiuso" nel pannello e nel menu;
  - sezioni Coperchio e Batteria nelle Impostazioni, con l'impostazione di Windows letta dal vivo e "Ripristina ora";
  - criteri aziendali riconosciuti.
- **Riga di comando**: `--lid`, `--no-lid`, `--restore-lid` (senza avviare l'app).
- **Verificato su LPT-MIKI con l'impostazione vera**, letta ogni volta: modifica e ritorno, crash con `RunOnce` e con la riapertura, scelta dell'utente rispettata, cambio di modalità, "Ripristina ora", modalità scrivania con due monitor, uscita. Alla fine l'impostazione è tornata "Sospendi" in carica e a batteria. Dettagli in `test.md`.
- **Da fare**, perché serve chiudere il coperchio: lo spike (`docs/SPIKE.md`) e le righe aperte di `test.md`.
- **Test**: 54 in Rust. Il controllo delle versioni ora guarda anche `package-lock.json`.

## 2026-09-22 — 0.0.1: il nucleo, la CI, lo strumento per lo spike

- Pianificazione chiusa: decisioni prese in [docs/ROADMAP.md](docs/ROADMAP.md) ("Decisioni prese scrivendo il codice"). Le principali:
  - il clic sinistro apre il pannello (configurabile);
  - la scelta "Solo il PC / PC e schermo" resta da una sessione all'altra, niente impostazione a parte;
  - la riga di comando anticipata alla 0.1;
  - la sessione riprende dopo un riavvio dell'app, non dopo un riavvio del PC né dopo un nuovo accesso.
- Check sulla macchina LPT-MIKI, un portatile con standby moderno: proprio il caso difficile. Scoperte:
  - l'azione del coperchio è nascosta (`powercfg /qh`, non `/q`);
  - si scrive senza elevazione;
  - lo stato di esecuzione del sistema si legge senza amministratore e rivela le richieste attive.
- Codice (Tauri 2, Rust, HTML/CSS/JS scritti a mano):
  - richieste di alimentazione con motivo leggibile;
  - sessioni a durata, fino alle HH:MM, per sempre;
  - "spegni lo schermo ora";
  - icona nella tray con tre stati, due varianti per barra chiara e scura (cambio al volo) e sei dimensioni (a 16 px disegnata pixel per pixel);
  - menu del clic destro;
  - pannello accanto all'icona;
  - Impostazioni create solo quando servono;
  - italiano e inglese da un'unica fonte;
  - riga di comando.
- Verificato sulla macchina (dettagli in `test.md`):
  - la riga di comando arriva all'istanza aperta;
  - le sessioni scadono da sole;
  - la richiesta sparisce se Moka viene chiusa a forza, e la sessione riprende alla riapertura;
  - `--quit`;
  - Impostazioni, lingua;
  - contrasti misurati in chiaro e scuro.
- Test: 29 in Rust, uno dei quali crea e rilascia una richiesta di alimentazione vera; 5 sulle traduzioni. Controlli su sintassi JS e versioni allineate.
- CI su GitHub Actions: Rust su Windows, pagine su Linux, `concurrency` fuori da `main`. Workflow manuale `build` per gli installer.
- Strumento per lo spike sullo standby moderno (`src-tauri/examples/spike.rs`) e procedura in [docs/SPIKE.md](docs/SPIKE.md). Manca l'esecuzione: serve qualcuno che chiuda il coperchio.

## 2026-09-21 — Coperchio chiuso come funzione di punta

- Il coperchio chiuso passa da funzione avanzata (0.4) a funzione di punta, con una tappa tutta sua (0.2) subito dopo il nucleo. La prima release pubblica slitta di conseguenza alla 0.3.
- Opzioni volutamente poche:
  - tre scelte ("come sempre", "resta acceso solo se è in carica", "anche a batteria");
  - modalità scrivania con monitor esterno;
  - blocco del PC alla riapertura del coperchio;
  - protezione zaino a tempo.
- Tutti i casi limite gestiti senza opzioni: a fine sessione a coperchio chiuso Moka fa ciò che Windows avrebbe fatto (Windows agisce solo nel momento della chiusura), idem quando si stacca l'alimentatore o si scollega il monitor.
- Modifica dell'impostazione di Windows progettata per non restare mai cambiata:
  - registro scritto su disco prima di toccare niente;
  - ripristino nello schema modificato, e solo se l'utente non l'ha cambiata nel frattempo;
  - sette strade di ripristino (comprese `RunOnce` e la disinstallazione), più il comando a mano nel README.
- Individuato il rischio principale del progetto: sui portatili con standby moderno il metodo classico non tiene sveglio il PC a schermo spento (casi documentati in PowerToys e ChargeKeeper). Prima dell'interfaccia del coperchio si fa una prova tecnica su un portatile vero, con ipotesi e metodo di misura già scritti.
- Dettagli in [docs/ROADMAP.md](docs/ROADMAP.md), sezione "Portatili: coperchio chiuso".

## 2026-09-21 — Progetto avviato

- Idea: un keep-awake per Windows nello spirito di Amphetamine (macOS), open source e completamente locale.
- Decisioni: nome **Moka**; repo `TarducciM/Moka`, privato fino alla prima versione da mostrare e poi pubblico; licenza MIT; Tauri 2 + Rust + HTML/CSS/JS senza framework; identifier `com.moka.app`; solo Windows 10/11; italiano e inglese; Presenza inclusa, spenta di default.
- Piano completo, roadmap per versione, architettura e trappole già note in [docs/ROADMAP.md](docs/ROADMAP.md).
- Nessun codice ancora: il lavoro riparte da un PC con la toolchain Rust installata.
