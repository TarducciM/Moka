// Il pannello della tray. Disegna lo stato che arriva da Rust (testi già
// tradotti e formattati) e manda le azioni con invoke: qui non si calcola
// niente, così pannello, menu e tooltip non possono raccontare cose diverse.

(() => {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;

  const $ = (id) => document.getElementById(id);
  const panel = $("panel");
  const power = $("power");
  const status = $("status");
  const modeHint = $("mode-hint");
  const chips = $("durations");
  const untilForm = $("until-form");
  const untilInput = $("until");
  const welcome = $("welcome");
  const lidQuestion = $("lid-question");
  const lidRow = $("lid-row");
  const lidSwitch = $("lid-switch");
  const lidHint = $("lid-row-hint");
  const thenSelect = $("then");
  let thenKey = "";
  const modeButtons = [...document.querySelectorAll("[data-mode]")];

  let state = null;
  let chipsKey = "";
  let lastHeight = 0;
  let ticker = null;

  async function apply(next) {
    state = next;
    if (I18n.lang !== next.lang) {
      await I18n.load(next.lang);
      I18n.apply();
      chipsKey = "";
    }
    render();
  }

  async function refresh() {
    try {
      await apply(await invoke("get_state"));
    } catch (err) {
      console.error("moka: stato non letto", err);
    }
  }

  function render() {
    const s = state;
    document.body.classList.toggle("is-on", s.active);
    document.body.classList.toggle("is-display", s.active && s.mode === "display");

    status.textContent = s.status;
    power.setAttribute("aria-checked", String(s.active));
    power.setAttribute("aria-label", I18n.t(s.active ? "popover.power_off" : "popover.power_on"));

    for (const btn of modeButtons) {
      const selected = btn.dataset.mode === s.mode;
      btn.setAttribute("aria-checked", String(selected));
      btn.tabIndex = selected ? 0 : -1;
    }
    modeHint.textContent = I18n.t(s.mode === "display" ? "mode.display_hint" : "mode.system_hint");

    renderChips(s);

    if (document.activeElement !== untilInput) {
      untilInput.value = s.untilDefault;
    }
    const untilActive = s.active && s.spec && s.spec.kind === "until";
    untilForm.classList.toggle("is-active", Boolean(untilActive));

    welcome.hidden = !s.welcome;

    // Coperchio: la domanda finché non ha risposta, poi la riga per la sessione.
    lidQuestion.hidden = !s.lidQuestion;
    lidRow.hidden = !s.lidRow;
    lidSwitch.setAttribute("aria-checked", String(s.lid));
    lidHint.textContent = I18n.t(s.lidMode === "always" ? "lid.row_hint_always" : "lid.row_hint_ac");

    // "…e poi": le scelte (e le etichette tradotte) arrivano da Rust.
    const key = s.thenChoices.map((c) => c.value + ":" + c.label).join("|");
    if (key !== thenKey) {
      thenKey = key;
      thenSelect.replaceChildren(
        ...s.thenChoices.map((c) => {
          const option = document.createElement("option");
          option.value = c.value;
          option.textContent = c.label;
          return option;
        }),
      );
    }
    if (document.activeElement !== thenSelect) thenSelect.value = s.then;
    $("then-row").hidden = !s.thenRow;

    // Aggiornamento: mai durante una sessione (cadrebbe a metà).
    $("update-card").hidden = !s.update;
    if (s.update) {
      $("update-text").textContent = I18n.t("popover.update", { version: s.update });
      $("update-btn").hidden = s.active;
      $("update-hint").hidden = !s.active;
    }
    $("star-card").hidden = !s.star || s.welcome || s.lidQuestion;
    fit();
  }

  function renderChips(s) {
    const key = s.durations.map((d) => `${d.minutes}:${d.label}`).join("|");
    if (key !== chipsKey) {
      chipsKey = key;
      chips.replaceChildren(
        ...s.durations.map((d) => chip(d.label, { kind: "minutes", minutes: d.minutes })),
        chip("∞", { kind: "never" }, I18n.t("popover.forever")),
      );
    }
    for (const el of chips.children) {
      const spec = JSON.parse(el.dataset.spec);
      const pressed =
        s.active &&
        s.spec &&
        s.spec.kind === spec.kind &&
        (spec.kind !== "minutes" || s.spec.minutes === spec.minutes);
      el.setAttribute("aria-pressed", String(Boolean(pressed)));
    }
  }

  function chip(text, spec, label) {
    const el = document.createElement("button");
    el.type = "button";
    el.className = "chip";
    el.textContent = text;
    el.dataset.spec = JSON.stringify(spec);
    if (label) {
      el.setAttribute("aria-label", label);
      el.title = label;
    }
    el.addEventListener("click", () => act("start_session", { spec }));
    return el;
  }

  // L'altezza della finestra segue il contenuto (il benvenuto, una lingua
  // con frasi più lunghe): la misura la pagina, la applica Rust.
  function fit() {
    requestAnimationFrame(() => {
      const height = Math.ceil(panel.getBoundingClientRect().height);
      if (height && height !== lastHeight) {
        lastHeight = height;
        invoke("fit_popover", { height }).catch(() => {});
      }
    });
  }

  async function act(command, args) {
    try {
      await invoke(command, args);
    } catch (err) {
      console.error(`moka: ${command} non riuscito`, err);
    }
    await refresh();
  }

  // Il conto alla rovescia gira solo mentre il pannello si vede.
  function startTicker() {
    if (!ticker) ticker = setInterval(refresh, 1000);
  }
  function stopTicker() {
    clearInterval(ticker);
    ticker = null;
  }

  power.addEventListener("click", () => act("toggle_session"));

  for (const btn of modeButtons) {
    btn.addEventListener("click", () => act("set_mode", { mode: btn.dataset.mode }));
  }
  // Frecce dentro il gruppo di radio, come vuole il pattern ARIA.
  document.querySelector(".segmented").addEventListener("keydown", (event) => {
    if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) return;
    event.preventDefault();
    const next = modeButtons.find((b) => b.getAttribute("aria-checked") !== "true");
    next.focus();
    next.click();
  });

  untilForm.addEventListener("submit", (event) => {
    event.preventDefault();
    const [hour, minute] = untilInput.value.split(":").map(Number);
    if (Number.isInteger(hour) && Number.isInteger(minute)) {
      act("start_session", { spec: { kind: "until", hour, minute } });
    }
  });

  $("screen-off").addEventListener("click", () => act("screen_off"));
  $("open-settings").addEventListener("click", () => act("open_settings"));
  $("welcome-ok").addEventListener("click", () => act("dismiss_welcome"));
  lidSwitch.addEventListener("click", () => act("set_lid", { on: !state.lid }));
  thenSelect.addEventListener("change", () => act("set_then", { then: thenSelect.value }));
  $("update-btn").addEventListener("click", () => act("install_update"));
  $("star-open").addEventListener("click", () => {
    window.__TAURI__.opener.openUrl("https://github.com/TarducciM/Moka").catch(() => {});
    act("answer_star", { never: true });
  });
  $("star-later").addEventListener("click", () => act("answer_star", { never: false }));
  $("star-never").addEventListener("click", () => act("answer_star", { never: true }));
  $("lid-confirm").addEventListener("click", () => {
    const mode = document.querySelector('input[name="lid-answer"]:checked').value;
    act("answer_lid", { mode, desk: $("lid-desk").checked });
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") invoke("hide_popover");
  });

  // Il pannello si nasconde quando perde il focus: lì il conto alla rovescia
  // si ferma anche se la WebView non segnala di essere nascosta.
  window.addEventListener("blur", stopTicker);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") {
      refresh();
      startTicker();
    } else {
      stopTicker();
    }
  });

  listen("moka://state", (event) => apply(event.payload));
  listen("moka://popover-shown", () => {
    refresh();
    startTicker();
  });
  listen("moka://focus-until", () => {
    untilInput.focus();
  });

  refresh();
})();
