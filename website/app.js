// Die Seite funktioniert auch ohne dieses Skript: alle Links zeigen auf echte Dateien.
// Es markiert nur das eigene System und ergänzt Version und Dateigrößen.
(function () {
  "use strict";
  document.documentElement.classList.add("js");

  const RELEASE_API = "https://api.github.com/repos/martinkopoky-wq/panzergame/releases/latest";

  function detectSystem() {
    const ua = navigator.userAgent;
    if (/Android|iPhone|iPad|Mobile/i.test(ua)) return null;
    const platform = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || ua;
    if (/Win/i.test(platform)) return "windows";
    if (/Mac/i.test(platform)) return "macos";
    if (/Linux|X11/i.test(platform)) return "linux";
    return null;
  }

  function markSystem() {
    const system = detectSystem();
    const row = system && document.querySelector('.system[data-system="' + system + '"]');
    const other = document.getElementById("hero-other");
    if (!row) {
      if (other) other.hidden = true;
      return;
    }
    row.classList.add("current");
    const tag = document.createElement("small");
    tag.textContent = "Dein System";
    row.querySelector("h3").append(tag);

    const hero = document.getElementById("hero-download");
    const first = row.querySelector(".get a");
    hero.href = first.href;
    hero.querySelector("span").textContent = "Für " + row.querySelector("h3").firstChild.textContent.trim() + " herunterladen";
  }

  function formatSize(bytes) {
    const mb = bytes / (1024 * 1024);
    return (mb < 10 ? mb.toFixed(1).replace(".", ",") : Math.round(mb)) + " MB";
  }

  // Schlägt das fehl (kein Netz, Limit erreicht), fehlen nur Version und Größen
  async function showRelease() {
    try {
      const response = await fetch(RELEASE_API, { headers: { Accept: "application/vnd.github+json" } });
      if (!response.ok) return;
      const release = await response.json();
      const sizes = new Map(release.assets.map((asset) => [asset.name, asset.size]));
      document.querySelectorAll("[data-asset]").forEach((link) => {
        const size = sizes.get(link.dataset.asset);
        const target = link.querySelector(".size");
        if (size && target) target.textContent = formatSize(size);
      });
      const news = document.createElement("a");
      news.href = "neuigkeiten.html#" + release.tag_name;
      news.textContent = "Aktuell: " + release.tag_name + ", was ist neu?";
      document.getElementById("version").replaceChildren(news);
    } catch (error) {
      console.debug("Release-Infos nicht geladen", error);
    }
  }

  // Für die Endlosschleife muss die Liste zweimal hintereinander stehen
  function startMarquee() {
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const marquee = document.querySelector(".marquee");
    if (!marquee || reduced) return;
    const list = marquee.querySelector(".items");
    Array.from(list.children).forEach((item) => {
      const copy = item.cloneNode(true);
      copy.setAttribute("aria-hidden", "true");
      list.append(copy);
    });
    marquee.classList.add("running");
  }

  function revealOnScroll() {
    const items = document.querySelectorAll(".reveal");
    if (!("IntersectionObserver" in window)) {
      items.forEach((item) => item.classList.add("in"));
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            entry.target.classList.add("in");
            observer.unobserve(entry.target);
          }
        });
      },
      { rootMargin: "0px 0px -8% 0px" }
    );
    items.forEach((item) => observer.observe(item));
  }

  markSystem();
  startMarquee();
  revealOnScroll();
  showRelease();
})();
