# Changelog

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
