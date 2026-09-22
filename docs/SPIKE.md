# Spike: lo standby moderno tiene sveglio il PC?

È la prova tecnica che la roadmap chiede **prima** di promettere "spegni lo schermo" e "coperchio chiuso". Sui portatili con standby moderno (S0 Low Power Idle), a schermo spento Windows può mandare il PC in standby anche con una richiesta `SystemRequired` attiva: è successo a PowerToys ([#48965](https://github.com/microsoft/powertoys/issues/48965)) e a ChargeKeeper ([#170](https://github.com/0z00z0/ChargeKeeper/issues/170)). Qui si misura, non si indovina.

Serve una persona davanti al portatile: le prove spengono lo schermo o chiedono di chiudere il coperchio, e Claude non può farlo al posto tuo. C'è anche un motivo in più per non lasciarle a una sessione automatica: se il PC va davvero in standby, si ferma anche lei.

## Il portatile di riferimento

| | |
|---|---|
| Macchina | LPT-MIKI (Acer Nitro ANV16S-41) |
| Sospensione | Standby moderno **connesso alla rete** (S0). S3 non disponibile (anche Device Guard lo disattiva). Ibernazione sì. |
| Coperchio | "Sospendi" in carica e a batteria. L'impostazione è **nascosta** (`ATTRIB_HIDE`): `powercfg /q` non la mostra, serve `powercfg /qh`. |
| Criteri aziendali | Nessuno sul coperchio: Moka può cambiarlo. |

## Preparazione (una volta)

```bash
cd src-tauri
cargo build --release --example spike
```

L'eseguibile è `src-tauri/target/release/examples/spike.exe`. Lancialo da un terminale in una cartella qualunque: il log (`spike-AAAAMMGG-HHMMSS.csv`) finisce lì.

```bash
spike info
```

stampa com'è fatto il PC, l'azione del coperchio attuale e se una richiesta si vede nello stato di esecuzione del sistema (senza amministratore).

## Come si legge il risultato

`spike run` scrive un battito ogni 10 secondi. Alla fine (o con `spike report FILE`) cerca i **buchi**: un intervallo di più di 25 s vuol dire che il processo non ha girato, cioè che il PC è andato in standby. Poi elenca gli eventi Kernel-Power dello stesso intervallo (42 sospensione, 107 ripresa, 506/507 ingresso/uscita dallo standby moderno).

- **Nessun buco** e nessun evento 506: la combinazione di richieste tiene sveglio il PC.
- **Buchi**: non basta. Il primo buco dice dopo quanto è entrato in standby.

Se durante la prova lo schermo si riaccende perché hai toccato il mouse, la prova non vale: rifalla.

## Le prove, in ordine

Tutte in carica, tranne l'ultima. Fra una prova e l'altra, riaccendi lo schermo e aspetta che lo spike abbia stampato il risultato.

| # | Comando | Cosa fare | Domanda |
|---|---|---|---|
| A | `spike run --minutes 10 --screen-off` | Niente: lo schermo si spegne da solo dopo 5 s. Non toccare niente per 10 minuti. | Basta `SystemRequired` a schermo spento? |
| B | `spike run --minutes 10 --screen-off --execution` | Come A. | Serve anche `ExecutionRequired`? |
| C | `spike run --minutes 10 --screen-off --display` | Come A. | Tenere anche `DisplayRequired` evita lo standby? (lo schermo resta spento lo stesso: l'abbiamo spento noi) |
| D | `spike run --minutes 30 --lid` | Quando lo dice, chiudi il coperchio. Riaprilo dopo 30 minuti. | A coperchio chiuso con "non fare nulla", basta `SystemRequired`? |
| E | `spike run --minutes 30 --lid --display` | Come D. | E con `DisplayRequired`? |
| F | la combinazione migliore fra D ed E | Prima di chiudere il coperchio premi **Win+L**. | La schermata di blocco annulla le richieste? |
| G | la combinazione migliore, con `--lid-battery` al posto di `--lid` | Stacca l'alimentatore, poi chiudi il coperchio. | A batteria vale lo stesso? |

Con `--lid` lo spike mette l'azione del coperchio su "non fare nulla" e la **rimette com'era** alla fine, anche con Ctrl+C. Se qualcosa va storto (il terminale chiuso di colpo), stampa all'inizio i comandi per rimetterla a mano. Controlla sempre con:

```bash
powercfg /qh SCHEME_CURRENT SUB_BUTTONS LIDACTION
```

(Le ultime due righe devono tornare a `0x00000001`, cioè "Sospendi", come prima.)

## Risultati

Annota qui ogni prova con data, esito e il riassunto dello spike (battiti, buchi, eventi). Il risultato decide quali richieste tiene Moka su ciascun tipo di PC, e va riportato nella roadmap prima di iniziare la 0.2.

| Data | Prova | Esito | Note |
|---|---|---|---|
| | A | | |
| | B | | |
| | C | | |
| | D | | |
| | E | | |
| | F | | |
| | G | | |
