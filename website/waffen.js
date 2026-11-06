// Baut die Waffen-Doku aus data/waffen.json. Ohne Skript bleibt der Hinweis stehen.
(function () {
  "use strict";
  const grid = document.getElementById("docs-grid");
  const buttons = document.querySelectorAll("[data-filter]");
  let items = [];

  function card(item) {
    const li = document.createElement("li");
    li.className = "docs-card";
    li.dataset.group = item.group;

    const media = document.createElement("div");
    media.className = "docs-media";
    const video = document.createElement("video");
    video.muted = true;
    video.loop = true;
    video.playsInline = true;
    video.preload = "none";
    video.poster = item.poster;
    video.setAttribute("aria-label", "Video: " + item.name);
    video.src = item.video;
    media.appendChild(video);
    li.appendChild(media);

    // Abspielen nur, solange die Karte sichtbar ist
    video.addEventListener("pointerenter", () => video.play().catch(() => {}));
    video.addEventListener("pointerleave", () => video.pause());

    const body = document.createElement("div");
    body.className = "docs-body";
    const head = document.createElement("div");
    head.className = "docs-head";
    const title = document.createElement("h2");
    title.textContent = item.name;
    const tag = document.createElement("span");
    tag.className = "docs-tag";
    tag.textContent = item.group;
    head.append(title, tag);
    const text = document.createElement("p");
    text.textContent = item.text;
    body.append(head, text);
    if (item.ammo > 0) {
      const ammo = document.createElement("p");
      ammo.className = "docs-ammo";
      ammo.textContent = (item.ammo === 999 ? "Dauer" : "Munition") + ": " + ammoText(item);
      body.appendChild(ammo);
    }
    li.appendChild(body);
    return li;
  }

  function ammoText(item) {
    if (item.ammo === 999) return "bis zur Rundenende";
    if (item.group === "Effekt") return item.ammo + " s";
    if (["Flame", "Mg"].includes(item.id)) return item.ammo + " Schuss (Dauerfeuer)";
    return item.ammo + "×";
  }

  function render(filter) {
    grid.replaceChildren();
    items
      .filter((i) => filter === "all" || i.group === filter)
      .forEach((i) => grid.appendChild(card(i)));
  }

  buttons.forEach((b) =>
    b.addEventListener("click", () => {
      buttons.forEach((o) => o.setAttribute("aria-pressed", String(o === b)));
      render(b.dataset.filter);
    })
  );

  fetch("data/waffen.json")
    .then((r) => r.json())
    .then((data) => {
      items = data;
      render("all");
    })
    .catch(() => {
      grid.textContent = "Die Waffenliste konnte nicht geladen werden.";
    });
})();
