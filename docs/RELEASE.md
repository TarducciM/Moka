# Pubblicare una versione

La prima release pubblica è la **0.5.0** (non la 0.3.0 prevista dalla roadmap: nel frattempo sono arrivate anche le tappe 0.4 e 0.5, e il numero deve dire cosa c'è dentro). Tutto il necessario è pronto nel repo: workflow `release.yml`, installer, updater, sito. Mancano i passi che toccano l'esterno o che non si possono annullare, e restano una decisione di Michele.

## Prima di tutto

- [ ] **Lo spike** sullo standby moderno fatto e scritto nella roadmap (`docs/SPIKE.md`). Senza, "anche a coperchio chiuso" e "spegni lo schermo ora" sui portatili moderni restano promesse non verificate.
- [ ] Le righe aperte di `test.md`, almeno quelle della tray, del menu e dell'installer.

## Una volta sola

### 1. La chiave dell'updater

La coppia di chiavi è stata generata il 2026-09-22 su LPT-MIKI:

- **privata**: `%USERPROFILE%\.tauri\moka-updater.key`, senza password. **Mai nel repo.**
- **pubblica**: già in `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`).

Da fare:

- [ ] una **copia di sicurezza** della chiave privata fuori da questo PC (gestore di password, chiavetta). Se si perde, le versioni già installate non potranno più aggiornarsi: bisognerebbe reinstallare a mano.
- [ ] metterla nei secret del repo:

```bash
gh secret set TAURI_SIGNING_PRIVATE_KEY --repo TarducciM/Moka < "%USERPROFILE%\.tauri\moka-updater.key"
```

(senza password non serve `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`).

### 2. Il repo pubblico

L'updater scarica da `releases/latest/download`, che per un repo privato non è raggiungibile: il repo deve essere pubblico **prima** della prima release. Non si torna indietro senza conseguenze (fork, stelle, link già condivisi).

```bash
gh repo edit TarducciM/Moka --visibility public --accept-visibility-change-consequences
```

### 3. Il sito

- [ ] DNS `moka.mtsolutions.studio` e hosting della cartella `site/`, come per ClipVault (vedi `MTSolutions/docs/VPS_ONBOARDING.md`).
- [ ] Voce nella sezione "Progetti open source" della home di mtsolutions.studio (`MTSolutions-Sites/index.html`), con l'icona in `MTSolutions-Sites/moka/icon.svg` (è `site/favicon.svg`).
- [ ] Far rivedere a un legale `site/privacy.html`, `site/terms.html`, `site/cookie-policy.html`: sono bozze scritte con i dati reali del titolare, come quelle delle altre app.

## A ogni versione

1. Versione aggiornata ovunque (`npm run version:check` deve dire "ovunque"): `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `Cargo.lock`.
2. Voce datata nel `CHANGELOG.md`.
3. Commit e push su `main`, CI verde.
4. Il tag, che fa partire `release.yml`:

   ```bash
   git tag v0.5.0
   ```

   ```bash
   git push origin v0.5.0
   ```

5. Il workflow costruisce NSIS, MSI e portable, firma l'installer NSIS per l'updater, costruisce e controlla `latest.json` e crea la release **in bozza**, con la tabella dei download. Se la build fallisce, la bozza si cancella da sola.
6. Guardare la bozza su GitHub (allegati, note) e **pubblicarla a mano**. Da quel momento le versioni installate la vedono entro 24 ore.

Il workflow controlla che il tag coincida con la versione dell'app: un `v0.5.0` su un'app che dice `0.0.8` si ferma prima di costruire.

## Stato al 2026-09-23

| Passo | Stato |
|---|---|
| Versione allineata ovunque (0.5.0) e CHANGELOG | fatto |
| CI verde su `main` | fatto |
| Secret `TAURI_SIGNING_PRIVATE_KEY` | **da fare**: `gh secret list` sul repo non mostra niente. Senza, il workflow si ferma alla firma |
| Repo pubblico | **da fare**: `gh repo view` dice ancora `PRIVATE`. L'updater scarica da `releases/latest/download`, che su un repo privato non è raggiungibile |
| Tag `v0.5.0` e bozza della release | in attesa dei due passi qui sopra |
| Sito, voce nella home, revisione legale | da fare |
| Spike (`docs/SPIKE.md`) | non fatto: le promesse sul coperchio restano dichiarate "in prova" nel sito e nel README |

## Verificato in locale (2026-09-22, LPT-MIKI)

La stessa build della release, con la chiave locale al posto del secret:

```bash
set TAURI_SIGNING_PRIVATE_KEY=<contenuto di moka-updater.key>
npx tauri build --config src-tauri/tauri.release.conf.json
```

Esito e prove dell'installer: `test.md`.
