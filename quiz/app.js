const STORAGE_KEY = "statische-pubquiz";

const elements = {
  appTitle: document.querySelector("#app-title"),
  quizTitle: document.querySelector("#quiz-title"),
  questionList: document.querySelector("#question-list"),
  questionForm: document.querySelector("#question-form"),
  emptyEditor: document.querySelector("#empty-editor"),
  questionEditorTitle: document.querySelector("#question-editor-title"),
  questionType: document.querySelector("#question-type"),
  typeFields: document.querySelector("#type-fields"),
  editorMessage: document.querySelector("#editor-message"),
  saveStatus: document.querySelector("#save-status"),
  presentationTitle: document.querySelector("#presentation-quiz-title"),
  questionCounter: document.querySelector("#question-counter"),
  presentationContent: document.querySelector("#presentation-content"),
  previous: document.querySelector("#previous-button"),
  next: document.querySelector("#next-button"),
  reveal: document.querySelector("#reveal-button"),
  importInput: document.querySelector("#import-input")
};

let quiz;
let selectedQuestionId;
let presentationIndex = 0;
let answerRevealed = false;
let timelineGame = null;

const alcoholDrinks = [
  { id: "beer", name: "Bier", percent: 5, image: "assets/drinks/beer.svg" },
  { id: "rose", name: "Rosé", percent: 12, image: "assets/drinks/rose.svg" },
  { id: "white-wine", name: "Witte wijn", percent: 12, image: "assets/drinks/white-wine.svg" },
  { id: "red-wine", name: "Rode wijn", percent: 13, image: "assets/drinks/red-wine.svg" },
  { id: "prosecco", name: "Prosecco", percent: 11, image: "assets/drinks/prosecco.svg" },
  { id: "champagne", name: "Champagne", percent: 12, image: "assets/drinks/champagne.svg" },
  { id: "sake", name: "Sake", percent: 15, image: "assets/drinks/sake.svg" },
  { id: "liqueur", name: "Likeur", percent: 20, image: "assets/drinks/liqueur.svg" },
  { id: "rum", name: "Rum", percent: 40, image: "assets/drinks/rum.svg" },
  { id: "vodka", name: "Wodka", percent: 40, image: "assets/drinks/vodka.svg" }
];

/*
 * New question types belong here. Each type owns its editor fields,
 * validation, and presentation rendering, while the app shell stays generic.
 */
