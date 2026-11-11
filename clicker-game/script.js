const upgrades = [
  { id: "role", icon: "✦", name: "Breakout Role", description: "+1 point per click", baseCost: 25, clickPower: 1 },
  { id: "interview", icon: "◌", name: "Press Interview", description: "+2 points per second", baseCost: 80, perSecond: 2 },
  { id: "premiere", icon: "◆", name: "Movie Premiere", description: "+8 points per second", baseCost: 260, perSecond: 8 },
  { id: "beauty", icon: "✿", name: "Beauty Brand", description: "+25 points per second", baseCost: 800, perSecond: 25 },
  { id: "production", icon: "▣", name: "Production Studio", description: "+75 points per second", baseCost: 2400, perSecond: 75 },
  { id: "foundation", icon: "♡", name: "Kindness Foundation", description: "+250 points per second", baseCost: 7000, perSecond: 250 }
];

const milestones = [
  { icon: "✦", title: "First Light", detail: "Earn 10 points", target: 10 },
  { icon: "◉", title: "On the Map", detail: "Earn 100 points", target: 100 },
  { icon: "✧", title: "Red Carpet Ready", detail: "Earn 1,000 points", target: 1000 },
  { icon: "★", title: "Household Name", detail: "Earn 10,000 points", target: 10000 },
  { icon: "♛", title: "Icon Status", detail: "Earn 100,000 points", target: 100000 }
];

const quotes = [
  "Every click is another step toward the premiere.",
  "The camera is ready. Make this moment count.",
  "A little confidence goes a very long way.",
  "The world is watching. Keep creating.",
  "Big dreams are built one bright moment at a time."
];

const defaultState = { points: 0, clicks: 0, clickPower: 1, purchased: {}, unlocked: [] };
let state = loadState();
let quoteIndex = 0;
let toastTimer;

const $ = (id) => document.getElementById(id);
const formatNumber = (value) => Math.floor(value).toLocaleString("en-US");
const costFor = (upgrade) => Math.floor(upgrade.baseCost * Math.pow(1.18, state.purchased[upgrade.id] || 0));
const passiveRate = () => upgrades.reduce((sum, upgrade) => sum + (upgrade.perSecond || 0) * (state.purchased[upgrade.id] || 0), 0);
const level = () => Math.max(1, Math.floor(Math.log10(Math.max(1, state.points))) + 1);

function loadState() {
  try {
    const saved = JSON.parse(localStorage.getItem("star-clicker-progress"));
    return saved ? { ...defaultState, ...saved } : { ...defaultState, purchased: {}, unlocked: [] };
  } catch {
    return { ...defaultState, purchased: {}, unlocked: [] };
  }
}

function saveState() {
  localStorage.setItem("star-clicker-progress", JSON.stringify(state));
  $("saveStatus").textContent = "SAVED JUST NOW";
  window.clearTimeout(saveState.statusTimer);
  saveState.statusTimer = window.setTimeout(() => $("saveStatus").textContent = "AUTO-SAVE ON", 1600);
}

function render() {
  $("points").textContent = formatNumber(state.points);
  $("clicks").textContent = formatNumber(state.clicks);
  $("rate").textContent = `+${formatNumber(passiveRate())} / sec`;
  $("levelLabel").textContent = `LEVEL ${level()}`;
  const purchasedCount = Object.values(state.purchased).filter(Boolean).length;
  $("upgradeCount").textContent = `${purchasedCount} / ${upgrades.length}`;
  $("upgradeList").innerHTML = upgrades.map((upgrade) => {
    const owned = state.purchased[upgrade.id] || 0;
    const cost = costFor(upgrade);
    return `<article class="upgrade">
      <span class="upgrade-icon">${upgrade.icon}</span>
      <div><h4>${upgrade.name} <span style="color:var(--yellow)">×${owned}</span></h4><p>${upgrade.description}</p></div>
      <button class="buy-button" data-upgrade="${upgrade.id}" ${state.points < cost ? "disabled" : ""}>${formatNumber(cost)} PTS</button>
    </article>`;
  }).join("");
  $("milestoneList").innerHTML = milestones.map((milestone) => {
    const isUnlocked = state.unlocked.includes(milestone.title);
    return `<article class="milestone ${isUnlocked ? "unlocked" : ""}">
      <span class="milestone-icon">${isUnlocked ? "✓" : milestone.icon}</span>
      <strong>${milestone.title}</strong><small>${milestone.detail}</small>
    </article>`;
  }).join("");
  $("milestoneProgress").textContent = `${state.unlocked.length} / ${milestones.length} unlocked`;
}

function checkMilestones() {
  milestones.forEach((milestone) => {
    if (state.points >= milestone.target && !state.unlocked.includes(milestone.title)) {
      state.unlocked.push(milestone.title);
      showToast(`MILESTONE UNLOCKED · ${milestone.title}`);
    }
  });
}

function showToast(message) {
  const toast = $("toast");
  toast.textContent = message;
  toast.classList.add("show");
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => toast.classList.remove("show"), 2600);
}

function addPoints(amount, event) {
  state.points += amount;
  if (event) {
    const point = document.createElement("span");
    point.className = "float-point";
    point.textContent = `+${amount}`;
    const rect = $("starButton").getBoundingClientRect();
    point.style.left = `${event.clientX || rect.left + rect.width / 2}px`;
    point.style.top = `${event.clientY || rect.top + rect.height / 2}px`;
    document.body.append(point);
    point.addEventListener("animationend", () => point.remove());
  }
  checkMilestones();
  render();
}

$("starButton").addEventListener("click", (event) => {
  state.clicks += 1;
  addPoints(state.clickPower, event);
  quoteIndex = (quoteIndex + 1) % quotes.length;
  $("quote").textContent = quotes[quoteIndex];
  saveState();
});

$("upgradeList").addEventListener("click", (event) => {
  const button = event.target.closest("[data-upgrade]");
  if (!button) return;
  const upgrade = upgrades.find((item) => item.id === button.dataset.upgrade);
  const cost = costFor(upgrade);
  if (!upgrade || state.points < cost) return;
  state.points -= cost;
  state.purchased[upgrade.id] = (state.purchased[upgrade.id] || 0) + 1;
  if (upgrade.clickPower) state.clickPower += upgrade.clickPower;
  showToast(`${upgrade.name.toUpperCase()} UNLOCKED`);
  saveState();
  render();
});

$("resetButton").addEventListener("click", () => {
  if (!window.confirm("Reset all Star Clicker progress?")) return;
  state = { ...defaultState, purchased: {}, unlocked: [] };
  saveState();
  render();
  showToast("PROGRESS RESET");
});

window.setInterval(() => {
  const rate = passiveRate();
  if (rate) {
    state.points += rate;
    checkMilestones();
    render();
    saveState();
  }
}, 1000);

render();
