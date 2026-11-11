const track = document.querySelector(".slider-track");
const slides = [...track.children];
const arrows = document.querySelectorAll(".slider-arrow");
const count = document.querySelector(".slider-count");
const dotsWrap = document.querySelector(".slider-dots");
const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");

let current = 0;
// Where the slider is heading, so quick repeat clicks don't count from a slide still scrolling past.
let target = 0;
let settleTimer;

const dots = slides.map((slide, i) => {
  slide.setAttribute("aria-roledescription", "slide");
  slide.setAttribute("aria-label", `${i + 1} of ${slides.length}`);

  const dot = document.createElement("button");
  dot.type = "button";
  dot.className = "slider-dot";
  dot.setAttribute("aria-label", `Go to project ${i + 1}`);
  dot.addEventListener("click", () => goTo(i));
  dotsWrap.append(dot);
  return dot;
});

function goTo(index) {
  target = Math.max(0, Math.min(index, slides.length - 1));
  track.scrollTo({
    left: slides[target].offsetLeft,
    behavior: reducedMotion.matches ? "auto" : "smooth",
  });
}

function render() {
  count.textContent = `${current + 1} / ${slides.length}`;
  dots.forEach((dot, i) => {
    if (i === current) dot.setAttribute("aria-current", "true");
    else dot.removeAttribute("aria-current");
  });
  // aria-disabled rather than disabled, so keyboard focus stays on the arrow at either end.
  arrows.forEach((arrow) => {
    const atEnd = arrow.dataset.dir === "-1" ? current === 0 : current === slides.length - 1;
    arrow.setAttribute("aria-disabled", String(atEnd));
  });
}

function syncToScroll() {
  const maxScroll = track.scrollWidth - track.clientWidth;
  let nearest = 0;

  // The last slide can't reach the left edge when it is narrower than the track.
  if (track.scrollLeft >= maxScroll - 1) {
    nearest = slides.length - 1;
  } else {
    slides.forEach((slide, i) => {
      const distance = Math.abs(slide.offsetLeft - track.scrollLeft);
      if (distance < Math.abs(slides[nearest].offsetLeft - track.scrollLeft)) nearest = i;
    });
  }

  if (nearest !== current) {
    current = nearest;
    render();
  }
}

track.addEventListener("scroll", () => {
  syncToScroll();
  clearTimeout(settleTimer);
  settleTimer = setTimeout(() => {
    target = current;
  }, 150);
}, { passive: true });

arrows.forEach((arrow) => {
  arrow.addEventListener("click", () => goTo(target + Number(arrow.dataset.dir)));
});

track.addEventListener("keydown", (event) => {
  if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
    event.preventDefault();
    goTo(target + (event.key === "ArrowRight" ? 1 : -1));
  }
});

render();