const questionTypes = {
  text: {
    label: "Open vraag",
    create: () => ({ data: { prompt: "", answer: "" } }),
    renderEditor(question) {
      elements.typeFields.innerHTML = `
        <label for="question-prompt">Vraag</label>
        <textarea id="question-prompt" placeholder="Typ hier de vraag...">${escapeHtml(question.data.prompt)}</textarea>
        <label for="question-answer">Antwoord</label>
        <input id="question-answer" type="text" value="${escapeAttribute(question.data.answer)}" placeholder="Het juiste antwoord">
      `;
    },
    readEditor() {
      return { data: {
        prompt: document.querySelector("#question-prompt").value.trim(),
        answer: document.querySelector("#question-answer").value.trim()
      } };
    },
    validate(data) {
      if (!data.data.prompt || !data.data.answer) return "Vul zowel de vraag als het antwoord in.";
    },
    renderPresentation(question) {
      return `<div class="presentation-question"><span class="presentation-type">Open vraag</span><h2>${escapeHtml(question.data.prompt)}</h2>${answerRevealed ? `<div class="answer-box"><span class="answer-label">Antwoord</span><span class="answer-value">${escapeHtml(question.data.answer)}</span></div>` : ""}</div>`;
    }
  },
  alcoholTimeline: {
    label: "Alcoholtijdlijn",
    create: () => ({
      data: {
        drinks: alcoholDrinks.map((drink) => ({ ...drink })),
        startDrinkId: "beer",
        playerCount: 3
      }
    }),
    renderEditor(question) {
      const data = question.data;
      elements.typeFields.innerHTML = `
        <p class="field-help">Richtwaarden in volumeprocent. Deze kunnen per merk verschillen.</p>
        <label for="timeline-start">Startdrankje</label>
        <select id="timeline-start">
          ${data.drinks.map((drink) => `<option value="${drink.id}" ${drink.id === data.startDrinkId ? "selected" : ""}>${escapeHtml(drink.name)}</option>`).join("")}
        </select>
        <div class="drink-editor-list">
          ${data.drinks.map((drink) => `
            <div class="drink-editor-row" data-drink-editor="${drink.id}">
              <img src="${drink.image}" alt="">
              <input data-drink-name="${drink.id}" type="text" value="${escapeAttribute(drink.name)}" aria-label="Naam van ${escapeAttribute(drink.name)}">
              <input id="percent-${drink.id}" data-drink-percent="${drink.id}" type="number" min="0" max="60" step="0.1" value="${drink.percent}">
              <span>%</span>
              <label class="button secondary small-file-button">Afbeelding
                <input data-drink-image="${drink.id}" type="file" accept="image/*">
              </label>
              <button type="button" class="icon-button" data-remove-drink="${drink.id}" aria-label="Verwijder ${escapeAttribute(drink.name)}">×</button>
            </div>`).join("")}
        </div>
        <button type="button" class="button secondary" data-add-drink>Drankje toevoegen</button>
      `;
    },
    readEditor() {
      const selected = document.querySelector("#timeline-start").value;
      const drinks = [...document.querySelectorAll("[data-drink-editor]")].map((row) => {
        const id = row.dataset.drinkEditor;
        const existing = questionTypes.alcoholTimeline.currentQuestion.data.drinks.find((drink) => drink.id === id);
        return {
          ...existing,
          name: row.querySelector(`[data-drink-name="${id}"]`).value.trim() || "Onbekend drankje",
          percent: Number(row.querySelector(`[data-drink-percent="${id}"]`).value)
        };
      });
      return { data: { drinks, startDrinkId: selected, playerCount: 3 } };
    },
    validate(data) {
      const drinks = data.data.drinks;
      if (drinks.some((drink) => !Number.isFinite(drink.percent) || drink.percent < 0 || drink.percent > 60)) {
        return "Gebruik percentages tussen 0 en 60.";
      }
      if (drinks.length < 2) return "Voeg minimaal twee drankjes toe.";
      if (new Set(drinks.map((drink) => drink.id)).size !== drinks.length) return "Elk drankje moet uniek zijn.";
      if (!data.data.startDrinkId) return "Kies een startdrankje.";
    },
    renderPresentation(question) {
      return renderAlcoholTimeline(question);
    },
    onPresentationRendered(question) {
      setupAlcoholTimeline(question);
    }
  }
};

questionTypes.alcoholTimeline.currentQuestion = null;

quiz = loadQuiz();
selectedQuestionId = quiz.questions[0]?.id || null;

function createQuiz() {
  return { version: 1, title: "Mijn pubquiz", questions: [] };
}

function loadQuiz() {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    return saved ? normalizeQuiz(JSON.parse(saved)) : createQuiz();
  } catch (error) {
    console.warn("Kon opgeslagen quiz niet laden.", error);
    return createQuiz();
  }
}

function normalizeQuiz(value) {
  if (!value || typeof value !== "object" || !Array.isArray(value.questions)) throw new Error("Ongeldige quiz");
  return {
    version: 1,
    title: typeof value.title === "string" && value.title.trim() ? value.title.trim() : "Mijn pubquiz",
    questions: value.questions.filter((question) => question && questionTypes[question.type]).map((question) => {
      const legacyData = { prompt: question.prompt || "", answer: question.answer || "" };
      const defaults = questionTypes[question.type].create().data || {};
      const importedData = question.data && typeof question.data === "object" ? question.data : legacyData;
      const data = question.type === "alcoholTimeline"
        ? normalizeAlcoholData(importedData, defaults)
        : { ...defaults, ...importedData };
      return { ...question, id: question.id || makeId(), type: question.type, data };
    })
  };
}

function normalizeAlcoholData(data, defaults) {
  const sourceDrinks = Array.isArray(data.drinks) && data.drinks.length ? data.drinks : defaults.drinks;
  const drinks = sourceDrinks.map((drink, index) => {
    const fallback = alcoholDrinks.find((item) => item.id === drink.id) || alcoholDrinks[index % alcoholDrinks.length];
    return {
      ...fallback,
      ...drink,
      id: typeof drink.id === "string" && drink.id ? drink.id : makeId(),
      name: typeof drink.name === "string" && drink.name.trim() ? drink.name.trim() : fallback.name,
      percent: Number.isFinite(Number(drink.percent)) ? Number(drink.percent) : fallback.percent,
      image: typeof drink.image === "string" && drink.image ? drink.image : fallback.image
    };
  });
  return {
    drinks,
    startDrinkId: drinks.some((drink) => drink.id === data.startDrinkId) ? data.startDrinkId : defaults.startDrinkId,
    playerCount: 3
  };
}

