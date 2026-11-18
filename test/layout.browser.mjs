// Browser-only regression test. Invoke from the LOCAL development preview:
// await (await import("/layout.browser.mjs")).checkLayout()
// All IPC is mocked inside isolated frames: no prompts, resets or settings are submitted.
export async function checkLayout() {
  const results = [];
  const failures = [];
  const timestamp = Math.floor(Date.now() / 1000);
  for (const locale of ["ca", "es", "en", "fr", "de", "it"]) {
    for (const [width, height] of [[460, 600], [460, 599], [460, 556], [400, 520], [400, 480]]) {
      const frame = document.createElement("iframe");
      frame.style.cssText = `width:${width}px;height:${height}px;border:0`;
      frame.title = `Layout test ${locale} ${width}×${height}`;
      const state = {
        locale,
        settings: { language: locale, automation: false, model: "gpt-6-luna", codexPath: "" },
        usage: {
          windows: [
            { used: 39, minutes: 300, resetsAt: timestamp + 10200 },
            { used: 36, minutes: 10080, resetsAt: timestamp + 180000 },
          ],
          credits: 2, expiresAt: timestamp + 100000, accountId: "test-only", updatedAt: timestamp,
        },
        error: null, busy: null, lastPromptAt: null, nextPromptAt: null,
        pendingReset: false, autostart: false, logs: [],
      };
      frame.srcdoc = `<!doctype html><html><head><link rel="stylesheet" href="/main.css"></head><body>
        <main id="app"></main><script>
          window.__testState=${JSON.stringify(state)};
          window.__TAURI_INTERNALS__={
            transformCallback:()=>1,
            invoke:async command=>command==="get_state"?window.__testState:1
          };
        </script><script type="module" src="/main.js"></script></body></html>`;
      try {
        await new Promise((resolve, reject) => {
          frame.onload = resolve;
          frame.onerror = reject;
          document.body.append(frame);
        });
        const doc = frame.contentDocument;
        if (!doc.querySelector(".cycle-card")) throw new Error("Frontend did not load");
        const checks = [];
        for (const card of doc.querySelectorAll(".card")) {
          const rect = card.getBoundingClientRect();
          const style = frame.contentWindow.getComputedStyle(card);
          // Check the inner padding boundary as well as overflow: clipped/hidden text
          // and captions drawn across a card border must NOT count as passing.
          const bottom = rect.bottom - parseFloat(style.borderBottomWidth) - parseFloat(style.paddingBottom);
          const top = rect.top + parseFloat(style.borderTopWidth) + parseFloat(style.paddingTop);
          checks.push({ name: `${card.className}: overflow`, pass: card.scrollHeight <= card.clientHeight });
          for (const child of card.children) {
            const box = child.getBoundingClientRect();
            checks.push({ name: `${card.className}: ${child.className || child.tagName}`, pass: box.bottom <= bottom + 1 && box.top >= top - 1 });
          }
        }
        const note = doc.querySelector(".cycle-card > .caption").getBoundingClientRect();
        const nextCard = doc.querySelector(".credit-card").getBoundingClientRect();
        checks.push({ name: "Note stays above credit card", pass: note.bottom < nextCard.top });
        const usage = doc.querySelector("#view-usage");
        checks.push({ name: "Usage panel fits", pass: usage.scrollHeight <= usage.clientHeight });
        doc.querySelector("#tab-settings").click();
        doc.querySelector("#advanced").open = true;
        const settings = doc.querySelector("#view-settings");
        checks.push({ name: "Expanded settings fit", pass: settings.scrollHeight <= settings.clientHeight });
        checks.push({ name: "No page scrollbar", pass: doc.documentElement.scrollHeight <= height });
        const failed = checks.filter((check) => !check.pass).map((check) => check.name);
        results.push({ locale, width, height, failed });
        if (failed.length) failures.push({ locale, width, height, failed });
      } finally {
        frame.remove();
      }
    }
  }
  return { tested: results.length, passed: results.length - failures.length, failures };
}
