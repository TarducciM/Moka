# Changelog

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