function makeId() {
  return `question-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function saveQuiz() {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(quiz));
  elements.saveStatus.textContent = `Opgeslagen om ${new Date().toLocaleTimeString("nl-NL", { hour: "2-digit", minute: "2-digit" })}`;
}

function renderAll() {
  elements.appTitle.textContent = quiz.title || "Mijn pubquiz";
  elements.quizTitle.value = quiz.title;
  renderQuestionList();
  renderQuestionEditor();
  renderPresentation();
}

function getQuestionPreview(question) {
  if (question.type === "alcoholTimeline") return "Alcoholtijdlijn: 10 drankjes";
  return question.data?.prompt || "Nieuwe vraag";
}

function renderQuestionList() {
  if (!quiz.questions.length) {
    elements.questionList.innerHTML = `<div class="empty-state"><p>Nog geen vragen. Voeg je eerste vraag toe.</p></div>`;
    return;
  }
  elements.questionList.innerHTML = quiz.questions.map((question, index) => {
    const type = questionTypes[question.type];
    return `
      <div class="question-row ${question.id === selectedQuestionId ? "is-selected" : ""}">
        <button class="question-select" data-select-question="${question.id}">
          <span class="question-number">${index + 1}</span>
          <span class="question-preview">${escapeHtml(getQuestionPreview(question))}</span>
        </button>
        <span class="eyebrow">${type.label}</span>
        <div class="row-actions">
          <button class="icon-button" data-move-question="${question.id}" data-direction="-1" aria-label="Vraag omhoog" ${index === 0 ? "disabled" : ""}>↑</button>
          <button class="icon-button" data-move-question="${question.id}" data-direction="1" aria-label="Vraag omlaag" ${index === quiz.questions.length - 1 ? "disabled" : ""}>↓</button>
        </div>
      </div>`;
  }).join("");
}

function renderQuestionEditor() {
  const question = quiz.questions.find((item) => item.id === selectedQuestionId);
  elements.questionForm.hidden = !question;
  elements.emptyEditor.hidden = Boolean(question);
  if (!question) return;
  elements.questionEditorTitle.textContent = `Vraag ${quiz.questions.indexOf(question) + 1}`;
  elements.questionType.innerHTML = Object.entries(questionTypes).map(([value, type]) => `<option value="${value}">${type.label}</option>`).join("");
  elements.questionType.value = question.type;
  if (question.type === "alcoholTimeline") questionTypes.alcoholTimeline.currentQuestion = question;
  questionTypes[question.type].renderEditor(question);
}

function getTimelineGame(question) {
  if (!timelineGame || timelineGame.questionId !== question.id) {
    const startDrink = question.data.drinks.find((drink) => drink.id === question.data.startDrinkId) || question.data.drinks[0];
    timelineGame = {
      questionId: question.id,
      placed: startDrink ? [startDrink.id] : [],
      pendingId: null,
      player: 1,
      pendingSlot: 1,
      pendingDropped: false,
      lastResult: null
    };
    timelineGame.pendingId = getNextTimelineDrink(question);
  }
  return timelineGame;
}

function getNextTimelineDrink(question) {
  const game = timelineGame;
  return question.data.drinks.find((drink) => !game.placed.includes(drink.id))?.id || null;
}

function renderAlcoholTimeline(question) {
  const game = getTimelineGame(question);
  const drinks = question.data.drinks;
  const placedDrinks = game.placed.map((id) => drinks.find((drink) => drink.id === id)).filter(Boolean);
  const pending = drinks.find((drink) => drink.id === game.pendingId);
  const sorted = [...drinks].sort((a, b) => a.percent - b.percent || a.name.localeCompare(b.name));
  const isComplete = !pending;
  const answerOverview = answerRevealed ? `
    <div class="alcohol-answer-overview">
      <div class="answer-label">Antwoordenoverzicht · richtwaarden</div>
      <p class="alcohol-disclaimer">Alcoholpercentages kunnen per merk verschillen.</p>
      <ol>${sorted.map((drink) => `<li><strong>${escapeHtml(drink.name)}</strong><span>${drink.percent}%</span></li>`).join("")}</ol>
    </div>` : "";

  return `
    <div class="alcohol-game">
      <div class="alcohol-game-header">
        <div>
          <span class="presentation-type">Alcoholtijdlijn</span>
          <h2>Van licht naar sterk</h2>
        </div>
        <div class="player-turn">Speler <strong>${game.player}</strong> is aan de beurt</div>
      </div>
      ${game.lastResult ? `<div class="alcohol-result ${game.lastResult.correct ? "is-correct" : "is-wrong"}">${escapeHtml(game.lastResult.message)}</div>` : ""}
      <div class="timeline-area">
        <div class="timeline-scale"><span>0%</span><span>10%</span><span>20%</span><span>30%</span><span>40%</span><span>50%</span><span>60%</span></div>
        <div class="alcohol-timeline" data-timeline>
          <div class="timeline-line"></div>
          <div class="timeline-order-track" data-order-track>
            ${placedDrinks.map((drink, index) => `
              ${pending ? `<div class="timeline-drop-slot ${game.pendingSlot === index ? (game.pendingDropped ? "is-dropped" : "is-selected") : ""}" data-drop-slot="${index}">${game.pendingSlot === index && game.pendingDropped ? renderPendingTimelineDrink(pending, answerRevealed) : "Plaats hier"}</div>` : ""}
              <div class="timeline-drink is-placed" title="${escapeAttribute(drink.name)}">
                <img src="${drink.image}" alt="${escapeAttribute(drink.name)}">
                <span>${escapeHtml(drink.name)}</span>
                ${answerRevealed ? `<b>${drink.percent}%</b>` : ""}
              </div>`).join("")}
            ${pending ? `<div class="timeline-drop-slot ${game.pendingSlot === placedDrinks.length ? (game.pendingDropped ? "is-dropped" : "is-selected") : ""}" data-drop-slot="${placedDrinks.length}">${game.pendingSlot === placedDrinks.length && game.pendingDropped ? renderPendingTimelineDrink(pending, answerRevealed) : "Plaats hier"}</div>` : ""}
          </div>
          ${pending && !game.pendingDropped ? `
            <div class="pending-drink-tray">
              <div class="timeline-drink is-draggable is-pending" data-pending-drink title="${escapeAttribute(pending.name)}">
                <img src="${pending.image}" alt="${escapeAttribute(pending.name)}">
                <span>${escapeHtml(pending.name)}</span>
              </div>
            </div>` : ""}
          ${pending ? `<div class="timeline-drop-hint">Sleep het drankje tussen de bestaande drankjes</div>` : `<div class="timeline-complete">Alle drankjes staan op de tijdlijn.</div>`}
        </div>
      </div>
      <div class="alcohol-game-actions">
        <span>${placedDrinks.length} van ${drinks.length} drankjes geplaatst</span>
        <div>
          <button class="button secondary" data-alcohol-reset>Opnieuw beginnen</button>
          <button class="button primary" data-alcohol-check ${isComplete ? "disabled" : ""}>Controleer plaatsing</button>
          <button class="button secondary" data-alcohol-answers>${answerRevealed ? "Verberg antwoorden" : "Antwoordenoverzicht"}</button>
        </div>
      </div>
      ${answerOverview}
    </div>`;
}

function renderPendingTimelineDrink(drink, revealed) {
    return `
      <div class="timeline-drink is-draggable is-pending is-dropped" data-pending-drink title="${escapeAttribute(drink.name)}">
        <img src="${drink.image}" alt="${escapeAttribute(drink.name)}">
        <span>${escapeHtml(drink.name)}</span>
        ${revealed ? `<b>${drink.percent}%</b>` : ""}
      </div>`;
}

function setupAlcoholTimeline(question) {
  const game = timelineGame;
  const content = elements.presentationContent;
  const timeline = content.querySelector("[data-timeline]");
  const orderTrack = content.querySelector("[data-order-track]");
  const pendingElement = content.querySelector("[data-pending-drink]");
  updateTimelineAlignment(orderTrack);
  if (!timeline || !pendingElement) {
    bindAlcoholButtons(question);
    return;
  }

  let dragging = false;
  let startPoint = { x: 0, y: 0 };
  const move = (event) => {
    if (!dragging) return;
    const trackRect = orderTrack.getBoundingClientRect();
    const edgeDistance = 72;
    if (event.clientX < trackRect.left + edgeDistance) {
      orderTrack.scrollLeft -= Math.max(4, (trackRect.left + edgeDistance - event.clientX) / 6);
    } else if (event.clientX > trackRect.right - edgeDistance) {
      orderTrack.scrollLeft += Math.max(4, (event.clientX - (trackRect.right - edgeDistance)) / 6);
    }
    pendingElement.style.setProperty("--drag-x", `${event.clientX - startPoint.x}px`);
    pendingElement.style.setProperty("--drag-y", `${event.clientY - startPoint.y}px`);
    const slots = [...content.querySelectorAll("[data-drop-slot]")];
    if (!slots.length) return;
    const pointerX = event.clientX;
    const closest = slots.reduce((best, slot) => {
      const rect = slot.getBoundingClientRect();
      const distance = Math.abs(pointerX - (rect.left + rect.width / 2));
      return distance < best.distance ? { slot, distance } : best;
    }, { slot: slots[0], distance: Number.POSITIVE_INFINITY }).slot;
    game.pendingSlot = Number(closest.dataset.dropSlot);
    content.querySelectorAll("[data-drop-slot]").forEach((slot) => slot.classList.toggle("is-selected", Number(slot.dataset.dropSlot) === game.pendingSlot));
  };
  const stop = () => {
    if (!dragging) return;
    dragging = false;
    pendingElement.classList.remove("is-dragging");
    pendingElement.style.removeProperty("--drag-x");
    pendingElement.style.removeProperty("--drag-y");
    game.pendingDropped = true;
    renderPresentation();
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", stop);
    window.removeEventListener("pointercancel", stop);
  };
  pendingElement.addEventListener("pointerdown", (event) => {
    dragging = true;
    startPoint = { x: event.clientX, y: event.clientY };
    pendingElement.classList.add("is-dragging");
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop);
    window.addEventListener("pointercancel", stop);
    event.preventDefault();
  });
  content.querySelectorAll("[data-drop-slot]").forEach((slot) => {
    slot.addEventListener("click", () => {
      game.pendingSlot = Number(slot.dataset.dropSlot);
      game.pendingDropped = true;
      renderPresentation();
    });
  });
  bindAlcoholButtons(question);
}

function updateTimelineAlignment(orderTrack) {
  if (!orderTrack) return;
  const align = () => {
    orderTrack.classList.toggle("is-overflowing", orderTrack.scrollWidth > orderTrack.clientWidth + 1);
  };
  align();
  window.requestAnimationFrame(align);
}

function bindAlcoholButtons(question) {
  const game = timelineGame;
  elements.presentationContent.querySelector("[data-alcohol-check]")?.addEventListener("click", () => {
    if (!game.pendingId) return;
    const pending = question.data.drinks.find((drink) => drink.id === game.pendingId);
    const proposedIds = [...game.placed];
    proposedIds.splice(game.pendingSlot, 0, pending.id);
    const expectedIds = [...question.data.drinks]
      .sort((a, b) => a.percent - b.percent || a.name.localeCompare(b.name))
      .filter((drink) => proposedIds.includes(drink.id))
      .map((drink) => drink.id);
    const correct = proposedIds.join("|") === expectedIds.join("|");
    if (correct) {
      game.placed.splice(game.pendingSlot, 0, pending.id);
      game.pendingId = getNextTimelineDrink(question);
      game.pendingSlot = Math.min(1, game.placed.length);
      game.pendingDropped = false;
      game.lastResult = { correct: true, message: `Goed! ${pending.name} staat op de juiste plek. Speler ${game.player} heeft het goed.` };
    } else {
      game.lastResult = { correct: false, message: `Nog niet goed: ${pending.name} hoort niet op deze plek. De volgende speler mag het proberen.` };
      game.pendingDropped = false;
    }
    game.player = game.player % 3 + 1;
    answerRevealed = false;
    renderPresentation();
  });
  elements.presentationContent.querySelector("[data-alcohol-reset]")?.addEventListener("click", () => {
    timelineGame = null;
    answerRevealed = false;
    renderPresentation();
  });
  elements.presentationContent.querySelector("[data-alcohol-answers]")?.addEventListener("click", () => {
    answerRevealed = !answerRevealed;
    renderPresentation();
  });
}

function renderPresentation() {
  elements.presentationTitle.textContent = quiz.title || "Mijn pubquiz";
  if (!quiz.questions.length) {
    elements.questionCounter.textContent = "0 / 0";
    elements.presentationContent.innerHTML = `<p class="presentation-empty">Voeg eerst vragen toe in Beheren.</p>`;
    elements.previous.disabled = true;
    elements.next.disabled = true;
    elements.reveal.disabled = true;
    elements.previous.hidden = false;
    elements.next.hidden = false;
    elements.reveal.hidden = false;
    return;
  }
  presentationIndex = Math.min(presentationIndex, quiz.questions.length - 1);
  const question = quiz.questions[presentationIndex];
  elements.previous.hidden = false;
  elements.next.hidden = false;
  elements.reveal.hidden = question.type === "alcoholTimeline";
  elements.questionCounter.textContent = `${presentationIndex + 1} / ${quiz.questions.length}`;
  elements.presentationContent.innerHTML = questionTypes[question.type].renderPresentation(question);
  questionTypes[question.type].onPresentationRendered?.(question);
  elements.previous.disabled = presentationIndex === 0;
  elements.next.disabled = presentationIndex === quiz.questions.length - 1;
  elements.reveal.disabled = false;
  elements.reveal.textContent = answerRevealed ? "Verberg antwoord" : "Toon antwoord";
}

function showMessage(message) {
  elements.editorMessage.textContent = message;
  window.setTimeout(() => {
    if (elements.editorMessage.textContent === message) elements.editorMessage.textContent = "";
  }, 3000);
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#039;" }[character]));
}

function escapeAttribute(value) {
  return escapeHtml(value).replace(/`/g, "&#096;");
}

