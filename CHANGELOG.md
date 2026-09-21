# Changelog

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
