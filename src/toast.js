// La finestrella degli avvisi: "Moka si spegne tra 5 min" e il conto alla
// rovescia di "…e poi". Il testo e i secondi arrivano da Rust ogni mezzo
// secondo; la pagina disegna e basta. Si chiude da sola quando Rust la toglie.

(() => {
  const { invoke } = window.__TAURI__.core;
  const $ = (id) => document.getElementById(id);
  const card = $("toast");
  let shown = false;
  let lastHeight = 0;

  async function refresh() {
    let toast;
    try {
      toast = await invoke("get_toast");
    } catch {
      return;
    }
    if (!toast) return;
    if (I18n.lang !== toast.lang) {
      await I18n.load(toast.lang);
      I18n.apply();
    }
    const countdown = toast.kind === "countdown";
    $("toast-title").textContent = toast.title;
    $("toast-body").textContent = toast.body;
    $("t-cancel").hidden = !countdown;
    $("t-now").hidden = !countdown;
    $("t-ok").hidden = countdown;
    if (toast.act) $("t-now").textContent = toast.act;
    $("toast-progress").hidden = !countdown;
    if (countdown && toast.totalSeconds) {
      const left = Math.max(0, toast.seconds) / toast.totalSeconds;
      $("toast-bar").style.width = `${Math.round(left * 100)}%`;
    }

    const height = Math.ceil(card.getBoundingClientRect().height);
    if (!shown) {
      shown = true;
      lastHeight = height;
      await invoke("toast_ready", { height });
    } else if (height !== lastHeight) {
      lastHeight = height;
      invoke("fit_toast", { height });
    }
  }

  const act = (command, args) => () => invoke(command, args).then(refresh);
  $("t-cancel").addEventListener("click", act("cancel_countdown"));
  $("t-now").addEventListener("click", act("countdown_now"));
  $("t-extend").addEventListener("click", act("extend_session", { minutes: 30 }));
  $("t-ok").addEventListener("click", act("dismiss_warning"));

  refresh();
  setInterval(refresh, 500);
})();