document.querySelectorAll("[data-view]").forEach((button) => {
  button.addEventListener("click", () => {
    document.querySelectorAll(".tab").forEach((tab) => tab.classList.toggle("is-active", tab === button));
    document.querySelectorAll(".view").forEach((view) => view.classList.toggle("is-active", view.id === `${button.dataset.view}-view`));
    if (button.dataset.view === "presentation") renderPresentation();
  });
});

elements.quizTitle.addEventListener("input", () => {
  quiz.title = elements.quizTitle.value.trim() || "Mijn pubquiz";
  saveQuiz();
  elements.appTitle.textContent = quiz.title;
});

document.querySelector("#add-question-button").addEventListener("click", () => {
  const question = { id: makeId(), type: "text", ...questionTypes.text.create() };
  quiz.questions.push(question);
  selectedQuestionId = question.id;
  saveQuiz();
  renderAll();
  document.querySelector("#question-prompt")?.focus();
});

elements.questionList.addEventListener("click", (event) => {
  const selectButton = event.target.closest("[data-select-question]");
  const moveButton = event.target.closest("[data-move-question]");
  if (selectButton) {
    selectedQuestionId = selectButton.dataset.selectQuestion;
    renderQuestionList();
    renderQuestionEditor();
  }
  if (moveButton) {
    const index = quiz.questions.findIndex((question) => question.id === moveButton.dataset.moveQuestion);
    const nextIndex = index + Number(moveButton.dataset.direction);
    if (index >= 0 && nextIndex >= 0 && nextIndex < quiz.questions.length) {
      [quiz.questions[index], quiz.questions[nextIndex]] = [quiz.questions[nextIndex], quiz.questions[index]];
      saveQuiz();
      renderAll();
    }
  }
});

