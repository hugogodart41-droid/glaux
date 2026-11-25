// Infobulle maison : les infobulles natives de macOS ne s'affichent pas
// dans un panneau de barre des menus. Tout élément avec un attribut
// data-tooltip en a une ; un seul écouteur pour tout le panneau (délégation),
// ce qui marche aussi pour les éléments recréés à chaque actualisation.

const DELAY_MS = 250; // délai avant affichage : évite le clignotement en passant
const GAP = 6; // espace entre l'élément survolé et l'infobulle
const EDGE = 8; // marge minimale avec les bords du panneau

export function setupTooltips(): void {
  const tip = document.getElementById("tooltip");
  if (!tip) return;

  let timer: number | undefined;
  let current: Element | null = null;

  const hide = () => {
    window.clearTimeout(timer);
    current = null;
    tip.hidden = true;
  };

  const show = (target: Element) => {
    const text = target.getAttribute("data-tooltip");
    if (!text || !target.isConnected) return;
    tip.textContent = text;
    tip.hidden = false;

    // Au-dessus de l'élément, centrée ; en dessous s'il n'y a pas la place.
    const anchor = target.getBoundingClientRect();
    const box = tip.getBoundingClientRect();
    const maxX = window.innerWidth - box.width - EDGE;
    const x = Math.max(EDGE, Math.min(anchor.left + anchor.width / 2 - box.width / 2, maxX));
    let y = anchor.top - box.height - GAP;
    if (y < EDGE) y = anchor.bottom + GAP;
    tip.style.left = `${Math.round(x)}px`;
    tip.style.top = `${Math.round(y)}px`;
  };

  document.addEventListener("mouseover", (event) => {
    const target = (event.target as Element | null)?.closest("[data-tooltip]") ?? null;
    if (target === current) return;
    hide();
    if (!target) return;
    current = target;
    timer = window.setTimeout(() => show(target), DELAY_MS);
  });

  // La souris quitte le panneau, un clic, une touche, le panneau se ferme : on masque.
  document.documentElement.addEventListener("mouseleave", hide);
  document.addEventListener("mouseout", (event) => {
    if (!event.relatedTarget) hide(); // sortie de la fenêtre
  });
  document.addEventListener("mousedown", hide);
  window.addEventListener("keydown", hide);
  window.addEventListener("blur", hide);
}