elements.questionType.addEventListener("change", () => {
  const question = quiz.questions.find((item) => item.id === selectedQuestionId);
  if (!question || !questionTypes[elements.questionType.value]) return;
  question.type = elements.questionType.value;
  Object.assign(question, questionTypes[question.type].create());
  renderQuestionEditor();
});

elements.typeFields.addEventListener("click", (event) => {
  const addButton = event.target.closest("[data-add-drink]");
  const removeButton = event.target.closest("[data-remove-drink]");
  const question = quiz.questions.find((item) => item.id === selectedQuestionId);
  if (!question || question.type !== "alcoholTimeline") return;
  if (addButton) {
    const id = `drink-${Date.now()}-${Math.random().toString(16).slice(2)}`;
    question.data.drinks.push({
      id,
      name: "Nieuw drankje",
      percent: 10,
      image: "assets/drinks/liqueur.svg"
    });
    renderQuestionEditor();
    document.querySelector(`[data-drink-name="${id}"]`)?.focus();
  }
  if (removeButton) {
    if (question.data.drinks.length <= 2) {
      showMessage("Er moeten minimaal twee drankjes blijven.");
      return;
    }
    const id = removeButton.dataset.removeDrink;
    question.data.drinks = question.data.drinks.filter((drink) => drink.id !== id);
    if (question.data.startDrinkId === id) question.data.startDrinkId = question.data.drinks[0].id;
    renderQuestionEditor();
  }
});

elements.typeFields.addEventListener("change", (event) => {
  const input = event.target.closest("[data-drink-image]");
  const question = quiz.questions.find((item) => item.id === selectedQuestionId);
  if (!input || !question || question.type !== "alcoholTimeline" || !input.files[0]) return;
  const reader = new FileReader();
  reader.addEventListener("load", () => {
    const drink = question.data.drinks.find((item) => item.id === input.dataset.drinkImage);
    if (!drink || typeof reader.result !== "string") return;
    drink.image = reader.result;
    saveQuiz();
    renderQuestionEditor();
    showMessage("Afbeelding opgeslagen.");
  });
  reader.readAsDataURL(input.files[0]);
});

elements.questionForm.addEventListener("submit", (event) => {
  event.preventDefault();
  const question = quiz.questions.find((item) => item.id === selectedQuestionId);
  if (!question) return;
  const type = questionTypes[question.type];
  const data = type.readEditor();
  const error = type.validate?.(data);
  if (error) {
    showMessage(error);
    return;
  }
  Object.assign(question, data);
  saveQuiz();
  renderQuestionList();
  showMessage("Vraag opgeslagen.");
});

document.querySelector("#delete-question-button").addEventListener("click", () => {
  const index = quiz.questions.findIndex((question) => question.id === selectedQuestionId);
  if (index < 0 || !window.confirm("Deze vraag verwijderen?")) return;
  quiz.questions.splice(index, 1);
  selectedQuestionId = quiz.questions[index]?.id || quiz.questions[index - 1]?.id || null;
  saveQuiz();
  renderAll();
});

document.querySelector("#new-quiz-button").addEventListener("click", () => {
  if (!window.confirm("Een nieuwe quiz starten? De huidige quiz blijft beschikbaar via export, maar wordt lokaal vervangen.")) return;
  quiz = createQuiz();
  selectedQuestionId = null;
  presentationIndex = 0;
  answerRevealed = false;
  saveQuiz();
  renderAll();
});

document.querySelector("#export-button").addEventListener("click", () => {
  const blob = new Blob([JSON.stringify(quiz, null, 2)], { type: "application/json" });
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob);
  link.download = `${(quiz.title || "pubquiz").toLowerCase().replace(/[^a-z0-9]+/g, "-")}.json`;
  link.click();
  URL.revokeObjectURL(link.href);
});

elements.importInput.addEventListener("change", async () => {
  const file = elements.importInput.files[0];
  if (!file) return;
  try {
    quiz = normalizeQuiz(JSON.parse(await file.text()));
    selectedQuestionId = quiz.questions[0]?.id || null;
    presentationIndex = 0;
    answerRevealed = false;
    saveQuiz();
    renderAll();
    showMessage("Quiz geimporteerd.");
  } catch (error) {
    showMessage("Importeren mislukt: kies een geldig quiz-JSON-bestand.");
  } finally {
    elements.importInput.value = "";
  }
});

elements.previous.addEventListener("click", () => {
  if (presentationIndex > 0) {
    presentationIndex -= 1;
    answerRevealed = false;
    renderPresentation();
  }
});

elements.next.addEventListener("click", () => {
  if (presentationIndex < quiz.questions.length - 1) {
    presentationIndex += 1;
    answerRevealed = false;
    renderPresentation();
  }
});

elements.reveal.addEventListener("click", () => {
  answerRevealed = !answerRevealed;
  renderPresentation();
});

renderAll();
